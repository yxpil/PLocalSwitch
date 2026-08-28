//! CustomOpenAICompat 协议适配器（各类 OpenAI 兼容推理服务）
//!
//! 与标准 OpenAI 兼容（NVIDIA NIM / DeepSeek / 各类一键部署网关），
//! 因此直接复用 OpenAIAdapter 的直通 + 规范化逻辑。
use async_trait::async_trait;
use crate::error::AppResult;
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, SseChunk};
use crate::router::CandidateNode;

pub struct CustomCompatAdapter;

#[async_trait]
impl crate::backend_adapters::BackendAdapter for CustomCompatAdapter {
    fn protocol(&self) -> crate::router::ProtocolKind {
        crate::router::ProtocolKind::CustomOpenAICompat
    }
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
