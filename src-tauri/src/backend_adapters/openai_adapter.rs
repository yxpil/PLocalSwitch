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

/// 智能拼接 chat 路径：
///   - 已以 `/chat/completions` 结尾 → 原样使用
///   - endpoint 带路径（如智谱 `https://open.bigmodel.cn/api/paas/v4`）→ 追加 `/chat/completions`
///   - 纯域名 → `/v1/chat/completions`（OpenAI/NVIDIA/DeepSeek/packyapi 等主流兼容上游）
pub(crate) fn build_chat_url(endpoint: &str) -> String {
    let e = endpoint.trim().trim_end_matches('/');
    if e.ends_with("/chat/completions") {
        return e.to_string();
    }
    let has_path = e
        .split_once("://")
        .map(|(_, rest)| rest.contains('/'))
        .unwrap_or(false);
    if has_path {
        format!("{e}/chat/completions")
    } else {
        format!("{e}/v1/chat/completions")
    }
}

#[async_trait]
impl crate::backend_adapters::BackendAdapter for OpenAIAdapter {
    fn protocol(&self) -> crate::router::ProtocolKind {
        crate::router::ProtocolKind::OpenAI
    }

    /// 构建指向上游 chat 端点的请求（Bearer 鉴权 + JSON body）
    async fn translate_request(&self, oai: &ChatCompletionRequest, node: &CandidateNode) -> AppResult<reqwest::RequestBuilder> {
        let url = build_chat_url(&node.endpoint);
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
