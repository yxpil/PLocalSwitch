//! =============================================================
//!  交付物 4：retry_controller —— attempt_chain 非流式真实执行
//!  核心循环：遍历候选节点 → 每个节点按 cached/sniffed protocol 调 adapter
//!            → 真实发起上游 HTTP 请求 → 解析响应 → 成功即返回
//!            → 失败按 label 决定是否继续下一个节点 / 是否终止
//! =============================================================
use crate::backend_adapters::adapter_for;
use crate::error::{AppError, ErrorLabel};
use crate::flex_adapter::retry_analysis::{build_human_reason, RecentWindowStats};
use crate::models::{ChatCompletionRequest, ChatCompletionResponse};
use crate::observability::trace::{GatewayTrace, SubAttempt, SubAttemptOutcome, UsageSnapshot};
use crate::router::{CandidateNode, ProtocolKind};
use crate::state::AppState;
use std::sync::Arc;

pub enum AttemptOutcome {
    /// 成功：最终响应 + 最后一个 ok 的 SubAttempt
    Ok(ChatCompletionResponse, SubAttempt),
    /// 全失败：所有产生的 SubAttempt + 最终 error + 运维侧诊断
    AllFailed(Vec<SubAttempt>, AppError, String),
}

/// 真实发起一次上游尝试（不负责 retry 决策；只负责请求 + 解析 + 成功时写 sub）
async fn execute_one(
    c:    &CandidateNode,
    req:  &ChatCompletionRequest,
    proto: ProtocolKind,
    sub:  &mut SubAttempt,
) -> Result<ChatCompletionResponse, AppError> {
    let adapter = adapter_for(proto);
    let rb = adapter.translate_request(req, c).await?;
    let resp = rb.send().await?; // reqwest::Error -> AppError::Reqwest
    let status = resp.status();
    sub.http_status_code = Some(status.as_u16());
    if !status.is_success() {
        let body_txt = resp.text().await.unwrap_or_default();
        let label = match status.as_u16() {
            401 | 403 => ErrorLabel::Auth401403,
            429        => ErrorLabel::Http429,
            400 | 404 | 405 => ErrorLabel::BadParam4xx,
            406..=499  => ErrorLabel::BadParam4xx,
            500..=599  => ErrorLabel::Upstream5xx,
            _          => ErrorLabel::Unknown,
        };
        // 带上游响应体（截断），供「协议不支持 → 自适应换协议」识别；客户端最终仍按 label 脱敏输出
        let detail = if body_txt.len() > 240 { body_txt[..240].to_string() } else { body_txt };
        return Err(AppError::Labeled { label, message: format!("upstream status {} - {}", status.as_u16(), detail) });
    }
    let bytes = resp.bytes().await?;
    let parsed = adapter.parse_response_body(bytes)?;
    let u = parsed.usage.clone().unwrap_or_default();
    sub.finish_ok(status.as_u16(), UsageSnapshot {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
    });
    Ok(parsed)
}

/// 判断某个上游错误是否表示「协议不支持/不匹配」（如 kimi 模型只走 Anthropic 而节点配成 OpenAI）。
/// 命中则说明当前协议选错了，应继续换下一个候选协议（自适应）。
fn protocol_mismatch(e: &AppError) -> bool {
    let s = e.to_string();
    s.contains("protocol_not_supported")
        || s.contains("unsupported")
        || s.contains("not support")
        || s.contains("不支持的协议")
        || s.contains("不支持")
        || s.contains("协议")
        || s.contains("protocol")
}

#[allow(unused_variables)]
pub async fn attempt_chain(
    state:      &Arc<AppState>,
    trace:      &mut GatewayTrace,
    candidates: &[CandidateNode],
    req:        ChatCompletionRequest,
    is_stream:  bool,
) -> AttemptOutcome {
    // 运行时配置从 cfg_swap 读取（跟随热更新；masking 同理，避免改动后日志脱敏失效）。
    // 块内提取后立即释放 ArcSwap guard，避免跨 await 持有导致 future !Send。
    let (max_subs, policy, mask) = {
        let cfg_rt = state.cfg_swap.load();
        (
            cfg_rt.flex_adapter.global_max_sub_attempts.max(1),
            cfg_rt.policy.clone(),
            cfg_rt.masking.clone(),
        )
    };
    let mut subs: Vec<SubAttempt> = Vec::new();
    let mut last_err: Option<AppError> = None;

    // ------- 流式：由 flex_adapter::execute_stream 单独处理（此处仅兜底）-------
    if is_stream {
        let err = AppError::Labeled { label: ErrorLabel::Internal, message: "stream via attempt_chain not supported".into() };
        let hr = build_human_reason(trace, &subs, &policy, &RecentWindowStats::default());
        return AttemptOutcome::AllFailed(subs, err, hr);
    }

    // ------- 非流式：真实多轮候选执行（自适应：上游「协议不支持」时自动换下一个候选协议）-------
    'outer: for c in candidates {
        if subs.len() as u32 >= max_subs { break; }
        let masked = c.to_masked(&mask);
        // 协议候选序列：已记忆协议优先 → hints+兜底去重。用于「协议不支持」时自动换协议。
        let mut protos: Vec<ProtocolKind> = Vec::new();
        if let Some(p) = crate::flex_adapter::protocol_sniffer::try_cached(state, &c.node_id) { protos.push(p); }
        for p in c.candidate_protocols.iter().copied() { if !protos.contains(&p) { protos.push(p); } }
        if protos.is_empty() { protos.push(c.protocol); }

        for (idx, pp) in protos.into_iter().enumerate() {
            if idx >= 4 { break; } // 单个节点最多尝试 4 个协议
            let mut sub = SubAttempt::new(&trace.trace_id, &c.node_id, &c.group_id);
            sub.masked_endpoint = masked.endpoint.clone();
            sub.masked_token   = masked.api_key.clone();
            sub.protocol = pp.to_string();

            match execute_one(c, &req, pp, &mut sub).await {
                Ok(resp) => return AttemptOutcome::Ok(resp, sub),
                Err(e) => {
                    let lbl = e.label();
                    let status = sub.http_status_code;
                    let mismatch = protocol_mismatch(&e);
                    let terminal = !mismatch && matches!(lbl, ErrorLabel::BadParam4xx | ErrorLabel::Auth401403);
                    sub.finish_fail(lbl, status, if terminal { SubAttemptOutcome::FailedTerminal } else { SubAttemptOutcome::FailedRetried });
                    subs.push(sub);
                    last_err = Some(e);
                    if mismatch {
                        continue; // 协议不支持 → 换下一个候选协议
                    }
                    if terminal { break 'outer; } // 客户端非法/鉴权失败 → 整链终止（不浪费重试配额）
                    break; // 其它（5xx/网络/解析）→ 交给下一个候选节点
                }
            }
        }
    }

    // 走到这里 = 没一个节点成功 → AllFailed，拼 human_reason
    let hr = build_human_reason(trace, &subs, &policy, &RecentWindowStats::default());
    let _act = last_err.as_ref().map(|e| crate::flex_adapter::retry_analysis::triage_action(e.label()));
    AttemptOutcome::AllFailed(subs, last_err.unwrap_or_else(|| AppError::Labeled {
        label: ErrorLabel::Unknown, message: "attempt exhausted without final error".into(),
    }), hr)
}
