//! =============================================================
//!  柔性适配层 Flex Adapter（核心模块 · 交付物 4）
//! =============================================================
//!  子模块划分：
//!    3-1 capability_cache    后台节点能力探测（toolcall、多模态、窗口、stream）
//!    3-2 param_adjust        参数改写（删除不支持字段 / 截断 max_tokens / response_format 降级）
//!    3-3 protocol_sniffer    多候选协议嗅探（仅非流式）+ 成功记忆缓存
//!    3-4 flexible_parser     宽容 Schema 解析（strict/flex 双模式，字段别名数组）
//!    3-5 retry_controller    多级试探回退控制（非流式 MAX_ATTEMPTS 硬上限）
//!    3-6 retry_analysis      故障研判 →  human_readable_reason + 错误标签
//!    3-7 upstream_fingerprint 响应头/错误栈指纹 → 上游语言/网关版本标签（仅日志统计）
//!
//!  ⚠ 总原则 4：流式只要向客户端吐出过任意 chunk，立即锁死当前协议/节点，禁止重试/试探/回退。
//! =============================================================
pub mod capability_cache;
pub mod param_adjust;
pub mod protocol_sniffer;
pub mod flexible_parser;
pub mod retry_controller;
pub mod retry_analysis;
pub mod upstream_fingerprint;

use crate::error::AppResult;
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, SseChunk};
use crate::observability::trace::{GatewayTrace, SubAttempt};
use crate::router::CandidateNode;
use crate::state::AppState;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;

/// 流内错误脱敏：只给客户端"标签级"消息，绝不透出上游 URL/token/诊断细节。
/// 完整错误由调用方先写 tracing 日志。
fn sanitize_stream_err(e: &crate::error::AppError) -> String {
    let label = e.label();
    let msg = crate::error::client_message_for_label_pub(&label);
    format!("{label}: {msg}")
}

/// FlexAdapter 执行非流式请求 → 统一返回标准 OpenAI ChatCompletionResponse
/// 内部：
///   1. capability_cache 查能力 → param_adjust 改写请求
///   2. retry_controller 按候选节点依次尝试
///   3. protocol_sniffer 对每个节点尝试多协议（仅非流式）
///   4. flexible_parser 在 strict 失败时走 flex 模板
///   5. 每次尝试生成 SubAttempt，失败调用 retry_analysis 写 human_readable_reason
pub async fn execute_non_stream(
    state:           &Arc<AppState>,
    trace:           &mut GatewayTrace,
    candidates:      &[CandidateNode],
    mut req:         ChatCompletionRequest,
) -> AppResult<ChatCompletionResponse> {
    use capability_cache::apply_capability_constraints;
    use retry_controller::{attempt_chain, AttemptOutcome};

    apply_capability_constraints(state, candidates, &mut req).await;
    let outcome = attempt_chain(state, trace, candidates, req, /*stream*/false).await;

    match outcome {
        AttemptOutcome::Ok(resp, final_sub) => {
            trace.sub_attempt_ids.push(final_sub.sub_attempt_id.clone());
            Ok(resp)
        }
        AttemptOutcome::AllFailed(all_subs, final_err, human_reason) => {
            for s in all_subs { trace.sub_attempt_ids.push(s.sub_attempt_id); }
            trace.human_readable_reason = Some(human_reason);
            Err(final_err)
        }
    }
}

/// FlexAdapter 执行流式请求 → 统一返回标准 OpenAI SSE chunk 流
/// ⚠️ 流式一旦向客户端吐过首字节即锁定当前节点/协议；后续 parse 失败只能直接 close stream，不得切节点
///
/// 候选链在「响应头阶段」逐个尝试（此时还未向客户端输出任何字节，切换候选不违反流式锁死原则）：
///   - 建连失败 / 30s 无响应头 / 非 2xx 状态 → 记录 sub attempt，换下一个候选
///   - 某个候选成功返回 2xx → 锁定该候选，把 bytes_stream 包装成 SSE 翻译流
///
/// 这里使用 async_stream::try_stream!，把上游 SSE 逐步翻译为 `SseChunk` 后 yield。
/// 上游 `data: [DONE]` 会结束本次流（不 yield）。
pub async fn execute_stream(
    state:           &Arc<AppState>,
    mut trace:       GatewayTrace, // moved 到 stream 任务内，结束时入库
    candidates:      &[CandidateNode],
    mut req:         ChatCompletionRequest,
) -> AppResult<Pin<Box<dyn Stream<Item = Result<SseChunk, String>> + Send + 'static>>> {
    use futures::StreamExt;

    // 流式头阶段候选数：全量尝试（AUTOMODE 候选抽样上限 48）
    // 用户要求「多试试多等待」：坏源占名额也好过漏掉排在后面的可用源
    let max_stream_candidates = candidates.len().min(48);
    let mask = state.cfg_swap.load().masking.clone();

    let mut last_err_msg = String::from("无可用候选");
    let mut tried = 0usize;
    // 已尝试清单：host + 结果（用户要求错误里必须说明试过哪些链接；只含 host/状态码，绝无 token）
    let mut tried_list: Vec<String> = Vec::new();

    for c in candidates.iter().take(max_stream_candidates) {
        tried += 1;
        let c_host = endpoint_host(&c.endpoint);
        // 每个候选用它自己的 real_model（多源同名场景各候选模型名不同）
        req.model = c.real_model.clone();
        // 错误清单条目：host[模型名]:结果 —— 定位「哪个源的哪个模型」失败
        let c_label = format!("{c_host}[{}]", c.real_model);
        let proto = crate::flex_adapter::protocol_sniffer::try_cached(state, &c.node_id).unwrap_or(c.protocol);
        let adapter = crate::backend_adapters::adapter_for(proto);
        let masked = c.to_masked(&mask);

        let mut sub = SubAttempt::new(&trace.trace_id, &c.node_id, &c.group_id);
        sub.masked_endpoint = masked.endpoint.clone();
        sub.masked_token   = masked.api_key.clone();
        sub.protocol = proto.to_string();

        // 构建请求 → 发送 → 45s 内必须拿到响应头（多等一会，慢源也 给机会）
        let resp = match adapter.translate_request(&req, c).await {
            Ok(rb) => {
                let send_fut = rb.send();
                match tokio::time::timeout(std::time::Duration::from_secs(45), send_fut).await {
                    Ok(Ok(r)) => Some(r),
                    Ok(Err(e)) => {
                        tracing::error!(target: "flex_adapter", "stream send fail (node={}): {e}", c.node_id);
                        last_err_msg = "上游连接失败".into();
                        tried_list.push(format!("{c_host}:连接失败"));
                        None
                    }
                    Err(_) => {
                        tracing::error!(target: "flex_adapter", "stream send timeout (45s, no response header, node={})", c.node_id);
                        last_err_msg = "上游 45 秒未响应（已超时）".into();
                        tried_list.push(format!("{c_label}:45s超时"));
                        None
                    }
                }
            }
            Err(e) => {
                tracing::error!(target: "flex_adapter", "stream translate_request fail (node={}): {e}", c.node_id);
                last_err_msg = "请求构建失败".into();
                tried_list.push(format!("{c_host}:构建失败"));
                None
            }
        };

        // 响应头阶段非 2xx → 换下一个候选（未输出任何字节，可安全重试）
        if let Some(r) = resp {
            let status = r.status();
            sub.http_status_code = Some(status.as_u16());
            if status.is_success() {
                // 锁定该候选：finish_ok 后进入流翻译（此后禁止任何重试）
                sub.finish_ok(status.as_u16(), crate::observability::trace::UsageSnapshot::default());
                trace.sub_attempt_ids.push(sub.sub_attempt_id.clone());
                return Ok(build_sse_stream(state.clone(), trace, r, adapter));
            }
            tracing::warn!(target: "flex_adapter", "stream upstream status {} (node={}) → try next candidate", status.as_u16(), c.node_id);
            last_err_msg = format!("upstream status {}", status.as_u16());
            tried_list.push(format!("{c_label}:{status}"));
            sub.finish_fail(crate::error::ErrorLabel::Upstream5xx, Some(status.as_u16()), crate::observability::trace::SubAttemptOutcome::FailedRetried);
            trace.sub_attempt_ids.push(sub.sub_attempt_id);
            continue;
        } else {
            sub.finish_fail(crate::error::ErrorLabel::Unknown, None, crate::observability::trace::SubAttemptOutcome::FailedRetried);
            trace.sub_attempt_ids.push(sub.sub_attempt_id);
        }
    }
    let _ = tried;

    // 所有候选头阶段全失败：落库 trace 并返回明细（用户要求必须列出试过哪些源）
    let mut detail = format!("已依次尝试 {tried} 个候选上游全部失败：{}", tried_list.join(" → "));
    detail.push_str(&format!("（末次错误：{last_err_msg}）。可稍后重试，或在「上游」页逐个用「获取模型列表」排查失效源。"));
    trace.human_readable_reason = Some(detail.clone());
    trace.close(502, None);
    crate::services::trace_store::record(state, &trace).await;
    Err(crate::error::AppError::ChainFailed { detail })
}

/// 从 endpoint 提取 host[:port]（去协议/路径，用于错误明细展示；不含 token/查询串）
fn endpoint_host(endpoint: &str) -> String {
    let e = endpoint.trim().trim_end_matches('/');
    let rest = e.split_once("://").map(|(_, r)| r).unwrap_or(e);
    rest.split('/').next().unwrap_or("").to_string()
}

/// 把已成功建立的上游响应流包装为 SSE 翻译流（锁定候选，不再重试）
fn build_sse_stream(
    state: Arc<AppState>,
    mut trace: GatewayTrace,
    resp: reqwest::Response,
    adapter: Box<dyn crate::backend_adapters::BackendAdapter>,
) -> Pin<Box<dyn Stream<Item = Result<SseChunk, String>> + Send + 'static>> {
    let stream = async_stream::try_stream! {
        use futures::StreamExt;
        let mut incoming = resp.bytes_stream();
        let mut line_buf: Vec<u8> = Vec::new();
        let mut done = false;
        // 流式 usage 累计（Anthropic 分 message_start/message_delta 两段给；OpenAI 在末块给累计值）
        let mut u_prompt: u32 = 0;
        let mut u_completion: u32 = 0;
        while let Some(chunk) = incoming.next().await {
            if done { break; }
            let bytes = match chunk {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!(target: "flex_adapter", "stream read fail: {e}");
                    Result::<(), String>::Err(sanitize_stream_err(&crate::error::AppError::Reqwest(e)))?;
                    unreachable!()
                }
            };
            line_buf.extend_from_slice(&bytes);
            // 逐行切分（SSE 事件以换行分隔）。try_stream! 会把 yield 的值包成 Ok(item)
            while let Some(pos) = line_buf.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = line_buf.drain(..=pos).collect();
                let line_str = String::from_utf8_lossy(&line)
                    .trim_end_matches('\n').trim_end_matches('\r').to_string();
                let trimmed = line_str.trim();
                if trimmed.is_empty() || trimmed.starts_with(':') { continue; }
                // OpenAI 以 `data: [DONE]` 收尾；逐行交给适配器（OpenAI 解析 data: 行，Ollama 解析裸 JSON 行）
                if trimmed == "data: [DONE]" { done = true; break; }
                match adapter.translate_sse_chunk(&line_str) {
                    Ok(Some(c)) => {
                        if let Some(u) = &c.usage {
                            u_prompt = u_prompt.max(u.prompt_tokens);
                            u_completion = u_completion.max(u.completion_tokens);
                        }
                        yield c;
                    }
                    Ok(None) => {}
                    Err(e) => Result::<(), String>::Err(sanitize_stream_err(&e))?,
                }
            }
        }
        // 流式正常结束：回填 usage（否则 trace 永远 0 tokens）→ 落库这条 trace
        trace.billed_usage = crate::observability::trace::UsageSnapshot {
            prompt_tokens: u_prompt,
            completion_tokens: u_completion,
            total_tokens: u_prompt + u_completion,
        };
        trace.close(200, None);
        crate::services::trace_store::record(&state, &trace).await;
    };
    Box::pin(stream)
}
