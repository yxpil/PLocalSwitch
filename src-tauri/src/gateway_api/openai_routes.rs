//! =============================================================
//!  交付物 7：/v1/... OpenAI v1 兼容路由（axum handler 签名）
//!    POST   /v1/chat/completions  → chat_completions_handler
//!    GET    /v1/models            → list_models_handler
//!    POST   /v1/embeddings        → embeddings_handler
//!  gw8a：执行管线抽取为 execute_chat_pipeline，供入站协议适配层
//!        （inbound_routes：Anthropic / Gemini / 自动嗅探）复用同一链路。
//! =============================================================
use crate::error::{AppError, ErrorLabel};
use crate::gateway_api::auth::AuthedClient;
use crate::gateway_api::error_resp::AppErrorResponse;
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse, ModelItem, ModelList, SseChunk};
use futures::{Stream, StreamExt};
use std::pin::Pin;
use crate::observability::trace::{GatewayTrace, UsageSnapshot};
use crate::state::AppState;
use axum::body::Body;
use axum::extract::State;
use axum::response::{Response};
use axum::Json;
use serde_json::Value;
use std::sync::Arc;

pub fn router(_app: Arc<AppState>) -> axum::Router<Arc<AppState>> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/chat/completions", post(chat_completions_handler))
        .route("/models",            get(list_models_handler))
        .route("/embeddings",        post(embeddings_handler))
}

/// 管线输出：非流式 JSON（OpenAI 形）或 SSE chunk 流 + trace_id
pub(crate) enum ChatPipelineOutput {
    Json(Value, GatewayTrace),
    Sse(Pin<Box<dyn Stream<Item = Result<SseChunk, String>> + Send + 'static>>, String),
}

/// 失败时落库 trace（含 5xx/429/超时等），保证成功率统计真实反映失败请求。
/// 返回透传给客户端的错误响应。
async fn persist_failure(app: &Arc<AppState>, trace: &mut GatewayTrace, e: AppError) -> AppErrorResponse {
    let label = e.label();
    trace.final_error_label = Some(label);
    let status = match label {
        ErrorLabel::BadParam4xx => 400,
        ErrorLabel::Auth401403 => 401,
        ErrorLabel::Http429 | ErrorLabel::Http413 => 413,
        ErrorLabel::Upstream5xx => 502,
        ErrorLabel::NetworkConnectRefused | ErrorLabel::DnsFail | ErrorLabel::TlsError => 503,
        ErrorLabel::ConnectTimeout | ErrorLabel::ReadTimeout => 504,
        _ => 500,
    };
    trace.close(status, Some(label));
    // 记录失败 trace（若要计入成功率分母）
    crate::services::trace_store::record(app, trace).await;
    // 错误日志工具：路由失败 / 非流式候选链失败统一落库（入库前自动凭证打码）
    crate::services::error_logger::record(
        app,
        if trace.is_stream { "stream" } else { "non_stream" },
        &label.to_string(),
        &e.to_string(),
        &trace.trace_id,
    ).await;
    AppErrorResponse(e)
}

/// 统一执行管线：限流 → trace → 缓存 → 路由 → 柔性层 → 账本钩子
/// （OpenAI 原生 handler 与入站协议适配 handler 共用；协议差异只在进出两端）
pub(crate) async fn execute_chat_pipeline(
    app: &Arc<AppState>,
    client: &AuthedClient,
    req: ChatCompletionRequest,
) -> Result<ChatPipelineOutput, AppErrorResponse> {
    // -- 1) Trace 初始化（明文 key 绝不入 trace）--
    // v0.2.27：trace 初始化提到限流之前 —— 429 限流拒绝的请求也要落库，
    // 否则链路追踪/对账统计漏掉被拒请求（此前是捕获盲区）
    let mut trace = GatewayTrace::new(req.model.clone(), "");
    trace.client_key_hash = client.key_hash.clone();
    trace.client_key_name = client.key_name.clone();
    trace.is_stream = req.stream;

    // -- 2) RPM/TPM 校验（不通过立即 429，同样落 trace/账本/错误日志）--
    let now = crate::observability::trace::now_ms();
    let c = &client.entity;
    if let Err(reason) = app.per_key_rate_limits.check_pass(&client.key_hash, c.rpm, c.tpm, 0, now) {
        let e = AppError::Labeled {
            label: ErrorLabel::Http429,
            message: format!("rate limit exceeded: {reason}"),
        };
        return Err(persist_failure(app, &mut trace, e).await);
    }

    // -- 3) 缓存（v0.2.26 全量接线）：tool/图片请求 hash 返回 None 自动跳过 --
    let cache_cfg = app.cfg_swap.load().cache_pool.clone();
    let cache_key_opt = if cache_cfg.enabled { crate::cache_pool::cache_key::hash_request(&req) } else { None };
    let cache_ttl_ms = Some(cache_cfg.default_ttl_seconds.saturating_mul(1000));
    let cache_model = req.model.clone();

    // -- 3a) 非流式缓存命中 → 直接本地返回，不打上游 --
    if !trace.is_stream {
        if let Some(k) = &cache_key_opt {
            if let Ok(Some(entry)) = crate::cache_pool::try_get_non_stream(app, k).await {
                trace.is_cached = true;
                trace.billed_usage = entry.billed_usage.clone();
                trace.close(200, None);
                // v0.2.27 修复：此前命中不落库，链路追踪/对账漏记缓存命中请求
                crate::services::trace_store::record(app, &trace).await;
                let v: Value = serde_json::from_slice(&entry.response_json).unwrap_or(Value::Null);
                tracing::info!(target: "cache", trace_id = %trace.trace_id, "cache hit (non_stream)");
                return Ok(ChatPipelineOutput::Json(v, trace));
            }
        }
    }

    // -- 4) 路由 + 柔性层执行 --
    let candidates = match crate::router::route_client_request(app, &req.model, trace.is_stream).await {
        Ok(c) => c,
        Err(e) => return Err(persist_failure(app, &mut trace, e).await),
    };
    if candidates.is_empty() {
        let e = AppError::Labeled {
            label: ErrorLabel::BadParam4xx,
            message: "no routing target for model alias or all nodes disabled".into(),
        };
        return Err(persist_failure(app, &mut trace, e).await);
    }
    if let Some(first) = candidates.first() {
        trace.resolved_model = first.real_model.clone();
        trace.node_group = first.group_id.clone();
    }

    if trace.is_stream {
        // -- 5a) 流式：响应头阶段按候选链逐个尝试（未输出字节前可换候选），成功才建流 --

        // 流式缓存命中 → 重放缓存的 chunk 序列（不建流、不打上游）
        if let Some(k) = &cache_key_opt {
            if let Ok(Some(entry)) = crate::cache_pool::try_get_stream(app, k).await {
                let sse = entry.sse_bytes.clone();
                if let Ok(chunks) = serde_json::from_slice::<Vec<SseChunk>>(&sse) {
                    if !chunks.is_empty() {
                        trace.is_cached = true;
                        trace.billed_usage = entry.billed_usage.clone();
                        trace.close(200, None);
                        crate::services::trace_store::record(app, &trace).await;
                        let tid = trace.trace_id.clone();
                        tracing::info!(target: "cache", trace_id = %tid, "cache hit (stream)");
                        let replay = futures::stream::iter(
                            chunks.into_iter().map(Ok::<SseChunk, String>)
                        );
                        return Ok(ChatPipelineOutput::Sse(Box::pin(replay), tid));
                    }
                }
            }
        }

        let trace_id = trace.trace_id.clone();
        let stream = match crate::flex_adapter::execute_stream(app, trace, &candidates, req).await {
            Ok(s) => s,
            Err(e) => {
                // execute_stream 内部已落库其自身 trace；这里仅透传错误
                return Err(AppErrorResponse(e));
            }
        };
        // v0.2.26 流式缓存写入：旁路捕获成功输出的 chunk，流完整结束后落缓存（不改变转发行为、不重试）
        if let Some(k) = cache_key_opt {
            let stream = capture_stream_for_cache(app.clone(), k, cache_ttl_ms, cache_model, stream);
            return Ok(ChatPipelineOutput::Sse(Box::pin(stream), trace_id));
        }
        return Ok(ChatPipelineOutput::Sse(stream, trace_id));
    } else {
        // -- 5b) 非流式 --
        let resp: ChatCompletionResponse = match crate::flex_adapter::execute_non_stream(app, &mut trace, &candidates, req).await {
            Ok(r) => r,
            Err(e) => return Err(persist_failure(app, &mut trace, e).await),
        };
        let usage = resp.usage.clone().unwrap_or_default();
        trace.billed_usage = UsageSnapshot {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        };
        trace.close(200, None);
        crate::services::trace_store::record(app, &trace).await;
        // v0.2.26 非流式缓存写入：成功响应落缓存，重复请求下次本地直返
        if let Some(k) = cache_key_opt {
            if let Ok(bytes) = serde_json::to_vec(&resp) {
                if bytes.len() <= 4 * 1024 * 1024 { // 单条缓存硬上限 4MB，防内存膨胀
                    let entry = crate::cache_pool::cache_entry::NonStreamEntry::new(
                        k, cache_model, cache_ttl_ms, bytes, false, trace.billed_usage.clone());
                    let _ = crate::cache_pool::put_non_stream(app, k, entry).await;
                }
            }
        }
        let v = serde_json::to_value(&resp).unwrap_or(Value::Null);
        Ok(ChatPipelineOutput::Json(v, trace))
    }
}

/// 流式缓存旁路捕获：转发行为不变，仅把成功输出的 chunk 克隆进缓冲，
/// 流完整结束（无错误项）后落缓存；中断/出错的流一律不缓存。
/// chunk 数上限 2048、序列化后 4MB 上限，双重防止内存膨胀。
fn capture_stream_for_cache(
    app: Arc<AppState>,
    key: u128,
    ttl_ms: Option<u64>,
    model: String,
    stream: Pin<Box<dyn Stream<Item = Result<SseChunk, String>> + Send>>,
) -> Pin<Box<dyn Stream<Item = Result<SseChunk, String>> + Send + 'static>> {
    let buf: Arc<std::sync::Mutex<Vec<SseChunk>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let failed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (b1, f1) = (buf.clone(), failed.clone());
    let captured = stream.map(move |item| {
        match &item {
            Ok(c) => {
                if let Ok(mut b) = b1.lock() {
                    if b.len() < 2048 { b.push(c.clone()); }
                }
            }
            Err(_) => { f1.store(true, std::sync::atomic::Ordering::Relaxed); }
        }
        item
    });
    let tail = futures::stream::once(async move {
        if !failed.load(std::sync::atomic::Ordering::Relaxed) {
            let chunks = buf.lock().map(|b| b.clone()).unwrap_or_default();
            if !chunks.is_empty() {
                if let Ok(bytes) = serde_json::to_vec(&chunks) {
                    if bytes.len() <= 4 * 1024 * 1024 {
                        // usage 取流末尾的 usage chunk（若上游有提供）
                        let u = chunks.iter().rev().find_map(|c| c.usage.clone());
                        let billed = UsageSnapshot {
                            prompt_tokens: u.as_ref().map(|x| x.prompt_tokens).unwrap_or(0),
                            completion_tokens: u.as_ref().map(|x| x.completion_tokens).unwrap_or(0),
                            total_tokens: u.as_ref().map(|x| x.total_tokens).unwrap_or(0),
                        };
                        let entry = crate::cache_pool::cache_entry::StreamEntry::new(
                            key, model, ttl_ms, bytes, false, billed);
                        let _ = crate::cache_pool::put_stream(&app, key, entry).await;
                    }
                }
            }
        }
    })
    // 尾部只做缓存落库，不向客户端输出任何额外 chunk
    .filter_map(|_: ()| async { None::<Result<SseChunk, String>> });
    Box::pin(captured.chain(tail))
}

/// POST /v1/chat/completions —— 核心业务 handler（OpenAI 原生格式直通）
pub async fn chat_completions_handler(
    State(app): State<Arc<AppState>>,
    client: AuthedClient,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<Response<Body>, AppErrorResponse> {
    let out = execute_chat_pipeline(&app, &client, req).await?;
    match out {
        ChatPipelineOutput::Json(v, trace) => {
            let body = serde_json::to_vec(&v).unwrap_or_default();
            Ok(Response::builder().status(200)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .header("x-pls-trace-id", trace.trace_id)
                .body(Body::from(body)).unwrap())
        }
        ChatPipelineOutput::Sse(stream, trace_id) => {
            // 把 SseChunk 序列化成标准 OpenAI SSE 文本流，最后补 data: [DONE]
            let body_stream = stream.map(|item| -> Result<bytes::Bytes, std::io::Error> {
                match item {
                    Ok(chunk) => {
                        let s = serde_json::to_string(&chunk).unwrap_or_else(|_| "{}".to_string());
                        Ok(bytes::Bytes::from(format!("data: {s}\n\n")))
                    }
                    Err(e) => Ok(bytes::Bytes::from(format!("data: [ERROR] {e}\n\n"))),
                }
            });
            let done = futures::stream::once(async move {
                Ok::<bytes::Bytes, std::io::Error>(bytes::Bytes::from("data: [DONE]\n\n"))
            });
            let body = Body::from_stream(body_stream.chain(done));
            Ok(Response::builder().status(200)
                .header(axum::http::header::CONTENT_TYPE, "text/event-stream")
                .header(axum::http::header::CACHE_CONTROL, "no-cache")
                .header("x-pls-trace-id", trace_id)
                .body(body).unwrap())
        }
    }
}

/// GET /v1/models —— 返回 alias + 模型目录真实模型合并列表（去重，下游设备可见全部可路由模型）
pub async fn list_models_handler(
    State(app): State<Arc<AppState>>,
    _client: AuthedClient,
) -> Result<Json<ModelList>, AppErrorResponse> {
    let cfg = app.cfg_swap.load();
    let mut seen = std::collections::HashSet::new();
    let mut data = Vec::new();
    // 1) 别名（客户端配置的对外模型名）
    for a in cfg.model_aliases.iter().filter(|a| a.enabled) {
        if seen.insert(a.alias.clone()) {
            data.push(ModelItem {
                id: a.alias.clone(),
                object: "model",
                created: 1700000000i64,
                owned_by: "plocal-switch".into(),
            });
        }
    }
    // 0) AUTOMODE 虚拟模型（设置开启时）：全目录自动尝试降级
    if app.cfg_swap.load().automode.enabled {
        data.push(ModelItem {
            id: "AUTOMODE".into(),
            object: "model",
            created: 1700000000i64,
            owned_by: "plocal-switch".into(),
        });
    }
    // 2) 模型目录（组合键 host|model）：对外展示纯模型名（多源同名去重，下游设备兼容）
    for entry in app.node_runtime.model_catalog.iter() {
        let name = entry.key().rsplit('|').next().unwrap_or(entry.key()).to_string();
        if seen.insert(name.clone()) {
            data.push(ModelItem {
                id: name,
                object: "model",
                created: 1700000000i64,
                owned_by: "upstream".into(),
            });
        }
    }
    Ok(Json(ModelList { object: "list", data }))
}

/// POST /v1/embeddings —— 占位 handler（柔性层 embed submodules 实现）
pub async fn embeddings_handler(
    State(_app): State<Arc<AppState>>,
    _client: AuthedClient,
    Json(_req): Json<EmbeddingRequest>,
) -> Result<Json<EmbeddingResponse>, AppErrorResponse> {
    Err(AppError::Labeled { label: ErrorLabel::Internal, message: "embeddings handler stub".into() }.into())
}
