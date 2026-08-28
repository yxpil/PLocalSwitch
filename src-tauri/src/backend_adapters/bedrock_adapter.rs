//! 空骨架：BedrockConverse 协议适配器（待后续补充双向转换逻辑）
use async_trait::async_trait;
use crate::error::{AppError, AppResult, ErrorLabel};
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, SseChunk};
pub struct BedrockConverseAdapter;
#[async_trait]
impl crate::backend_adapters::BackendAdapter for BedrockConverseAdapter {
    fn protocol(&self) -> crate::router::ProtocolKind {
        crate::router::ProtocolKind::BedrockConverse
    }
    async fn translate_request(&self, oai: &ChatCompletionRequest, node: &crate::router::CandidateNode) -> AppResult<reqwest::RequestBuilder> {
        let _ = (oai, node);
        Err(AppError::Labeled { label: ErrorLabel::Internal, message: "BedrockConverseAdapter stub: translate_request TODO".into() })
    }
    fn parse_response_body(&self, bytes: bytes::Bytes) -> AppResult<ChatCompletionResponse> {
        let _ = bytes;
        Err(AppError::Labeled { label: ErrorLabel::JsonParseFail, message: "BedrockConverseAdapter stub: parse_response_body TODO".into() })
    }
    fn translate_sse_chunk(&self, vendor_line: &str) -> AppResult<Option<SseChunk>> {
        let _ = vendor_line;
        Ok(None)
    }
}
