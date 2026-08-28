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
use axum::response::{IntoResponse, Response};
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

/// 统一执行管线：限流 → trace → 缓存 → 路由 → 柔性层 → 账本钩子
/// （OpenAI 原生 handler 与入站协议适配 handler 共用；协议差异只在进出两端）
pub(crate) async fn execute_chat_pipeline(
    app: &Arc<AppState>,
    client: &AuthedClient,
    mut req: ChatCompletionRequest,
) -> Result<ChatPipelineOutput, AppErrorResponse> {
    // -- 1) RPM/TPM 校验（纯校验，不通过立即 429）--
    let now = crate::observability::trace::now_ms();
    let c = &client.entity;
    if let Err(reason) = app.per_key_rate_limits.check_pass(&client.key_hash, c.rpm, c.tpm, 0, now) {
        return Err(AppError::Labeled {
            label: ErrorLabel::Http429,
            message: format!("rate limit exceeded: {reason}"),
        }.into());
    }

    // -- 2) Trace 初始化（明文 key 绝不入 trace）--
    let mut trace = GatewayTrace::new(req.model.clone(), "");
    trace.client_key_hash = client.key_hash.clone();
    trace.client_key_name = client.key_name.clone();
    trace.is_stream = req.stream;

    // -- 3) 非流式缓存查询 --
    if !trace.is_stream {
        if let Some(k) = crate::cache_pool::cache_key::hash_request(&req) {
            if let Ok(Some(entry)) = crate::cache_pool::try_get_non_stream(app, &k).await {
                trace.is_cached = true;
                trace.billed_usage = entry.billed_usage.clone();
                trace.close(200, None);
                let v: Value = serde_json::from_slice(&entry.response_json).unwrap_or(Value::Null);
                return Ok(ChatPipelineOutput::Json(v, trace));
            }
        }
    }

    // -- 4) 路由 + 柔性层执行 --
    let candidates = crate::router::route_client_request(app, &req.model, trace.is_stream).await
        .map_err(AppErrorResponse::from)?;
    if candidates.is_empty() {
        return Err(AppError::Labeled {
            label: ErrorLabel::BadParam4xx,
            message: "no routing target for model alias or all nodes disabled".into(),
        }.into());
    }
    if let Some(first) = candidates.first() {
        trace.resolved_model = first.real_model.clone();
        trace.node_group = first.group_id.clone();
        req.model = first.real_model.clone();
    }

    if trace.is_stream {
        // -- 5a) 流式：锁定首个候选，直接 SSE chunk 流（永不重试）--
        let trace_id = trace.trace_id.clone();
        let first = candidates.into_iter().next().unwrap();
        let stream = crate::flex_adapter::execute_stream(app, trace, first, req).await.map_err(AppErrorResponse::from)?;
        Ok(ChatPipelineOutput::Sse(stream, trace_id))
    } else {
        // -- 5b) 非流式 --
        let resp: ChatCompletionResponse = crate::flex_adapter::execute_non_stream(app, &mut trace, &candidates, req).await.map_err(AppErrorResponse::from)?;
        let usage = resp.usage.clone().unwrap_or_default();
        trace.billed_usage = UsageSnapshot {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        };
        trace.close(200, None);
        crate::services::trace_store::record(app, &trace).await;
        let v = serde_json::to_value(&resp).unwrap_or(Value::Null);
        Ok(ChatPipelineOutput::Json(v, trace))
    }
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

/// GET /v1/models —— 返回 alias + 后端真实模型合并列表
pub async fn list_models_handler(
    State(app): State<Arc<AppState>>,
    _client: AuthedClient,
) -> Result<Json<ModelList>, AppErrorResponse> {
    let cfg = app.cfg_swap.load();
    let mut data = Vec::with_capacity(cfg.model_aliases.len());
    for a in cfg.model_aliases.iter().filter(|a| a.enabled) {
        data.push(ModelItem {
            id: a.alias.clone(),
            object: "model",
            created: 1700000000i64,
            owned_by: "plocal-switch".into(),
        });
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
