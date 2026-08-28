//! 腾讯混元 协议适配器 —— 混元提供 OpenAI 兼容端点（/v1/chat/completions），
//! 因此复用 OpenAIAdapter 的直通 + 规范化逻辑。
//! 注：配置 endpoint 请填 OpenAI 兼容 base（如 https://api.hunyuan.cloud.tencent.com/v1）。
use async_trait::async_trait;
use crate::error::AppResult;
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, SseChunk};
use crate::router::CandidateNode;

pub struct HunyuanAdapter;

#[async_trait]
impl crate::backend_adapters::BackendAdapter for HunyuanAdapter {
    fn protocol(&self) -> crate::router::ProtocolKind { crate::router::ProtocolKind::Hunyuan }
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
