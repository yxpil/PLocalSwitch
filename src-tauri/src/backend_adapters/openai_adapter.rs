//! OpenAI Chat-Completions 协议适配器（真正的上游转发 + 响应/流式格式转换）
//!
//! 上游通用格式为 OpenAI v1：
//!   - 非流式响应 choices[].message { role, content, ... } + usage
//!   - 流式 SSE data: {"choices":[{"delta":{"content": "..."}}]} … data: [DONE]
//!
//! 做的是“直通 + 规范化”：请求 body 基本原样透传（model 已在路由层替换为真实模型名），
//! 响应非流式解析为 ChatCompletionResponse；流式逐块翻译为标准 SseChunk。
use async_trait::async_trait;
use crate::error::{AppError, AppResult, ErrorLabel};
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, SseChunk};
use crate::router::CandidateNode;

pub struct OpenAIAdapter;

#[async_trait]
impl crate::backend_adapters::BackendAdapter for OpenAIAdapter {
    fn protocol(&self) -> crate::router::ProtocolKind {
        crate::router::ProtocolKind::OpenAI
    }

    /// 构建指向 `{endpoint}/chat/completions` 的上游请求（Bearer 鉴权 + JSON body）
    async fn translate_request(&self, oai: &ChatCompletionRequest, node: &CandidateNode) -> AppResult<reqwest::RequestBuilder> {
        // 与模型检测 {endpoint}/v1/models 保持一致：多数 OpenAI 兼容上游（OpenAI/packyapi/DeepSeek）
        // 的 chat 路径都在 /v1 前缀下，统一拼 /v1/chat/completions。
        let url = format!("{}/v1/chat/completions", node.endpoint.trim_end_matches('/'));
        let client = crate::backend_adapters::http_client();
        Ok(client.post(&url)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", node._api_key))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(oai))
    }

    /// 解析上游非流式 JSON → 标准响应（含 usage）
    fn parse_response_body(&self, bytes: bytes::Bytes) -> AppResult<ChatCompletionResponse> {
        let v: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::Labeled { label: ErrorLabel::JsonParseFail, message: format!("upstream non-stream json parse fail: {e}") })?;
        ChatCompletionResponse::from_upstream(v)
    }

    /// 逐行翻译上游 SSE（一次一行 `data: {...}` 或非 data 行）
    ///   - 返回 Ok(Some(chunk)) 表示有效 chunk
    ///   - 返回 Ok(None) 表示应忽略（注释行 / 空行）
    ///   - `data: [DONE]` 由调用方（execute_stream）负责截断
    fn translate_sse_chunk(&self, vendor_line: &str) -> AppResult<Option<SseChunk>> {
        let line = vendor_line.trim();
        if line.is_empty() || line.starts_with(':') {
            return Ok(None);
        }
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(None);
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            return Ok(None);
        }
        let v: serde_json::Value = serde_json::from_str(data)
            .map_err(|e| AppError::Labeled { label: ErrorLabel::JsonParseFail, message: format!("upstream sse json parse fail: {e}") })?;
        SseChunk::from_upstream(v).map(Some)
    }
}
