//! OpenAI Responses API 适配器（POST /v1/responses，非流式 + SSE）。
//!
//! 适用于只支持 `openai-response` 端点的模型（例如聚合网关上的 Grok）。
//! 请求：{model, input, instructions, max_output_tokens, stream, temperature, top_p}
//! 响应：{id, object:response, status, model, output:[{type:message,content:[{type:output_text,text}]}], usage}
//! 流式：SSE 事件 response.output_text.delta / response.completed 等。
use async_trait::async_trait;
use crate::error::{AppError, AppResult, ErrorLabel};
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, MessageContent, SseChunk, SseChoice, ToolCall, ToolCallFn, ToolChoice, Usage};
use crate::router::CandidateNode;
use serde_json::{json, Value};

/// ResponsesAdapter 每请求新建实例（adapter_for 按请求派发）。
/// Responses 的 output_index 含 reasoning/message 等项，需映射到 OpenAI tool_calls 连续序号。
#[derive(Default)]
pub struct ResponsesAdapter {
    tool_idx_map: std::sync::Mutex<std::collections::HashMap<u64, u32>>,
    tool_seq:     std::sync::atomic::AtomicU32,
}

impl ResponsesAdapter {
    fn seq_for(&self, out_index: u64) -> u32 {
        let mut map = self.tool_idx_map.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(&s) = map.get(&out_index) { return s; }
        let next = self.tool_seq.load(std::sync::atomic::Ordering::Relaxed);
        self.tool_seq.store(next + 1, std::sync::atomic::Ordering::Relaxed);
        map.insert(out_index, next);
        next
    }
}

/// OpenAI tool_choice → Responses tool_choice（Responses 用字符串或 {type:function,name}）
fn map_tool_choice(tc: &Option<ToolChoice>) -> Option<Value> {
    match tc {
        None => None,
        Some(ToolChoice::Str(s)) => match s.as_str() {
            "none" | "auto" | "required" => Some(json!(s)),
            _ => Some(json!("auto")),
        },
        Some(ToolChoice::Obj(v)) => {
            if v.get("type").and_then(|x| x.as_str()) == Some("function") {
                let name = v.pointer("/function/name").and_then(|x| x.as_str()).unwrap_or("");
                Some(json!({"type": "function", "name": name}))
            } else { None }
        }
    }
}

/// 上游 output[] → 拼接文本（message.content[].output_text / text）
fn collect_output_text(v: &Value) -> String {
    let Some(arr) = v.get("output").and_then(|x| x.as_array()) else { return String::new() };
    let mut out = String::new();
    for item in arr {
        let Some(msg) = item.get("content").and_then(|x| x.as_array()) else { continue };
        for part in msg {
            let t = part.get("text").or_else(|| part.get("output_text")).and_then(|x| x.as_str()).unwrap_or("");
            out.push_str(t);
        }
    }
    out
}

fn stop_reason_map(status: &str) -> Option<String> {
    Some(match status {
        "incomplete" => "length".to_string(),
        _ => "stop".to_string(),
    })
}

#[async_trait]
impl crate::backend_adapters::BackendAdapter for ResponsesAdapter {
    fn protocol(&self) -> crate::router::ProtocolKind { crate::router::ProtocolKind::OpenAIResponse }

    async fn translate_request(&self, oai: &ChatCompletionRequest, node: &CandidateNode) -> AppResult<reqwest::RequestBuilder> {
        let url = format!("{}/v1/responses", node.endpoint.trim_end_matches('/'));
        let client = crate::backend_adapters::http_client();

        let mut instructions = String::new();
        let mut input: Vec<Value> = Vec::new();
        for m in &oai.messages {
            if m.role == "system" {
                if let MessageContent::Text(s) = &m.content {
                    if !instructions.is_empty() { instructions.push('\n'); }
                    instructions.push_str(s);
                }
                continue;
            }
            // assistant 携带 tool_calls → 展开为 function_call 项（宁滥不缺：完整转发）
            if let Some(tcs) = &m.tool_calls {
                for tc in tcs {
                    input.push(json!({
                        "type": "function_call",
                        "call_id": tc.id.clone().unwrap_or_default(),
                        "name": tc.function.as_ref().and_then(|f| f.name.clone()).unwrap_or_default(),
                        "arguments": tc.function.as_ref().and_then(|f| f.arguments.clone()).unwrap_or_default(),
                    }));
                }
            }
            // tool 角色消息 → function_call_output 项
            if m.role == "tool" {
                let out = match &m.content { MessageContent::Text(s) => s.clone(), MessageContent::MultiPart(ps) => ps.iter().filter_map(|p| p.text.clone()).collect::<Vec<_>>().join("") };
                input.push(json!({
                    "type": "function_call_output",
                    "call_id": m.tool_call_id.clone().unwrap_or_default(),
                    "output": out,
                }));
                continue;
            }
            let content = match &m.content {
                MessageContent::Text(s) => Value::String(s.clone()),
                MessageContent::MultiPart(parts) => {
                    let arr: Vec<Value> = parts.iter().map(|p| {
                        if p.kind == "image_url" {
                            let url = p.image_url.as_ref().map(|i| i.url.clone()).unwrap_or_default();
                            json!({"type": "input_image", "image_url": url})
                        } else {
                            json!({"type": "input_text", "text": p.text.clone().unwrap_or_default()})
                        }
                    }).collect();
                    Value::Array(arr)
                }
            };
            if !content.is_null() && !(content.is_string() && content.as_str().unwrap_or("").is_empty() && m.role == "assistant") {
                input.push(json!({"role": m.role, "content": content}));
            }
        }

        let mut body = serde_json::Map::new();
        body.insert("model".into(), json!(oai.model));
        body.insert("input".into(), json!(input));
        if !instructions.is_empty() { body.insert("instructions".into(), json!(instructions)); }
        body.insert("stream".into(), json!(oai.stream));
        if let Some(t) = oai.max_tokens { body.insert("max_output_tokens".into(), json!(t)); }
        if let Some(t) = oai.temperature { body.insert("temperature".into(), json!(t)); }
        if let Some(p) = oai.top_p { body.insert("top_p".into(), json!(p)); }
        // tools：Responses 用扁平 function 形 {type,name,description,parameters}
        if let Some(tools) = &oai.tools {
            if !tools.is_empty() {
                let arr: Vec<Value> = tools.iter().map(|t| json!({
                    "type": "function",
                    "name": t.function.name,
                    "description": t.function.description.clone().unwrap_or_default(),
                    "parameters": if t.function.parameters.is_null() { json!({"type":"object","properties":{}}) } else { t.function.parameters.clone() },
                })).collect();
                body.insert("tools".into(), json!(arr));
            }
        }
        if let Some(tc) = map_tool_choice(&oai.tool_choice) { body.insert("tool_choice".into(), tc); }

        Ok(client.post(&url)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", node._api_key))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&Value::Object(body)))
    }

    fn parse_response_body(&self, bytes: bytes::Bytes) -> AppResult<ChatCompletionResponse> {
        let v: Value = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::Labeled { label: ErrorLabel::JsonParseFail, message: format!("responses parse fail: {e}") })?;
        let created = chrono::Utc::now().timestamp();
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("resp").to_string();
        let model = v.get("model").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let text = collect_output_text(&v);
        // output[] 中 type=function_call 的项 → OpenAI tool_calls
        let tool_calls: Vec<ToolCall> = v.get("output").and_then(|x| x.as_array()).map(|arr| {
            arr.iter().filter(|it| it.get("type").and_then(|x| x.as_str()) == Some("function_call"))
                .map(|it| ToolCall {
                    id: it.get("call_id").and_then(|x| x.as_str()).map(String::from).or_else(|| it.get("id").and_then(|x| x.as_str()).map(String::from)),
                    kind: Some("function".into()),
                    index: None,
                    function: Some(ToolCallFn {
                        name: it.get("name").and_then(|x| x.as_str()).map(String::from),
                        arguments: it.get("arguments").and_then(|x| x.as_str()).map(String::from),
                    }),
                }).collect()
        }).unwrap_or_default();
        let status = v.get("status").and_then(|x| x.as_str()).unwrap_or("completed");
        let finish = if !tool_calls.is_empty() { Some("tool_calls".to_string()) } else { stop_reason_map(status) };
        let input_u = v.pointer("/usage/input_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let output_u = v.pointer("/usage/output_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let message = ChatMessage { role: "assistant".into(), content: MessageContent::Text(text), name: None, tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) }, tool_call_id: None, extra: json!({}) };
        Ok(ChatCompletionResponse {
            id, object: "chat.completion", created, model,
            choices: vec![Choice { index: 0, message, finish_reason: finish }],
            usage: Some(Usage { prompt_tokens: input_u, completion_tokens: output_u, total_tokens: input_u + output_u, extra: json!({}) }),
            extra: json!({}),
        })
    }

    /// 把 Responses API SSE 事件（data: {...}）逐条翻译为 OpenAI chunk
    fn translate_sse_chunk(&self, vendor_line: &str) -> AppResult<Option<SseChunk>> {
        let line = vendor_line.trim();
        if line.is_empty() || line.starts_with(':') || line.starts_with("event:") { return Ok(None); }
        let Some(data) = line.strip_prefix("data:") else { return Ok(None); };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" { return Ok(None); }
        let v: Value = serde_json::from_str(data)
            .map_err(|e| AppError::Labeled { label: ErrorLabel::JsonParseFail, message: format!("responses sse parse fail: {e}") })?;
        let created = chrono::Utc::now().timestamp();
        let kind = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
        match kind {
            "response.created" => {
                let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("resp").to_string();
                let model = v.pointer("/response/model").and_then(|x| x.as_str()).unwrap_or("").to_string();
                Ok(Some(SseChunk {
                    id: Some(id), object: Some("chat.completion.chunk"), created: Some(created), model: Some(model),
                    choices: vec![SseChoice { index: 0, delta: Some(ChatMessage { role: "assistant".into(), content: MessageContent::Text(String::new()), name: None, tool_calls: None, tool_call_id: None, extra: json!({}) }), finish_reason: None }],
                    usage: None,
                }))
            }
            "response.output_text.delta" => {
                let text = v.get("delta").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let d = Some(ChatMessage { role: "assistant".into(), content: MessageContent::Text(text), name: None, tool_calls: None, tool_call_id: None, extra: json!({}) });
                Ok(Some(SseChunk { id: None, object: Some("chat.completion.chunk"), created: Some(created), model: None, choices: vec![SseChoice { index: 0, delta: d, finish_reason: None }], usage: None }))
            }
            // function_call 输出项开始 → tool_call 头部（id + name + 空 arguments）
            "response.output_item.added" => {
                let item = v.get("item").cloned().unwrap_or_else(|| json!({}));
                if item.get("type").and_then(|x| x.as_str()) != Some("function_call") { return Ok(None); }
                let out_index = v.get("output_index").and_then(|x| x.as_u64()).unwrap_or(0);
                let seq = self.seq_for(out_index);
                let tc = ToolCall {
                    id: item.get("call_id").and_then(|x| x.as_str()).map(String::from).or_else(|| item.get("id").and_then(|x| x.as_str()).map(String::from)),
                    kind: Some("function".into()),
                    index: Some(seq),
                    function: Some(ToolCallFn {
                        name: item.get("name").and_then(|x| x.as_str()).map(String::from),
                        arguments: Some(String::new()),
                    }),
                };
                let d = ChatMessage { role: "assistant".into(), content: MessageContent::Text(String::new()), name: None, tool_calls: Some(vec![tc]), tool_call_id: None, extra: json!({}) };
                Ok(Some(SseChunk { id: None, object: Some("chat.completion.chunk"), created: Some(created), model: None, choices: vec![SseChoice { index: 0, delta: Some(d), finish_reason: None }], usage: None }))
            }
            // function_call 参数增量
            "response.function_call_arguments.delta" => {
                let out_index = v.get("output_index").and_then(|x| x.as_u64()).unwrap_or(0);
                let seq = self.seq_for(out_index);
                let piece = v.get("delta").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let tc = ToolCall { id: None, kind: None, index: Some(seq), function: Some(ToolCallFn { name: None, arguments: Some(piece) }) };
                let d = ChatMessage { role: "assistant".into(), content: MessageContent::Text(String::new()), name: None, tool_calls: Some(vec![tc]), tool_call_id: None, extra: json!({}) };
                Ok(Some(SseChunk { id: None, object: Some("chat.completion.chunk"), created: Some(created), model: None, choices: vec![SseChoice { index: 0, delta: Some(d), finish_reason: None }], usage: None }))
            }
            "response.completed" => {
                let status = v.pointer("/response/status").and_then(|x| x.as_str()).unwrap_or("completed");
                let io = async_usage(&v);
                // 输出含 function_call 时 finish_reason 纠正为 tool_calls
                let has_fc = v.pointer("/response/output").and_then(|x| x.as_array())
                    .map(|arr| arr.iter().any(|it| it.get("type").and_then(|x| x.as_str()) == Some("function_call")))
                    .unwrap_or(false);
                let finish = if has_fc { Some("tool_calls".to_string()) } else { stop_reason_map(status) };
                Ok(Some(SseChunk { id: None, object: Some("chat.completion.chunk"), created: Some(created), model: None, choices: vec![SseChoice { index: 0, delta: None, finish_reason: finish }], usage: io }))
            }
            _ => Ok(None),
        }
    }
}

fn async_usage(v: &Value) -> Option<Usage> {
    let u = v.pointer("/response/usage")?;
    let i = u.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    let o = u.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    Some(Usage { prompt_tokens: i, completion_tokens: o, total_tokens: i + o, extra: json!({}) })
}
