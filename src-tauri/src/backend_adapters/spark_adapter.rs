//! 讯飞星火 协议适配器 —— 星火提供 OpenAI 兼容端点（/v1/chat/completions），
//! 因此复用 OpenAIAdapter 的直通 + 规范化逻辑。
//! 注：配置 endpoint 请填 OpenAI 兼容 base（如 https://spark-api-open.xf-yun.com/v1）。
//!     鉴权用 Bearer: apiKey:apiSecret 组合形式（星火 OpenAI 兼容接口约定）。
use async_trait::async_trait;
use crate::error::AppResult;
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, SseChunk};
use crate::router::CandidateNode;

pub struct SparkAdapter;

#[async_trait]
impl crate::backend_adapters::BackendAdapter for SparkAdapter {
    fn protocol(&self) -> crate::router::ProtocolKind { crate::router::ProtocolKind::Spark }
    async fn translate_request(&self, oai: &ChatCompletionRequest, node: &CandidateNode) -> AppResult<reqwest::RequestBuilder> {
        crate::backend_adapters::openai_adapter::OpenAIAdapter.translate_request(oai, node).await
    }
    fn parse_response_body(&self, bytes: bytes::Bytes) -> AppResult<ChatCompletionResponse> {
        crate::backend_adapters::openai_adapter::OpenAIAdapter.parse_response_body(bytes)
    }
    fn translate_sse_chunk(&self, vendor_line: &str) -> AppResult<Option<SseChunk>> {
        crate::backend_adapters::openai_adapter::OpenAIAdapter.translate_sse_chunk(vendor_line)
    }
}
