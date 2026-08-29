//! =============================================================
//!  gw8a：入站协议适配路由（协议无感中转）
//! =============================================================
//!  路由：
//!    POST /v1/messages                              → 自动识别（路径偏 Anthropic + body 嗅探确认）
//!    POST /anthropic/v1/messages                    → 强制 Anthropic
//!    POST /gemini/v1beta/models/{*rest}             → 强制 Gemini（model 从 path 提取）
//!    POST /v1/{*rest}（未知子路径兜底）              → 纯 body 嗅探
//!  全部复用 openai_routes::execute_chat_pipeline（同一 限流/缓存/路由/柔性/计费 链路），
//!  仅在入口 normalize_request、出口 denormalize_response。
//! =============================================================
use crate::error::AppError;
use crate::gateway_api::auth::AuthedClient;
use crate::gateway_api::error_resp::AppErrorResponse;
use crate::gateway_api::inbound_sniffer::{self, InboundProtocol};
use crate::gateway_api::openai_routes::{execute_chat_pipeline, ChatPipelineOutput};
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use futures::StreamExt;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;
use std::sync::Arc;

/// 统一入口：给一个 InboundProtocol + body + model_hint → 归一化 → 管线 → 反归一化
async fn run_inbound(
    app: Arc<AppState>,
    client: AuthedClient,
    proto_forced: Option<InboundProtocol>,
    body: Value,
    model_hint: Option<String>,
) -> Result<Response<Body>, AppErrorResponse> {
    // 1) 协议判定：强制 > 路径 > 嗅探
    let proto = proto_forced.unwrap_or_else(|| inbound_sniffer::sniff_body(&body).0);

    // 2) 归一化（协议错误 → 400，带客户端协议形错误体由 error_resp 统一渲染）
    let req = inbound_sniffer::normalize_request(proto, &body, model_hint.as_deref())
        .map_err(AppErrorResponse::from)?;

    // 3) 同一条管线（限流/trace/缓存/路由/柔性/账本）
    let out = execute_chat_pipeline(&app, &client, req).await?;

    match out {
        ChatPipelineOutput::Json(v, _trace) => {
            let resp = inbound_sniffer::denormalize_response(proto, &v, model_hint.as_deref().unwrap_or(""));
            Ok((StatusCode::OK, Json(resp)).into_response())
        }
        ChatPipelineOutput::Sse(stream, _tid) => {
            // 流式：与 OpenAI 原生路由一致，把 SseChunk 序列化写回 SSE 文本流
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
                .body(body).unwrap())
        }
    }
}

/// POST /v1/messages —— Anthropic SDK 标准路径（自动识别，允许 OpenAI 格式打进同一入口）
pub async fn messages_auto_handler(
    State(app): State<Arc<AppState>>,
    client: AuthedClient,
    Json(body): Json<Value>,
) -> Result<Response<Body>, AppErrorResponse> {
    run_inbound(app, client, Some(InboundProtocol::Anthropic), body, None).await
}

/// POST /anthropic/v1/messages —— 强制 Anthropic
pub async fn anthropic_messages_handler(
    State(app): State<Arc<AppState>>,
    client: AuthedClient,
    Json(body): Json<Value>,
) -> Result<Response<Body>, AppErrorResponse> {
    run_inbound(app, client, Some(InboundProtocol::Anthropic), body, None).await
}

/// POST /gemini/v1beta/models/{model}:generateContent —— 强制 Gemini
pub async fn gemini_generate_handler(
    State(app): State<Arc<AppState>>,
    client: AuthedClient,
    Path(rest): Path<String>,
    Json(body): Json<Value>,
) -> Result<Response<Body>, AppErrorResponse> {
    // rest = "{model}:generateContent"（也可能是 streamGenerateContent —— stub 同样处理非流式）
    let model = rest.rsplit_once(':').map(|(m, _)| m.to_string()).unwrap_or(rest);
    run_inbound(app, client, Some(InboundProtocol::Gemini), body, Some(model)).await
}

/// POST /api/chat —— Ollama 原生客户端路径（自动识别）
pub async fn ollama_chat_handler(
    State(app): State<Arc<AppState>>,
    client: AuthedClient,
    Json(body): Json<Value>,
) -> Result<Response<Body>, AppErrorResponse> {
    run_inbound(app, client, Some(InboundProtocol::Ollama), body, None).await
}

/// POST /v1/{*rest} —— 未知子路径兜底：纯 body 嗅探（"奇奇怪怪的请求"自动匹配入口）
pub async fn v1_catchall_handler(
    State(app): State<Arc<AppState>>,
    client: AuthedClient,
    Json(body): Json<Value>,
) -> Result<Response<Body>, AppErrorResponse> {
    run_inbound(app, client, None, body, None).await
}

/// /v1/{*rest} 兜底路由注册（axum 静态路由优先于 wildcard，/chat/completions 等不受影响）
pub fn router() -> axum::Router<Arc<AppState>> {
    use axum::routing::post;
    axum::Router::new()
        .route("/messages", post(messages_auto_handler))
        .route("/*rest", post(v1_catchall_handler))
        // 兜底兜不到已知静态路径，但会把 /v1/chat/completions 之外的一切 POST 吸进来
}

/// 根级入站路由（anthropic / gemini / ollama 专用前缀）
pub fn root_router() -> axum::Router<Arc<AppState>> {
    use axum::routing::post;
    axum::Router::new()
        .route("/anthropic/v1/messages", post(anthropic_messages_handler))
        .route("/gemini/v1beta/models/*rest", post(gemini_generate_handler))
        .route("/api/chat", post(ollama_chat_handler))
        .route("/api/generate", post(ollama_chat_handler))
}

/// 错误体辅助（供 error_resp 未来按客户端协议渲染扩展；当前保留 OpenAI 形）
#[allow(dead_code)]
fn proto_error(proto: InboundProtocol, openai_err: &Value) -> Value {
    inbound_sniffer::error_body(proto, openai_err)
}

#[allow(dead_code)]
fn _unused_app_error(e: AppError) { let _ = e; }
