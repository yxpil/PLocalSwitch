//! Ollama 原生协议适配器（POST /api/chat，流式 newline-delimited JSON）
//!
//! 非流式响应：{model, created_at, message:{role,content}, done, prompt_eval_count, eval_count}
//! 流式响应： 每个 JSON 对象一行（无 `data:` 前缀），done=true 为最后一块
//!
//! 这里把 OpenAI v1 标准请求/响应双向转换成 Ollama 格式。
use async_trait::async_trait;
use crate::error::{AppError, AppResult, ErrorLabel};
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, MessageContent, SseChunk, SseChoice, ToolCall, ToolCallFn, Usage};
use crate::router::CandidateNode;
use serde_json::{json, Map, Value};

pub struct OllamaAdapter;

/// Ollama message.tool_calls（arguments 可能是对象或字符串）→ OpenAI ToolCall 列表
fn tool_calls_from_ollama(v: &Value) -> Option<Vec<ToolCall>> {
    let arr = v.as_array()?;
    let mut out = Vec::new();
    for (i, tc) in arr.iter().enumerate() {
        let f = tc.get("function").cloned().unwrap_or_else(|| json!({}));
        let arguments = match f.get("arguments") {
            Some(Value::String(s)) => s.clone(),
            Some(x) => x.to_string(),
            None => "{}".to_string(),
        };
        out.push(ToolCall {
            id: tc.get("id").and_then(|x| x.as_str()).map(String::from)
                .or_else(|| Some(format!("call_{}", i))),
            kind: Some("function".into()),
            index: None,
            function: Some(ToolCallFn {
                name: f.get("name").and_then(|x| x.as_str()).map(String::from),
                arguments: Some(arguments),
            }),
        });
    }
    if out.is_empty() { None } else { Some(out) }
}

/// 把 OpenAI 消息转换为 Ollama 可用的消息（content 必须是字符串；图片转 images[] base64 数组）
fn to_ollama_message(m: &ChatMessage) -> Option<Value> {
    let content = match &m.content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::MultiPart(parts) => parts.iter().filter_map(|p| p.text.clone()).collect::<Vec<_>>().join(""),
    };
    // tool 结果消息：Ollama 用 role=tool + content
    if m.role == "tool" {
        return Some(json!({ "role": "tool", "content": content }));
    }
    let mut msg = json!({ "role": m.role, "content": content });
    // 图片：OpenAI image_url → Ollama images[]（裸 base64，不带 data: 前缀）
    if let MessageContent::MultiPart(parts) = &m.content {
        let imgs: Vec<String> = parts.iter().filter_map(|p| {
            let url = p.image_url.as_ref()?.url.clone();
            url.strip_prefix("data:").and_then(|r| r.split_once(',')).map(|(_, b64)| b64.to_string())
                .or_else(|| if url.starts_with("http") { Some(url) } else { None })
        }).collect();
        if !imgs.is_empty() { msg["images"] = json!(imgs); }
    }
    // assistant tool_calls → 透传（arguments 转回对象以兼容严格校验的 Ollama 版本）
    if let Some(tcs) = &m.tool_calls {
        if !tcs.is_empty() {
            let arr: Vec<Value> = tcs.iter().map(|tc| {
                let args: Value = tc.function.as_ref().and_then(|f| f.arguments.clone())
                    .and_then(|a| serde_json::from_str(&a).ok()).unwrap_or_else(|| json!({}));
                json!({"function": {"name": tc.function.as_ref().and_then(|f| f.name.clone()).unwrap_or_default(), "arguments": args}})
            }).collect();
            msg["tool_calls"] = json!(arr);
        }
    }
    if msg.get("tool_calls").is_none() && content.is_empty() { return None; }
    Some(msg)
}

/// 从 Ollama JSON 消息块构造 OpenAI ChatMessage
fn message_from_ollama(v: &Value) -> ChatMessage {
    let role = v.get("role").and_then(|x| x.as_str()).unwrap_or("assistant").to_string();
    let content = v.get("content").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let tool_calls = v.get("tool_calls").and_then(tool_calls_from_ollama);
    ChatMessage { role, content: MessageContent::Text(content), name: None, tool_calls, tool_call_id: None, extra: serde_json::json!({}) }
}

#[async_trait]
impl crate::backend_adapters::BackendAdapter for OllamaAdapter {
    fn protocol(&self) -> crate::router::ProtocolKind {
        crate::router::ProtocolKind::Ollama
    }

    /// 构造 `{endpoint}/api/chat` 的 Ollama 请求（默认无鉴权；给了 key 才带 Bearer）
    async fn translate_request(&self, oai: &ChatCompletionRequest, node: &CandidateNode) -> AppResult<reqwest::RequestBuilder> {
        let url = format!("{}/api/chat", node.endpoint.trim_end_matches('/'));
        let messages: Vec<Value> = oai.messages.iter().filter_map(to_ollama_message).collect();

        let mut options = Map::new();
        if let Some(t) = oai.temperature { options.insert("temperature".into(), json!(t)); }
        if let Some(p) = oai.top_p { options.insert("top_p".into(), json!(p)); }
        if let Some(m) = oai.max_tokens { options.insert("num_predict".into(), json!(m)); }

        let mut body = Map::new();
        body.insert("model".into(), json!(oai.model));
        body.insert("messages".into(), json!(messages));
        body.insert("stream".into(), json!(oai.stream));
        if !options.is_empty() { body.insert("options".into(), Value::Object(options)); }
        // tools：Ollama 原生接受 OpenAI function 形（{type,function:{name,description,parameters}}）→ 完整透传
        if let Some(tools) = &oai.tools {
            if !tools.is_empty() {
                body.insert("tools".into(), serde_json::to_value(tools).unwrap_or(Value::Null));
            }
        }

        let client = crate::backend_adapters::ollama_http_client();
        let mut rb = client.post(&url).header(reqwest::header::CONTENT_TYPE, "application/json").json(&Value::Object(body));
        if !node._api_key.is_empty() {
            rb = rb.header(reqwest::header::AUTHORIZATION, format!("Bearer {}", node._api_key));
        }
        Ok(rb)
    }

    /// 解析非流式 Ollama 响应 → 标准 ChatCompletionResponse
    fn parse_response_body(&self, bytes: bytes::Bytes) -> AppResult<ChatCompletionResponse> {
        let v: Value = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::Labeled { label: ErrorLabel::JsonParseFail, message: format!("ollama response parse fail: {e}") })?;
        let created = chrono::Utc::now().timestamp();
        let model = v.get("model").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let msg = v.get("message").cloned().unwrap_or_else(|| json!({"role":"assistant","content":""}));
        let message = message_from_ollama(&msg);
        let prompt = v.get("prompt_eval_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let completion = v.get("eval_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let done = v.get("done").and_then(|x| x.as_bool()).unwrap_or(true);
        let finish = if message.tool_calls.is_some() { Some("tool_calls".to_string()) } else if done { Some("stop".to_string()) } else { None };
        let usage = Usage { prompt_tokens: prompt, completion_tokens: completion, total_tokens: prompt + completion, extra: json!({}) };
        Ok(ChatCompletionResponse {
            id: format!("ollama-{created}"),
            object: "chat.completion",
            created,
            model,
            choices: vec![Choice { index: 0, message, finish_reason: finish }],
            usage: Some(usage),
            extra: json!({}),
        })
    }

    /// 逐行（newline-delimited JSON）翻译 Ollama 流式块
    fn translate_sse_chunk(&self, vendor_line: &str) -> AppResult<Option<SseChunk>> {
        let line = vendor_line.trim().strip_prefix("data:").map(|s| s.trim()).unwrap_or(vendor_line.trim());
        if line.is_empty() || line.starts_with(':') { return Ok(None); }
        let v: Value = serde_json::from_str(line)
            .map_err(|e| AppError::Labeled { label: ErrorLabel::JsonParseFail, message: format!("ollama sse parse fail: {e}") })?;
        let created = chrono::Utc::now().timestamp();
        let model = v.get("model").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let msg = v.get("message").cloned().unwrap_or_else(|| json!({"role":"assistant","content":""}));
        let delta = message_from_ollama(&msg);
        let done = v.get("done").and_then(|x| x.as_bool()).unwrap_or(false);
        let finish_reason = if delta.tool_calls.is_some() { Some("tool_calls".to_string()) } else if done { Some("stop".to_string()) } else { None };
        // done 块携带 token 统计 → 回填 usage
        let usage = if done {
            let p = v.get("prompt_eval_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let c = v.get("eval_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            Some(Usage { prompt_tokens: p, completion_tokens: c, total_tokens: p + c, extra: json!({}) })
        } else { None };
        Ok(Some(SseChunk {
            id: Some(format!("ollama-{created}")),
            object: Some("chat.completion.chunk"),
            created: Some(created),
            model: Some(model),
            choices: vec![SseChoice { index: 0, delta: Some(delta), finish_reason }],
            usage,
        }))
    }
}
