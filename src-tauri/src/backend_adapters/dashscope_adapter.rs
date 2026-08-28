//! 阿里云 DashScope(通义) 协议适配器 —— 官方提供 OpenAI 兼容模式
//! （compatible-mode/v1，OpenAI SDK 直接用），因此复用 OpenAIAdapter。
//! 注：配置 endpoint 请填 OpenAI 兼容地址（如 https://dashscope.aliyuncs.com/compatible-mode/v1）。
use async_trait::async_trait;
use crate::error::AppResult;
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, SseChunk};
use crate::router::CandidateNode;

pub struct DashScopeAdapter;

#[async_trait]
impl crate::backend_adapters::BackendAdapter for DashScopeAdapter {
    fn protocol(&self) -> crate::router::ProtocolKind { crate::router::ProtocolKind::DashScope }
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
