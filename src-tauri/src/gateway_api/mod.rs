//! =============================================================
//!  对外 HTTP 入口（axum Router 组装）—— 交付物 7 的真实 Rust 骨架
//!  两条子路由：
//!    • /v1/...           OpenAI v1 100% 兼容（chat/models/embeddings）
//!    • /manage/...       内部管理接口（账单 / trace / 节点质量 / 对账报表 / sub_attempt 详情）
//!    • /metrics          Prometheus 拉取
//!  全局 tower 层：CORS、大小限制、请求超时、并发限流、RequestId、catch_panic、SensitiveHeaders
//! =============================================================
pub mod auth;              // 网关自有 API Key 校验 + client_key 速率/余额/并发前置
pub mod rate_limit;        // RPM/TPM 限流（基于 dashmap + 滑动窗口）
pub mod openai_routes;     // /v1/chat/completions、/v1/models、/v1/embeddings 处理
pub mod inbound_sniffer;   // gw8a 入站协议嗅探 + 归一化/反归一化（OpenAI/Anthropic/Gemini）
pub mod inbound_routes;    // gw8a 入站协议路由（/v1/messages、/anthropic/...、/gemini/...、/v1/* 兜底）
pub mod manage_routes;     // 管理接口（账单 / Trace / 节点 / 对账 / 单条 sub_attempt）
pub mod sse_utils;         // SSE chunk 归一化（流式输出序列化 + event/data:前缀）
pub mod error_resp;        // axum 错误统一映射为 OpenAI 标准错误响应（严格不泄露上游地址）

use crate::error::AppResult;
use crate::state::AppState;
use axum::extract::State;
use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::sensitive_headers::{SetSensitiveRequestHeadersLayer, SetSensitiveResponseHeadersLayer};
use tower::limit::ConcurrencyLimitLayer;
// gw: 构造 CORS 层（按配置）：allow_credentials=true 时禁止 "Any"，改用 origins 白名单
fn build_cors_layer(cfg: &crate::config::CorsConfig) -> CorsLayer {
    let methods: Vec<axum::http::Method> = cfg
        .allow_methods
        .iter()
        .filter_map(|m| m.parse().ok())
        .collect();
    let mut allowed_headers: Vec<axum::http::HeaderName> = if cfg.allow_headers.is_empty() {
        vec![
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderName::from_static("x-manage-token"),
        ]
    } else {
        cfg.allow_headers.iter().filter_map(|h| h.parse().ok()).collect()
    };
    // 本机托盘菜单 HTTP 控制需要自定义头 x-manage-token，始终放行
    if !allowed_headers.iter().any(|h| h.as_str().eq_ignore_ascii_case("x-manage-token")) {
        allowed_headers.push(axum::http::HeaderName::from_static("x-manage-token"));
    }

    let has_wildcard_origin = cfg.allow_origins.iter().any(|o| o == "*" || o == "any");

    if cfg.allow_credentials && !has_wildcard_origin {
        // 允许携带凭证（cookie/Authorization）：必须精确匹配请求 Origin，不能用通配 *
        let origins = cfg.allow_origins.clone();
        let origin_layer = AllowOrigin::predicate(move |origin: &axum::http::HeaderValue, _req: &axum::http::request::Parts| {
            let s = origin.to_str().unwrap_or("");
            origins.iter().any(|o| o.eq_ignore_ascii_case(s))
        });
        CorsLayer::new()
            .allow_origin(origin_layer)
            .allow_methods(if methods.is_empty() {
                Vec::<axum::http::Method>::from([axum::http::Method::GET, axum::http::Method::POST, axum::http::Method::OPTIONS])
            } else { methods })
            .allow_headers(allowed_headers)
            .allow_credentials(true)
    } else {
        // 常规模式：origins 含 * 或未开启 credentials → Any（宽松，桌面壳/前端开发允许跨域）
        let mut l = CorsLayer::new().allow_origin(AllowOrigin::any());
        if !methods.is_empty() { l = l.allow_methods(methods); }
        l = l.allow_headers(allowed_headers);
        l
    }
}

/// 根路径着陆页 —— 访问网关根地址时，展示“这是你自己的网关 + 完整使用方法”。
/// 黑白极简，自包含（内联 CSS），不依赖任何外部资源。
fn landing_html(listen: &str, base: &str) -> String {
    format!(r#"<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>PLocalSwitch Gateway · 使用说明</title>
<style>
  :root{{color-scheme:light dark}}
  *{{box-sizing:border-box;margin:0;padding:0}}
  body{{min-height:100vh;display:flex;align-items:center;justify-content:center;padding:24px;
       font-family:-apple-system,BlinkMacSystemFont,"Segoe UI","PingFang SC","Microsoft YaHei",sans-serif;
       background:#0a0a0a;color:#fafafa}}
  .card{{max-width:560px;width:100%;background:#111;border:1px solid #2a2a2a;border-radius:24px;
        padding:36px 34px;box-shadow:0 30px 80px -30px rgba(0,0,0,.8)}}
  .logo{{width:56px;height:56px;border-radius:14px;background:#fafafa;display:flex;align-items:center;justify-content:center;
        margin-bottom:20px;font-weight:900;color:#0a0a0a;font-size:20px}}
  h1{{font-size:21px;font-weight:800;letter-spacing:.2px}}
  .badge{{display:inline-flex;align-items:center;gap:6px;margin-top:8px;padding:4px 12px;border-radius:999px;
         font-size:12px;background:rgba(255,255,255,.08);border:1px solid rgba(255,255,255,.14)}}
  .dot{{width:8px;height:8px;border-radius:50%;background:#2dd4a7;box-shadow:0 0 10px #2dd4a7}}
  p.desc{{margin-top:16px;font-size:14px;line-height:1.8;color:#9a9a9a}}
  .addr{{margin-top:14px;padding:12px 14px;border-radius:12px;background:#0a0a0a;border:1px solid #262626;
        font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:13px;color:#eaeaea;word-break:break-all}}
  .hint{{margin-top:12px;font-size:12px;color:#6a6a6a;line-height:1.7}}
  .steps{{margin-top:20px}}
  .step-t{{font-size:12px;font-weight:700;letter-spacing:.6px;color:#6a6a6a;text-transform:uppercase;margin-bottom:10px}}
  ol{{padding-left:18px}} ul{{padding-left:18px}}
  li{{font-size:13.5px;line-height:1.9;color:#c9c9c9;margin-bottom:4px}}
  li b{{color:#fafafa}}
  code{{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12.5px;color:#eaeaea;
       background:rgba(255,255,255,.06);padding:2px 6px;border-radius:6px}}
  pre{{margin-top:10px;padding:12px 14px;border-radius:12px;background:#0a0a0a;border:1px solid #262626;overflow:auto;
       font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12px;color:#9fe08f;line-height:1.7}}
</style></head><body>
<div class="card">
  <div class="logo">PLS</div>
  <h1>PLocalSwitch Gateway</h1>
  <div class="badge"><span class="dot"></span>正在运行 · Working</div>
  <p class="desc">这是<b>你自己的</b>本地 LLM 中转网关（OpenAI v1 兼容）。把下游客户端的 Base URL 指到下面即可使用。</p>
  <div class="addr">{base}</div>
   <p class="hint">监听网卡 {listen}（<code>0.0.0.0</code> 是通配地址，浏览器请用 <b>{base}</b> 访问）。</p>

  <div class="steps">
    <div class="step-t">使用步骤</div>
    <ol>
      <li>在桌面 App 的「模型与路由」填加你的<b>上游网关</b>（OpenAI / Anthropic / Gemini 自动识别）。</li>
      <li>在「网关配置」中添加一个<b>Client Key</b>，作为下游调用你的令牌。</li>
      <li>把客户端 Base URL 设为 <code>{base}/v1</code>，用上面的 Client Key 鉴权即可。</li>
    </ol>
  </div>

  <div class="steps">
    <div class="step-t">常用接口</div>
    <ul>
      <li><code>GET  /v1/models</code> 模型列表</li>
      <li><code>POST /v1/chat/completions</code> Chat（流式 / 非流式）</li>
      <li><code>POST /v1/embeddings</code> 向量</li>
      <li><code>GET  /metrics</code> Prometheus 指标</li>
    </ul>
  </div>

  <div class="steps">
    <div class="step-t">调用示例</div>
    <pre>curl {base}/v1/chat/completions \
  -H "Authorization: Bearer &lt;CLIENT_KEY&gt;" \
  -H "Content-Type: application/json" \
  -d '{{...}}'</pre>
  </div>
</div>
</body></html>"#)
}

/// 404 页（浏览器访问不存在的路径时返回的 HTML）
fn notfound_html(path: &str) -> String {
    format!(r#"<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>404 · PLocalSwitch Gateway</title>
<style>
  :root{{color-scheme:light dark}}
  *{{box-sizing:border-box;margin:0;padding:0}}
  body{{min-height:100vh;display:flex;align-items:center;justify-content:center;padding:24px;
       font-family:-apple-system,BlinkMacSystemFont,"Segoe UI","PingFang SC","Microsoft YaHei",sans-serif;
       background:#0a0a0a;color:#fafafa}}
  .card{{max-width:520px;width:100%;background:#111;border:1px solid #2a2a2a;border-radius:24px;
        padding:40px 36px;box-shadow:0 30px 80px -30px rgba(0,0,0,.8)}}
  .code{{font-size:64px;font-weight:900;letter-spacing:-2px}}
  h1{{font-size:20px;font-weight:800;margin-top:8px}}
  p.desc{{margin-top:16px;font-size:14px;line-height:1.8;color:#9a9a9a}}
  .addr{{margin-top:18px;padding:12px 14px;border-radius:12px;background:#0a0a0a;border:1px solid #262626;
        font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:13px;color:#eaeaea;word-break:break-all}}
</style></head><body>
<div class="card">
  <div class="code">404</div>
  <h1>网关找不到这个路径</h1>
  <p class="desc">这是一个正常的网关 404 提示：你访问的路径不存在。请使用 <code>/v1/chat/completions</code>、<code>/v1/models</code> 等合法接口，并携带已配置的 Client Key。</p>
  <div class="addr">GET {path} → NotFound</div>
</div>
</body></html>"#)
}

/// GET / → 网关状态着陆页
async fn root_landing_handler(
    State(app): State<Arc<AppState>>,
) -> axum::response::Response {
    let listen = app.cfg_swap.load().http.listen.clone();
    let port = listen.rsplit(':').next().unwrap_or("8787").to_string();
    let base_local = format!("http://127.0.0.1:{}", port);
    let html = landing_html(&listen, &base_local);
    axum::response::Response::builder()
        .status(200)
        .header(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(axum::body::Body::from(html))
        .unwrap()
}

/// fallback → 浏览器返回 HTML 404，API 客户端返回 JSON（OpenAI 风格）
async fn not_found_handler(
    State(_app): State<Arc<AppState>>,
    req: axum::extract::Request,
) -> axum::response::Response {
    let accepts_html = req
        .headers()
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("text/html"))
        .unwrap_or(false);
    if accepts_html {
        let path = req.uri().path().to_string();
        axum::response::Response::builder()
            .status(404)
            .header(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(axum::body::Body::from(notfound_html(&path)))
            .unwrap()
    } else {
        let body = serde_json::json!({
            "error": {
                "message": "the requested path does not exist",
                "type": "invalid_request_error",
                "code": "not_found",
            }
        })
        .to_string();
        axum::response::Response::builder()
            .status(404)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body))
            .unwrap()
    }
}

/// 构造 axum Router：请求入口
///
/// 路由层次：
///   `POST   /v1/chat/completions`  非流式/流式 Chat（核心）
///   `POST   /v1/embeddings`        Embedding
///   `GET    /v1/models`            模型列表（真实后端模型 + 别名合并）
///   `GET    /manage/billing/...`   账本 / 费率 / client_key
///   `GET    /manage/traces/...`    Trace 链路 / 单条 sub_attempt
///   `GET    /manage/nodes/...`     节点质量 / 上游指纹 / 配置节点
///   `GET    /manage/audit/...`     分词对账报表
///   `GET    /metrics`              Prometheus
pub fn build_router(app: Arc<AppState>) -> Router {
    let openai_api = openai_routes::router(app.clone());
    let inbound_api = inbound_routes::router();
    let inbound_root = inbound_routes::root_router();
    let manage_api = manage_routes::router(app.clone());
    let metrics_handler = get(observability_prometheus_handler);

    let concurrency_limit = app.cfg.http.global_concurrency_limit;
    let body_limit        = app.cfg.http.request_body_max_bytes;

    // gw: CORS 从 gateway.yaml cors 段读取（支持 allow_credentials；origins 含 * 时回退 Any）
    let cors_layer   = build_cors_layer(&app.cfg_swap.load().cors);
    let sensitive_request = axum::http::HeaderMap::new(); // 在 auth 层单独 mask：Authorization / x-api-key / x-api-token

    Router::new()
        .nest("/v1",     openai_api.merge(inbound_api)) // /v1/* 静态优先，wildcard 兜底
        .nest("/manage", manage_api)
        .merge(inbound_root)                            // /anthropic/... /gemini/...
        .route("/metrics", metrics_handler)
        .route("/", get(root_landing_handler))          // 直接访问网关根地址时的状态首页
        .fallback(not_found_handler)                    // 其余路径（浏览器→HTML，API→JSON）
        .layer(ConcurrencyLimitLayer::new(concurrency_limit))
        .layer(RequestBodyLimitLayer::new(body_limit))
        .layer(cors_layer)
        .layer(SetSensitiveRequestHeadersLayer::new([
            axum::http::header::AUTHORIZATION,
            axum::http::header::COOKIE,
            "x-api-key".parse().unwrap(),
            "x-stainless-arch".parse().unwrap(),
        ]))
        .layer(SetSensitiveResponseHeadersLayer::new([
            "x-upstream-authorization".parse().unwrap(),
            "x-real-api-key".parse().unwrap(),
        ]))
        .with_state(app)
}

async fn observability_prometheus_handler(
    axum::extract::State(_s): axum::extract::State<Arc<AppState>>,
) -> &'static str {
    // TODO: 调用 crate::observability::prometheus_registry::render()
    "# prometheus metrics placeholder — wire with crate::observability\n"
}

/// 启动入口（由 safety_runtime::spawn_axum_server 调用）：绑定 TCP 并服务
pub async fn serve_forever(app: Arc<AppState>, shutdown: tokio::sync::oneshot::Receiver<()>) -> AppResult<()> {
    let router = build_router(app.clone());
    let listener = tokio::net::TcpListener::bind(&app.cfg.http.listen).await?;
    tracing::info!("axum gateway listening on {}", app.cfg.http.listen);
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown.await;
            tracing::warn!("axum graceful shutdown signal received");
        })
        .await?;
    Ok(())
}
