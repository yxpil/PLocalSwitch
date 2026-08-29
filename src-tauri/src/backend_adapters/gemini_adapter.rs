//! Google Gemini generateContent 适配器（非流式 :generateContent / 流式 :streamGenerateContent?alt=sse）
//!
//! 请求：{contents:[{role:"user"|"model", parts:[{text}]}], systemInstruction:{parts:[{text}]}, generationConfig:{temperature, topP, maxOutputTokens}}
//! 响应：{candidates:[{content:{parts:[{text}], role}, finishReason}], usageMetadata:{...}}
//! 流式：data: {candidates:[{content:{parts:[{text}]}, finishReason}]} （chunked JSON via SSE）
use async_trait::async_trait;
use crate::error::{AppError, AppResult, ErrorLabel};
use crate::models::{ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, MessageContent, SseChunk, SseChoice, ToolCall, ToolCallFn, ToolChoice, Usage};
use crate::router::CandidateNode;
use serde_json::{json, Value};

#[derive(Default)]
pub struct GeminiAdapter {
    /// 流内 functionCall part 的连续序号（Gemini 流式可能分多个 functionCall）
    tool_seq: std::sync::atomic::AtomicU32,
}

/// OpenAI tool_choice → Gemini toolConfig
fn map_tool_choice(tc: &Option<ToolChoice>) -> Option<Value> {
    match tc {
        None => None,
        Some(ToolChoice::Str(s)) => {
            let mode = match s.as_str() {
                "none" => "NONE",
                "required" => "ANY",
                _ => "AUTO",
            };
            Some(json!({"functionCallingConfig": {"mode": mode}}))
        }
        Some(ToolChoice::Obj(v)) => {
            if v.get("type").and_then(|x| x.as_str()) == Some("function") {
                let name = v.pointer("/function/name").and_then(|x| x.as_str()).unwrap_or("");
                Some(json!({"functionCallingConfig": {"mode": "ANY", "allowedFunctionNames": [name]}}))
            } else { None }
        }
    }
}

/// Gemini functionCall part → OpenAI ToolCall
fn tool_call_from_fc(fc: &Value, index: Option<u32>) -> ToolCall {
    ToolCall {
        id: fc.get("id").and_then(|x| x.as_str()).map(String::from),
        kind: Some("function".into()),
        index,
        function: Some(ToolCallFn {
            name: fc.get("name").and_then(|x| x.as_str()).map(String::from),
            arguments: Some(fc.get("args").cloned().unwrap_or_else(|| json!({})).to_string()),
        }),
    }
}

fn gemini_role(r: &str) -> &'static str { if r == "assistant" { "model" } else { "user" } }

fn map_finish(s: &str) -> Option<String> {
    if s.is_empty() { return None; }
    Some(match s {
        "MAX_TOKENS" => "length".to_string(),
        _ => "stop".to_string(),
    })
}

/// 从 Gemini content（parts）里抽文本
fn parts_text(v: &Value) -> String {
    v.get("parts")
        .and_then(|p| p.as_array())
        .map(|arr| arr.iter().filter_map(|it| it.get("text").and_then(|x| x.as_str()).map(|s| s.to_string())).collect::<Vec<_>>().join(""))
        .unwrap_or_default()
}

#[async_trait]
impl crate::backend_adapters::BackendAdapter for GeminiAdapter {
    fn protocol(&self) -> crate::router::ProtocolKind { crate::router::ProtocolKind::Gemini }

    async fn translate_request(&self, oai: &ChatCompletionRequest, node: &CandidateNode) -> AppResult<reqwest::RequestBuilder> {
        let base = node.endpoint.trim_end_matches('/');
        // model 名可能带 models/ 前缀；统一去掉，只保留模型名
        let model = oai.model.strip_prefix("models/").unwrap_or(&oai.model);
        let model = model.replace(' ', "");
        let url = if oai.stream {
            format!("{base}/v1beta/models/{model}:streamGenerateContent?alt=sse")
        } else {
            format!("{base}/v1beta/models/{model}:generateContent")
        };
        let client = crate::backend_adapters::http_client();

        let mut contents: Vec<Value> = Vec::new();
        let mut system_parts: Vec<Value> = Vec::new();
        for m in &oai.messages {
            if m.role == "system" {
                if let MessageContent::Text(s) = &m.content {
                    system_parts.push(json!({"text": s}));
                }
                continue;
            }
            // tool 角色 → functionResponse part（结果回填给模型）
            if m.role == "tool" {
                let out = match &m.content { MessageContent::Text(s) => s.clone(), MessageContent::MultiPart(ps) => ps.iter().filter_map(|p| p.text.clone()).collect::<Vec<_>>().join("") };
                let parsed: Value = serde_json::from_str(&out).unwrap_or_else(|_| json!({"result": out}));
                contents.push(json!({"role": "user", "parts": [{"functionResponse": {"name": m.name.clone().unwrap_or_default(), "response": parsed}}]}));
                continue;
            }
            let mut parts: Vec<Value> = Vec::new();
            match &m.content {
                MessageContent::Text(s) => { if !s.is_empty() { parts.push(json!({"text": s})); } }
                MessageContent::MultiPart(ps) => {
                    for p in ps {
                        if p.kind == "image_url" {
                            if let Some(img) = &p.image_url {
                                if let Some(rest) = img.url.strip_prefix("data:") {
                                    if let Some((mime, b64)) = rest.split_once(',') {
                                        parts.push(json!({"inlineData": {"mimeType": mime.trim_end_matches(";base64"), "data": b64}}));
                                    }
                                }
                            }
                        } else if p.kind == "input_audio" {
                            // 音频 → Gemini inlineData（base64）/ fileData（URL）
                            if let Some(a) = &p.audio {
                                let mime = a.mime_type.clone()
                                    .unwrap_or_else(|| format!("audio/{}", a.format.clone().unwrap_or_else(|| "wav".into())));
                                if let Some(d) = &a.data {
                                    parts.push(json!({"inlineData": {"mimeType": mime, "data": d}}));
                                } else if let Some(u) = &a.url {
                                    parts.push(json!({"fileData": {"fileUri": u, "mimeType": mime}}));
                                }
                            }
                        } else if p.kind == "document" || p.kind == "file" {
                            // 文档/文件 → Gemini inlineData（base64）/ fileData（URL）
                            if let Some(f) = &p.file {
                                let mime = f.mime_type.clone().unwrap_or_else(|| "application/pdf".into());
                                if let Some(d) = &f.data {
                                    parts.push(json!({"inlineData": {"mimeType": mime, "data": d}}));
                                } else if let Some(u) = &f.url {
                                    parts.push(json!({"fileData": {"fileUri": u, "mimeType": mime}}));
                                }
                            }
                        } else if let Some(t) = &p.text {
                            if !t.is_empty() { parts.push(json!({"text": t})); }
                        }
                    }
                }
            }
            // assistant tool_calls → functionCall parts（完整转发）
            if let Some(tcs) = &m.tool_calls {
                for tc in tcs {
                    let name = tc.function.as_ref().and_then(|f| f.name.clone()).unwrap_or_default();
                    let args: Value = tc.function.as_ref().and_then(|f| f.arguments.clone())
                        .and_then(|a| serde_json::from_str(&a).ok()).unwrap_or_else(|| json!({}));
                    parts.push(json!({"functionCall": {"name": name, "args": args}}));
                }
            }
            if parts.is_empty() { continue; }
            contents.push(json!({"role": gemini_role(&m.role), "parts": parts}));
        }

        let mut gen: serde_json::Map<String, Value> = serde_json::Map::new();
        if let Some(t) = oai.temperature { gen.insert("temperature".into(), json!(t)); }
        if let Some(p) = oai.top_p { gen.insert("topP".into(), json!(p)); }
        if let Some(m) = oai.max_tokens { gen.insert("maxOutputTokens".into(), json!(m)); }

        let mut body = serde_json::Map::new();
        body.insert("contents".into(), json!(contents));
        if !system_parts.is_empty() { body.insert("systemInstruction".into(), json!({"parts": system_parts})); }
        if !gen.is_empty() { body.insert("generationConfig".into(), Value::Object(gen)); }
        // tools：OpenAI function 形 → Gemini {functionDeclarations:[{name,description,parameters}]}
        if let Some(tools) = &oai.tools {
            if !tools.is_empty() {
                let decls: Vec<Value> = tools.iter().map(|t| json!({
                    "name": t.function.name,
                    "description": t.function.description.clone().unwrap_or_default(),
                    "parameters": if t.function.parameters.is_null() { json!({"type":"object","properties":{}}) } else { t.function.parameters.clone() },
                })).collect();
                body.insert("tools".into(), json!([{"functionDeclarations": decls}]));
            }
            if let Some(tc) = map_tool_choice(&oai.tool_choice) { body.insert("toolConfig".into(), tc); }
        }

        let mut rb = client.post(&url)
            .header("x-goog-api-key", &node._api_key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&Value::Object(body));
        if !node._api_key.is_empty() {
            rb = rb.header(reqwest::header::AUTHORIZATION, format!("Bearer {}", node._api_key));
        }
        Ok(rb)
    }

    fn parse_response_body(&self, bytes: bytes::Bytes) -> AppResult<ChatCompletionResponse> {
        let v: Value = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::Labeled { label: ErrorLabel::JsonParseFail, message: format!("gemini response parse fail: {e}") })?;
        let created = chrono::Utc::now().timestamp();
        let cand = v.get("candidates").and_then(|c| c.get(0)).cloned().unwrap_or_else(|| json!({}));
        let model = v.get("modelVersion").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let content = cand.get("content").cloned().unwrap_or_else(|| json!({}));
        let text = parts_text(&content);
        // parts[].functionCall → tool_calls（完整还原）
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        if let Some(parts) = content.get("parts").and_then(|p| p.as_array()) {
            for p in parts {
                if let Some(fc) = p.get("functionCall") {
                    tool_calls.push(tool_call_from_fc(fc, Some(tool_calls.len() as u32)));
                }
            }
        }
        let fr = cand.get("finishReason").and_then(|x| x.as_str()).unwrap_or("");
        let finish = if !tool_calls.is_empty() { Some("tool_calls".to_string()) } else { map_finish(fr) };
        let input = v.pointer("/usageMetadata/promptTokenCount").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let output = v.pointer("/usageMetadata/candidatesTokenCount").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let total = v.pointer("/usageMetadata/totalTokenCount").and_then(|x| x.as_u64()).unwrap_or((input + output) as u64) as u32;
        let message = ChatMessage { role: "assistant".into(), content: MessageContent::Text(text), name: None, tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) }, tool_call_id: None, extra: json!({}) };
        Ok(ChatCompletionResponse {
            id: format!("gemini-{created}"), object: "chat.completion", created, model,
            choices: vec![Choice { index: 0, message, finish_reason: finish }],
            usage: Some(Usage { prompt_tokens: input, completion_tokens: output, total_tokens: total, extra: json!({}) }),
            extra: json!({}),
        })
    }

    /// Gemini SSE（alt=sse 返回 data: {...}）逐条翻译为 OpenAI chunk
    fn translate_sse_chunk(&self, vendor_line: &str) -> AppResult<Option<SseChunk>> {
        let line = vendor_line.trim();
        if line.is_empty() || line.starts_with(':') { return Ok(None); }
        let Some(data) = line.strip_prefix("data:") else { return Ok(None); };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" { return Ok(None); }
        let v: Value = serde_json::from_str(data)
            .map_err(|e| AppError::Labeled { label: ErrorLabel::JsonParseFail, message: format!("gemini sse parse fail: {e}") })?;
        if v.get("error").is_some() {
            return Err(AppError::Labeled { label: ErrorLabel::Upstream5xx, message: format!("gemini sse error: {data}") });
        }
        let cand = v.get("candidates").and_then(|c| c.get(0)).cloned().unwrap_or_else(|| json!({}));
        let content = cand.get("content").cloned().unwrap_or_else(|| json!({}));
        let text = parts_text(&content);
        // functionCall parts → delta.tool_calls（序号在本流内保持连续）
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        if let Some(parts) = content.get("parts").and_then(|p| p.as_array()) {
            for p in parts {
                if let Some(fc) = p.get("functionCall") {
                    let seq = self.tool_seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    tool_calls.push(tool_call_from_fc(fc, Some(seq)));
                }
            }
        }
        let fr = cand.get("finishReason").and_then(|x| x.as_str()).unwrap_or("");
        let finish = if !fr.is_empty() && !tool_calls.is_empty() { Some("tool_calls".to_string()) } else { map_finish(fr) };
        let created = chrono::Utc::now().timestamp();
        let model = v.get("modelVersion").and_then(|x| x.as_str()).map(|s| s.to_string());
        let d = if text.is_empty() && tool_calls.is_empty() {
            None
        } else {
            Some(ChatMessage { role: "assistant".into(), content: MessageContent::Text(text), name: None, tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) }, tool_call_id: None, extra: json!({}) })
        };
        Ok(Some(SseChunk {
            id: Some(format!("gemini-{created}")), object: Some("chat.completion.chunk"), created: Some(created), model,
            choices: vec![SseChoice { index: 0, delta: d, finish_reason: finish }],
            usage: None,
        }))
    }
}
