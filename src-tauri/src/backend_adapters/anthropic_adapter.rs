//! Anthropic Messages API 适配器（/v1/messages，非流式 + SSE）
//!
//! 请求：{model, max_tokens, system, messages:[{role, content:[{type:text,text}...]}], temperature, top_p, stream}
//! 响应：{id, type:message, role:assistant, content:[{type:text,text}], model, stop_reason, usage:{input_tokens, output_tokens}}
//! 流式：SSE event: message_start / content_block_delta / message_delta / message_stop，每条 data 是独立 JSON
use async_trait::async_trait;
use crate::error::{AppError, AppResult, ErrorLabel};
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, MessageContent, SseChunk, SseChoice, ToolCall, ToolCallFn, ToolChoice, Usage};
use crate::router::CandidateNode;
use serde_json::{json, Value};

/// 每个流式请求会新建一个 adapter 实例（adapter_for 按请求派发）。
/// Anthropic 的 content_block index 含 text/tool_use 混排且不连续，
/// 需把它映射到 OpenAI delta.tool_calls 的连续序号，故用 map 记录本条流的映射。
#[derive(Default)]
pub struct AnthropicAdapter {
    tool_idx_map: std::sync::Mutex<std::collections::HashMap<u64, u32>>,
    tool_seq:     std::sync::atomic::AtomicU32,
    /// message_start 里拿到的 prompt_tokens，message_delta 组 usage 时回填
    last_prompt:  std::sync::atomic::AtomicU32,
}

/// OpenAI tool_choice → Anthropic tool_choice
fn map_tool_choice(tc: &Option<ToolChoice>) -> Option<Value> {
    match tc {
        None => None,
        Some(ToolChoice::Str(s)) => match s.as_str() {
            "none"     => Some(json!({"type": "none"})),
            "required" => Some(json!({"type": "any"})),
            _          => Some(json!({"type": "auto"})),
        },
        Some(ToolChoice::Obj(v)) => {
            if v.get("type").and_then(|x| x.as_str()) == Some("function") {
                let name = v.pointer("/function/name").and_then(|x| x.as_str()).unwrap_or("");
                Some(json!({"type": "tool", "name": name}))
            } else {
                Some(json!({"type": "auto"}))
            }
        }
    }
}

/// Anthropic tool_use 块 → OpenAI ToolCall
fn tool_call_from_block(b: &Value) -> Option<ToolCall> {
    let id = b.get("id").and_then(|x| x.as_str())?.to_string();
    let name = b.get("name").and_then(|x| x.as_str())?.to_string();
    let input = b.get("input").cloned().unwrap_or_else(|| json!({}));
    Some(ToolCall {
        id: Some(id), kind: Some("function".into()), index: None,
        function: Some(ToolCallFn { name: Some(name), arguments: Some(input.to_string()) }),
    })
}

/// 把 OpenAI 消息正文转成 Anthropic content blocks
fn to_anthropic_blocks(m: &ChatMessage) -> Vec<Value> {
    match &m.content {
        MessageContent::Text(s) => {
            let mut blocks = Vec::new();
            // 仅在非空时输出 text block，避免 assistant 纯 tool_calls 时产生非法空 text 块
            if !s.is_empty() { blocks.push(json!({"type": "text", "text": s})); }
            if let Some(tcs) = &m.tool_calls {
                for tc in tcs {
                    let args = tc.function.as_ref().and_then(|f| f.arguments.clone()).unwrap_or_default();
                    let input: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({}));
                    let name = tc.function.as_ref().and_then(|f| f.name.clone()).unwrap_or_default();
                    blocks.push(json!({"type": "tool_use", "id": tc.id.clone().unwrap_or_default(), "name": name, "input": input}));
                }
            }
            blocks
        }
        MessageContent::MultiPart(parts) => {
            let mut blocks = Vec::new();
            for p in parts {
                if p.kind == "image_url" {
                    if let Some(img) = &p.image_url {
                        if let Some((mime, b64)) = split_data_url(&img.url) {
                            blocks.push(json!({"type": "image", "source": {"type": "base64", "media_type": mime, "data": b64}}));
                        } else if img.url.starts_with("http") {
                            // 远程图 URL：Anthropic 支持 url source（宁滥不缺：两种形式都转发，由上游决定接受哪种）
                            blocks.push(json!({"type": "image", "source": {"type": "url", "url": img.url}}));
                        }
                    }
                } else if p.kind == "input_audio" {
                    // 音频（OpenAI input_audio / Anthropic audio 块）→ Anthropic audio block
                    if let Some(a) = &p.audio {
                        let mime = a.mime_type.clone()
                            .unwrap_or_else(|| format!("audio/{}", a.format.clone().unwrap_or_else(|| "wav".into())));
                        if let Some(d) = &a.data {
                            blocks.push(json!({"type": "audio", "source": {"type": "base64", "media_type": mime, "data": d}}));
                        } else if let Some(u) = &a.url {
                            blocks.push(json!({"type": "audio", "source": {"type": "url", "url": u}}));
                        }
                    }
                } else if p.kind == "document" || p.kind == "file" {
                    // 文档/文件 → Anthropic document block
                    if let Some(f) = &p.file {
                        let mime = f.mime_type.clone().unwrap_or_else(|| "application/pdf".into());
                        if let Some(d) = &f.data {
                            blocks.push(json!({"type": "document", "source": {"type": "base64", "media_type": mime, "data": d}}));
                        } else if let Some(u) = &f.url {
                            blocks.push(json!({"type": "document", "source": {"type": "url", "url": u}}));
                        }
                    }
                } else if let Some(text) = &p.text {
                    blocks.push(json!({"type": "text", "text": text}));
                }
            }
            blocks
        }
    }
}

/// 拆分 data URL = data:<mime>;base64,<data>
fn split_data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (mime, b64) = rest.split_once(',')?;
    let mime = mime.trim_end_matches(";base64").to_string();
    Some((mime, b64.to_string()))
}

fn stop_reason_map(s: &str) -> Option<String> {
    Some(match s {
        "max_tokens" => "length".to_string(),
        "stop_sequence" | "end_turn" => "stop".to_string(),
        "tool_use" => "tool_calls".to_string(),
        _ => "stop".to_string(),
    })
}

fn collect_text(blocks: &[Value]) -> String {
    blocks.iter().filter_map(|b| b.get("text").and_then(|x| x.as_str()).map(|s| s.to_string())).collect::<Vec<_>>().join("")
}

#[async_trait]
impl crate::backend_adapters::BackendAdapter for AnthropicAdapter {
    fn protocol(&self) -> crate::router::ProtocolKind { crate::router::ProtocolKind::Anthropic }

    async fn translate_request(&self, oai: &ChatCompletionRequest, node: &CandidateNode) -> AppResult<reqwest::RequestBuilder> {
        let url = format!("{}/v1/messages", node.endpoint.trim_end_matches('/'));
        let client = crate::backend_adapters::http_client();

        let mut system: Vec<String> = Vec::new();
        let mut messages: Vec<Value> = Vec::new();
        // Anthropic 要求同角色消息必须合并为一条（连续 user / 连续 assistant 会 400）
        let push_msg = |role: &str, blocks: Vec<Value>, messages: &mut Vec<Value>| {
            if blocks.is_empty() { return; }
            match messages.last_mut() {
                Some(last) if last.get("role").and_then(|x| x.as_str()) == Some(role) => {
                    if let Some(arr) = last.get_mut("content").and_then(|x| x.as_array_mut()) {
                        arr.extend(blocks);
                    }
                }
                _ => messages.push(json!({"role": role, "content": blocks})),
            }
        };
        for m in &oai.messages {
            if m.role == "system" {
                if let MessageContent::Text(s) = &m.content { system.push(s.clone()); }
                continue;
            }
            if m.role == "tool" {
                // 工具结果 → tool_result 块，绑定到 tool_call_id
                let text = match &m.content { MessageContent::Text(s) => s.clone(), MessageContent::MultiPart(ps) => ps.iter().filter_map(|p| p.text.clone()).collect::<Vec<_>>().join("") };
                let blocks = vec![json!({"type": "tool_result", "tool_use_id": m.tool_call_id.clone().unwrap_or_default(), "content": text})];
                push_msg("user", blocks, &mut messages);
                continue;
            }
            let blocks = to_anthropic_blocks(m);
            // 空 content 且无 tool_calls 的消息直接跳过（Anthropic 不接受空 content 块数组）
            let role = if m.role == "assistant" { "assistant" } else { "user" };
            push_msg(role, blocks, &mut messages);
        }

        let mut body = serde_json::Map::new();
        body.insert("model".into(), json!(oai.model));
        body.insert("max_tokens".into(), json!(oai.max_tokens.unwrap_or(4096)));
        if !system.is_empty() { body.insert("system".into(), json!(system.join("\n"))); }
        body.insert("messages".into(), json!(messages));
        body.insert("stream".into(), json!(oai.stream));
        if let Some(t) = oai.temperature { body.insert("temperature".into(), json!(t)); }
        if let Some(p) = oai.top_p { body.insert("top_p".into(), json!(p)); }
        // tools：完整转发（OpenAI function 形 → Anthropic {name, description, input_schema}）
        if let Some(tools) = &oai.tools {
            if !tools.is_empty() {
                let arr: Vec<Value> = tools.iter().map(|t| json!({
                    "name": t.function.name,
                    "description": t.function.description.clone().unwrap_or_default(),
                    "input_schema": if t.function.parameters.is_null() { json!({"type": "object", "properties": {}}) } else { t.function.parameters.clone() },
                })).collect();
                body.insert("tools".into(), json!(arr));
            }
        }
        if let Some(tc) = map_tool_choice(&oai.tool_choice) { body.insert("tool_choice".into(), tc); }

        let mut rb = client.post(&url)
            .header("x-api-key", &node._api_key)
            .header("anthropic-version", "2023-06-01")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&Value::Object(body));
        if !node._api_key.is_empty() {
            rb = rb.header(reqwest::header::AUTHORIZATION, format!("Bearer {}", node._api_key));
        }
        Ok(rb)
    }

    fn parse_response_body(&self, bytes: bytes::Bytes) -> AppResult<ChatCompletionResponse> {
        let v: Value = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::Labeled { label: ErrorLabel::JsonParseFail, message: format!("anthropic response parse fail: {e}") })?;
        let created = chrono::Utc::now().timestamp();
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("msg").to_string();
        let model = v.get("model").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let blocks: Vec<Value> = v.get("content").and_then(|x| x.as_array()).cloned().unwrap_or_default();
        let text = collect_text(&blocks);
        let tool_calls: Vec<ToolCall> = blocks.iter()
            .filter(|b| b.get("type").and_then(|x| x.as_str()) == Some("tool_use"))
            .filter_map(tool_call_from_block)
            .collect();
        let role = v.get("role").and_then(|x| x.as_str()).unwrap_or("assistant").to_string();
        let sr = v.get("stop_reason").and_then(|x| x.as_str()).unwrap_or("end_turn");
        let input = v.pointer("/usage/input_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let output = v.pointer("/usage/output_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let finish = match stop_reason_map(sr).as_deref() {
            // 有 tool_calls 但上游没给 tool_use stop_reason 时也要纠正为 tool_calls
            Some("tool_calls") => Some("tool_calls".to_string()),
            _ if !tool_calls.is_empty() => Some("tool_calls".to_string()),
            other => other.map(|s| s.to_string()),
        };
        let message = ChatMessage { role, content: MessageContent::Text(text), name: None, tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) }, tool_call_id: None, extra: serde_json::json!({}) };
        Ok(ChatCompletionResponse {
            id, object: "chat.completion", created, model,
            choices: vec![Choice { index: 0, message, finish_reason: finish }],
            usage: Some(Usage { prompt_tokens: input, completion_tokens: output, total_tokens: input + output, extra: json!({}) }),
            extra: json!({}),
        })
    }

    /// 把 Anthropic SSE 事件（data: {...}）逐条翻译为 OpenAI chunk
    fn translate_sse_chunk(&self, vendor_line: &str) -> AppResult<Option<SseChunk>> {
        let line = vendor_line.trim();
        if line.is_empty() || line.starts_with(':') || line.starts_with("event:") { return Ok(None); }
        let Some(data) = line.strip_prefix("data:") else { return Ok(None); };
        let data = data.trim();
        if data.is_empty() { return Ok(None); }
        let v: Value = serde_json::from_str(data)
            .map_err(|e| AppError::Labeled { label: ErrorLabel::JsonParseFail, message: format!("anthropic sse parse fail: {e}") })?;
        let kind = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
        let created = chrono::Utc::now().timestamp();
        match kind {
            "message_start" => {
                let msg = v.get("message").cloned().unwrap_or_else(|| json!({}));
                let id = msg.get("id").and_then(|x| x.as_str()).unwrap_or("msg").to_string();
                let model = msg.get("model").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let role = msg.get("role").and_then(|x| x.as_str()).unwrap_or("assistant").to_string();
                // 透传 prompt 侧 usage（Anthropic 在 message_start 给 input_tokens）
                let prompt = msg.pointer("/usage/input_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                self.last_prompt.store(prompt, std::sync::atomic::Ordering::Relaxed);
                let usage = Some(Usage { prompt_tokens: prompt, completion_tokens: 0, total_tokens: prompt, extra: json!({}) });
                Ok(Some(SseChunk {
                    id: Some(id), object: Some("chat.completion.chunk"), created: Some(created), model: Some(model),
                    choices: vec![SseChoice { index: 0, delta: Some(ChatMessage { role, content: MessageContent::Text(String::new()), name: None, tool_calls: None, tool_call_id: None, extra: json!({}) }), finish_reason: None }],
                    usage,
                }))
            }
            "content_block_start" => {
                // tool_use 块开始 → 发出该 tool_call 的"头部" delta（id + name + 空 arguments）
                let cb = v.get("content_block").cloned().unwrap_or_else(|| json!({}));
                if cb.get("type").and_then(|x| x.as_str()) != Some("tool_use") { return Ok(None); }
                let anth_idx = v.get("index").and_then(|x| x.as_u64()).unwrap_or(0);
                let seq = {
                    let mut map = self.tool_idx_map.lock().unwrap_or_else(|p| p.into_inner());
                    let next = self.tool_seq.load(std::sync::atomic::Ordering::Relaxed);
                    *map.entry(anth_idx).or_insert_with(|| {
                        self.tool_seq.store(next + 1, std::sync::atomic::Ordering::Relaxed);
                        next
                    })
                };
                let tc = ToolCall {
                    id: cb.get("id").and_then(|x| x.as_str()).map(String::from),
                    kind: Some("function".into()),
                    index: Some(seq),
                    function: Some(ToolCallFn {
                        name: cb.get("name").and_then(|x| x.as_str()).map(String::from),
                        arguments: Some(String::new()),
                    }),
                };
                let d = ChatMessage { role: "assistant".into(), content: MessageContent::Text(String::new()), name: None, tool_calls: Some(vec![tc]), tool_call_id: None, extra: json!({}) };
                Ok(Some(SseChunk { id: None, object: Some("chat.completion.chunk"), created: Some(created), model: None, choices: vec![SseChoice { index: 0, delta: Some(d), finish_reason: None }], usage: None }))
            }
            "content_block_delta" => {
                let delta = v.get("delta").cloned().unwrap_or_else(|| json!({}));
                let dtype = delta.get("type").and_then(|x| x.as_str()).unwrap_or("");
                if dtype == "input_json_delta" {
                    // 参数增量片段 → delta.tool_calls[index].function.arguments
                    let anth_idx = v.get("index").and_then(|x| x.as_u64()).unwrap_or(0);
                    let seq = self.tool_idx_map.lock().unwrap_or_else(|p| p.into_inner()).get(&anth_idx).copied().unwrap_or(0);
                    let piece = delta.get("partial_json").and_then(|x| x.as_str()).unwrap_or("");
                    if piece.is_empty() { return Ok(None); }
                    let tc = ToolCall { id: None, kind: None, index: Some(seq), function: Some(ToolCallFn { name: None, arguments: Some(piece.to_string()) }) };
                    let d = ChatMessage { role: "assistant".into(), content: MessageContent::Text(String::new()), name: None, tool_calls: Some(vec![tc]), tool_call_id: None, extra: json!({}) };
                    return Ok(Some(SseChunk { id: None, object: Some("chat.completion.chunk"), created: Some(created), model: None, choices: vec![SseChoice { index: 0, delta: Some(d), finish_reason: None }], usage: None }));
                }
                if dtype == "thinking_delta" {
                    // 思维链增量 → 透传为 reasoning_content（DeepSeek 风格扩展字段，extra flatten）
                    let th = delta.get("thinking").and_then(|x| x.as_str()).unwrap_or("");
                    if th.is_empty() { return Ok(None); }
                    let d = ChatMessage { role: "assistant".into(), content: MessageContent::Text(String::new()), name: None, tool_calls: None, tool_call_id: None, extra: json!({"reasoning_content": th}) };
                    return Ok(Some(SseChunk { id: None, object: Some("chat.completion.chunk"), created: Some(created), model: None, choices: vec![SseChoice { index: 0, delta: Some(d), finish_reason: None }], usage: None }));
                }
                // signature_delta / citations_delta 等其他增量类型：仅提取 text（有则转发，无则丢弃空 chunk）
                let text = delta.get("text").and_then(|x| x.as_str()).unwrap_or("").to_string();
                if text.is_empty() { return Ok(None); }
                let d = Some(ChatMessage { role: "assistant".into(), content: MessageContent::Text(text), name: None, tool_calls: None, tool_call_id: None, extra: json!({}) });
                Ok(Some(SseChunk { id: None, object: Some("chat.completion.chunk"), created: Some(created), model: None, choices: vec![SseChoice { index: 0, delta: d, finish_reason: None }], usage: None }))
            }
            "message_delta" => {
                let sr = v.pointer("/delta/stop_reason").and_then(|x| x.as_str()).unwrap_or("end_turn");
                // Anthropic 在 message_delta 给累计 output_tokens；prompt 用 message_start 记录的值回填
                let p = self.last_prompt.load(std::sync::atomic::Ordering::Relaxed);
                let o = v.pointer("/usage/output_tokens").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                let usage = Some(Usage { prompt_tokens: p, completion_tokens: o, total_tokens: p + o, extra: json!({}) });
                Ok(Some(SseChunk { id: None, object: Some("chat.completion.chunk"), created: Some(created), model: None, choices: vec![SseChoice { index: 0, delta: None, finish_reason: stop_reason_map(sr) }], usage }))
            }
            "message_stop" | "content_block_stop" | "ping" => Ok(None),
            "error" => {
                // 上游中途出错：不吞掉，转为网关错误（消息只含上游错误类型，不含端点/token）
                let emsg = v.pointer("/error/message").and_then(|x| x.as_str()).unwrap_or("upstream stream error");
                Err(AppError::Labeled { label: ErrorLabel::Upstream5xx, message: format!("upstream stream error: {emsg}") })
            }
            _ => Ok(None),
        }
    }
}
