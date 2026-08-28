//! =============================================================
//!  协议通用数据模型（网关唯一数据标准）
//!  对外永远输出：OpenAI v1 标准 ChatCompletionRequest / Response / SseChunk
//!  所有厂商适配器必须最终翻译为此三态。
//! =============================================================
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------- Request ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model:    String,
    pub messages: Vec<ChatMessage>,
    #[serde(default = "default_true")]
    pub stream: bool,
    #[serde(default)]
    pub temperature:     Option<f32>,
    #[serde(default)]
    pub top_p:           Option<f32>,
    #[serde(default)]
    pub max_tokens:      Option<u32>,
    #[serde(default)]
    pub response_format: Option<ResponseFormat>,
    #[serde(default)]
    pub tools:           Option<Vec<ToolDef>>,
    #[serde(default)]
    pub tool_choice:     Option<ToolChoice>,
    #[serde(default, flatten)]
    pub extras: Value,
}
fn default_true() -> bool { false }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,                      // system/user/assistant/tool
    #[serde(default)]
    pub content: MessageContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// 透传未被映射的扩展字段（如 deepseek-reasoner 的 reasoning_content、logprobs 等）
    #[serde(default, flatten)]
    pub extra: Value,
}
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    MultiPart(Vec<ContentPart>),
}
impl Default for MessageContent {
    fn default() -> Self { MessageContent::Text(String::new()) }
}
// 自定义反序列化：兼容 content 为 null / 缺失 / 字符串 / 数组 的常见情况（tool-call 消息常带 content:null）
impl<'de> serde::Deserialize<'de> for MessageContent {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let v = Value::deserialize(d)?;
        match v {
            Value::Null => Ok(MessageContent::Text(String::new())),
            Value::String(s) => Ok(MessageContent::Text(s)),
            Value::Array(items) => {
                let mut parts = Vec::with_capacity(items.len());
                for it in items {
                    let p: ContentPart = serde_json::from_value(it).map_err(serde::de::Error::custom)?;
                    parts.push(p);
                }
                Ok(MessageContent::MultiPart(parts))
            }
            other => Ok(MessageContent::Text(other.to_string())),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPart {
    #[serde(rename = "type")]
    pub kind: String,                     // text | image_url
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub image_url: Option<ImageUrl>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl { pub url: String, #[serde(default)] pub detail: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub json_schema: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionDef,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    pub name:        String,
    #[serde(default)]
    pub description: Option<String>,
    pub parameters:  Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Str(String),
    Obj(Value),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 流式增量时后续块常缺 id/type/name，故全部可选（宁滥不缺：有则转发，无则省略字段）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<ToolCallFn>,
    /// 流式增量时 OpenAI 用 index 定位第几个 tool_call；非流式可省略
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFn {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name:      Option<String>,
    /// JSON 字符串（OpenAI 标准）；流式时为增量片段
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

// ---------- Non-stream Response ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id:      String,
    pub object:  &'static str,
    pub created: i64,
    pub model:   String,
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage:   Option<Usage>,
    #[serde(default, flatten)]
    pub extra:   Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index:         u32,
    pub message:       ChatMessage,
    pub finish_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    pub prompt_tokens:     u32,
    pub completion_tokens: u32,
    pub total_tokens:      u32,
    #[serde(default)]
    pub extra: Value,
}

// ---------- Stream chunk (SSE data: {...}) ----------
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SseChunk {
    pub id:      Option<String>,
    pub object:  Option<&'static str>,
    pub created: Option<i64>,
    pub model:   Option<String>,
    pub choices: Vec<SseChoice>,
    pub usage:   Option<Usage>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SseChoice {
    pub index:         u32,
    pub delta:         Option<ChatMessage>,
    pub finish_reason: Option<String>,
}

/// 宽容地从上游 JSON 构造 ChatMessage：
///   - role 缺省按 "assistant"
///   - content 缺省/为 null 时为空文本；字符串直接取；list 作为 MultiPart
///   - 流式 delta 常缺 role / content，因此必须宽容，否则中间块/结束块会解析失败
fn chat_message_from_value(v: &serde_json::Value) -> crate::error::AppResult<ChatMessage> {
    let role = v.get("role").and_then(|r| r.as_str()).unwrap_or("assistant").to_string();
    let content = match v.get("content") {
        Some(serde_json::Value::String(s)) => MessageContent::Text(s.clone()),
        Some(serde_json::Value::Array(items)) => {
            let mut parts = Vec::new();
            for it in items {
                let kind = it.get("type").and_then(|x| x.as_str()).unwrap_or("text").to_string();
                let text = it.get("text").and_then(|x| x.as_str()).map(|s| s.to_string());
                let image_url = it.get("image_url").and_then(|x| serde_json::from_value::<ImageUrl>(x.clone()).ok());
                parts.push(ContentPart { kind, text, image_url });
            }
            MessageContent::MultiPart(parts)
        }
        // content:null（流式推理块常这样，表示没内容）→ 空串，避免变成字符串 "null"
        Some(serde_json::Value::Null) | None => MessageContent::Text(String::new()),
        Some(other) => MessageContent::Text(other.to_string()),
    };
    let name = v.get("name").and_then(|x| x.as_str()).map(|s| s.to_string());
    let tool_calls = v.get("tool_calls").and_then(|x| serde_json::from_value::<Vec<ToolCall>>(x.clone()).ok());
    let tool_call_id = v.get("tool_call_id").and_then(|x| x.as_str()).map(|s| s.to_string());
    // 剩余字段（reasoning_content / logprobs 等）原样透传，保证响应无损
    let mut extra = v.clone();
    if let Some(m) = extra.as_object_mut() {
        for k in ["role", "content", "name", "tool_calls", "tool_call_id"] { m.remove(k); }
    } else {
        extra = serde_json::json!({});
    }
    Ok(ChatMessage { role, content, name, tool_calls, tool_call_id, extra })
}

impl ChatCompletionResponse {
    /// 从上游 OpenAI 兼容 JSON 构造标准响应。
    /// 注意：object 是 &'static str，无法直接 serde 反序列化，所以手工映射。
    /// 兼容字段别名：choices[].message / choices[].delta / choices[].text (旧式)。
    pub fn from_upstream(v: serde_json::Value) -> crate::error::AppResult<Self> {
        use crate::error::{AppError, ErrorLabel};
        let get_str = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let id = if get_str("id").is_empty() { "chatcmpl".to_string() } else { get_str("id") };
        let created = v.get("created").and_then(|x| x.as_i64()).unwrap_or(0);
        let model = get_str("model");

        let mut choices = Vec::new();
        if let Some(arr) = v.get("choices").and_then(|x| x.as_array()) {
            for c in arr {
                // message 优先，其次 delta（流式），再其次 text（旧式 chat completion）
                let message = if let Some(m) = c.get("message").or_else(|| c.get("delta")) {
                    chat_message_from_value(m)?
                } else if let Some(text) = c.get("text").and_then(|x| x.as_str()) {
                    ChatMessage { role: "assistant".into(), content: MessageContent::Text(text.to_string()), name: None, tool_calls: None, tool_call_id: None, extra: serde_json::json!({}) }
                } else {
                    return Err(AppError::Labeled { label: ErrorLabel::JsonParseFail, message: "bad choice: no message".into() });
                };
                let index = c.get("index").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                let finish_reason = c.get("finish_reason").and_then(|x| x.as_str()).map(|s| s.to_string());
                choices.push(Choice { index, message, finish_reason });
            }
        }

        let usage = v.get("usage")
            .and_then(|u| serde_json::from_value::<Usage>(u.clone()).ok());
        if choices.is_empty() {
            return Err(AppError::Labeled { label: ErrorLabel::JsonParseFail, message: "no choices in upstream response".into() });
        }
        Ok(Self { id, object: "chat.completion", created, model, choices, usage, extra: serde_json::json!({}) })
    }
}

impl SseChunk {
    /// 从上游 OpenAI 兼容 SSE data 载荷构造流式 chunk（object: chat.completion.chunk）
    pub fn from_upstream(v: serde_json::Value) -> crate::error::AppResult<Self> {
        let get_str = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let id = if get_str("id").is_empty() { None } else { Some(get_str("id")) };
        let created = v.get("created").and_then(|x| x.as_i64());
        let model = { let s = get_str("model"); if s.is_empty() { None } else { Some(s) } };
        let mut choices = Vec::new();
        if let Some(arr) = v.get("choices").and_then(|x| x.as_array()) {
            for c in arr {
                // 兼容 choices[].delta 与 choices[].text
                let delta = if let Some(d) = c.get("delta") {
                    Some(chat_message_from_value(d)?)
                } else if let Some(t) = c.get("text").and_then(|x| x.as_str()) {
                    Some(ChatMessage { role: "assistant".into(), content: MessageContent::Text(t.to_string()), name: None, tool_calls: None, tool_call_id: None, extra: serde_json::json!({}) })
                } else {
                    None
                };
                let index = c.get("index").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                let finish_reason = c.get("finish_reason").and_then(|x| x.as_str()).map(|s| s.to_string());
                choices.push(SseChoice { index, delta, finish_reason });
            }
        }
        let usage = v.get("usage").and_then(|u| serde_json::from_value::<Usage>(u.clone()).ok());
        Ok(Self { id, object: Some("chat.completion.chunk"), created, model, choices, usage })
    }
}

// ---------- Models list ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelList {
    pub object: &'static str,
    pub data:   Vec<ModelItem>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelItem {
    pub id:         String,
    pub object:     &'static str,
    pub created:    i64,
    pub owned_by:   String,
}

// ---------- Embeddings ----------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: EmbeddingInput,
    #[serde(default)] pub encoding_format: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    One(String),
    Many(Vec<String>),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub object: &'static str,
    pub data:   Vec<EmbeddingItem>,
    pub model:  String,
    pub usage:  Usage,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingItem {
    pub object:    &'static str,
    pub embedding: Vec<f32>,
    pub index:     u32,
}

// ---------- 保留原先 API 信封（桌面 IPC 用）----------
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
    pub timestamp: i64,
}
impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true, data: Some(data), message: "ok".into(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
    pub fn fail(msg: impl Into<String>) -> Self {
        Self {
            success: false, data: None, message: msg.into(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemInfo {
    pub app_version: String,
    pub rust_version: String,
    pub os: String,
    pub arch: String,
    pub uptime: i64,
    pub request_count: u64,
}
