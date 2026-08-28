//! =============================================================
//!  4. 硬编码适配器集合（13 类厂商原生协议双向转换）
//! =============================================================
//!  每个适配器实现 `trait BackendAdapter`：
//!    - `translate_request(&self, oai: ChatCompletionRequest) -> VendorRequest`
//!    - `parse_response(&self, vendor_resp) -> ChatCompletionResponse`   非流式
//!    - `translate_chunk(&self, vendor_bytes) -> SseChunk`              流式
//!    - `translate_toolcall`（双向）
//!    - `translate_multimodal`（images → 厂商 URL/base64 格式）
//!
//!  不在此硬编码里匹配成功的协议，走 flex_adapter.protocol_sniffer + flexible_parser 宽容模式。
//! =============================================================
pub mod openai_adapter;
pub mod responses_adapter;
pub mod anthropic_adapter;
pub mod gemini_adapter;
pub mod bedrock_adapter;
pub mod cohere_v2_adapter;
pub mod qianfan_adapter;
pub mod dashscope_adapter;
pub mod spark_adapter;
pub mod hunyuan_adapter;
pub mod ollama_adapter;
pub mod vllm_adapter;
pub mod tgi_adapter;
pub mod custom_openai_compat_adapter;

use async_trait::async_trait;
use crate::error::AppResult;
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, SseChunk};

#[async_trait]
pub trait BackendAdapter: Send + Sync {
    fn protocol(&self) -> crate::router::ProtocolKind;
    async fn translate_request(&self, oai: &ChatCompletionRequest, node: &crate::router::CandidateNode) -> AppResult<reqwest::RequestBuilder>;
    fn parse_response_body(&self, bytes: bytes::Bytes) -> AppResult<ChatCompletionResponse>;
    fn translate_sse_chunk(&self, vendor_line: &str) -> AppResult<Option<SseChunk>>;
}

/// 上游代理配置（地区受限网络必须走代理才能访问某些上游）。
#[derive(Clone)]
pub struct UpstreamProxy { pub url: String, pub no_proxy: Vec<String> }

/// 当前生效的上游代理。使用 RwLock 以便设置页在运行时随时切换并实时生效。
static UPSTREAM_PROXY: std::sync::RwLock<Option<UpstreamProxy>> = std::sync::RwLock::new(None);

/// 基础构建（无代理），首次用 None；后续代理变更时重建。
fn build_client(proxy: Option<&UpstreamProxy>) -> reqwest::Client {
    let mut b = reqwest::Client::builder()
        .use_rustls_tls()
        .connect_timeout(std::time::Duration::from_secs(10))
        .read_timeout(std::time::Duration::from_secs(120));
    if let Some(p) = proxy {
        if let Ok(pr) = reqwest::Proxy::all(&p.url) {
            b = b.proxy(pr);
        }
    }
    b.build().expect("failed to build gateway http client")
}

/// 网关统一上游 HTTP 客户端（共享连接池）。代理变更时整体重建。
static GATEWAY_HTTP: once_cell::sync::Lazy<std::sync::RwLock<reqwest::Client>> =
    once_cell::sync::Lazy::new(|| std::sync::RwLock::new(build_client(None)));

/// 应用代理配置：enable=false 时不代理；http 优先，否则 socks。更新并重建 client。
pub fn apply_upstream_proxy(enabled: bool, http: Option<String>, socks: Option<String>, no_proxy: Vec<String>) {
    let url = if enabled {
        http.filter(|s| !s.is_empty()).or_else(|| socks.filter(|s| !s.is_empty()))
    } else { None };
    let p = url.map(|u| UpstreamProxy { url: u, no_proxy });
    *UPSTREAM_PROXY.write().unwrap() = p.clone();
    *GATEWAY_HTTP.write().unwrap() = build_client(p.as_ref());
}

/// 获取共享的 reqwest::Client（适配器构建 RequestBuilder 时使用）。
/// 返回 clone，共享底层连接池，可随代理变更整体重建。
pub fn http_client() -> reqwest::Client {
    GATEWAY_HTTP.read().unwrap().clone()
}

/// Ollama 专用上游客户端：公开 Ollama 常为自签名证书，故放宽证书校验（仅用于 Ollama 节点）。
static OLLAMA_HTTP: once_cell::sync::Lazy<reqwest::Client> = once_cell::sync::Lazy::new(|| {
    reqwest::Client::builder()
        .use_rustls_tls()
        .danger_accept_invalid_certs(true)
        .connect_timeout(std::time::Duration::from_secs(10))
        .read_timeout(std::time::Duration::from_secs(120))
        .build()
        .expect("failed to build ollama http client")
});

/// 获取 Ollama 专用的 reqwest::Client（接受自签名证书）
pub fn ollama_http_client() -> &'static reqwest::Client {
    &OLLAMA_HTTP
}

/// 根据 ProtocolKind 派发实例（共享无状态）
pub fn adapter_for(kind: crate::router::ProtocolKind) -> Box<dyn BackendAdapter> {
    use crate::router::ProtocolKind::*;
    match kind {
        OpenAI             => Box::new(openai_adapter::OpenAIAdapter),
        OpenAIResponse     => Box::new(responses_adapter::ResponsesAdapter::default()),
        Anthropic          => Box::new(anthropic_adapter::AnthropicAdapter::default()),
        Gemini             => Box::new(gemini_adapter::GeminiAdapter::default()),
        BedrockConverse    => Box::new(bedrock_adapter::BedrockConverseAdapter),
        CohereV2           => Box::new(cohere_v2_adapter::CohereV2Adapter),
        Qianfan            => Box::new(qianfan_adapter::QianfanAdapter),
        DashScope          => Box::new(dashscope_adapter::DashScopeAdapter),
        Spark              => Box::new(spark_adapter::SparkAdapter),
        Hunyuan            => Box::new(hunyuan_adapter::HunyuanAdapter),
        Ollama             => Box::new(ollama_adapter::OllamaAdapter),
        Vllm               => Box::new(vllm_adapter::VllmAdapter),
        Tgi                => Box::new(tgi_adapter::TgiAdapter),
        CustomOpenAICompat => Box::new(custom_openai_compat_adapter::CustomCompatAdapter),
    }
}
