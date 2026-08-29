//! =============================================================
//!  入站协议自动识别 + 归一化（gw8a 新增 · 协议无感中转核心）
//! =============================================================
//!  设计：
//!   • 客户端可以用 OpenAI / Anthropic / Gemini 任意格式打进来
//!   • 识别顺序：路径特征（快且无歧义）→ body 字段形状嗅探（打分制，同出站 sniffer 一致）
//!   • 归一化到内部统一 IR = models::ChatCompletionRequest
//!     → 复用同一条 路由 / 缓存 / 计费 / 质量 / 柔性出站 链路
//!   • 响应按客户端协议反归一化（denormalize）
//!   • 打分不确定时 fallback OpenAI；管理端可通过 header `x-pls-protocol` 强制指定
//! =============================================================
use crate::error::{AppError, AppResult, ErrorLabel};
use crate::models::{ChatCompletionRequest, ChatMessage, ContentPart, ImageUrl, MessageContent, ToolDef, FunctionDef, ToolCall, ToolCallFn, ToolChoice};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum InboundProtocol {
    OpenAI,
    Anthropic,
    Gemini,
    Ollama,
}

// ----------------------------------------------------------------
// 1) 路径特征识别（优先，快且无歧义）
// ----------------------------------------------------------------
pub fn detect_path(path_lower: &str) -> Option<InboundProtocol> {
    // Ollama 原生客户端：POST /api/chat
    if path_lower.ends_with("/api/chat") || path_lower.ends_with("/api/generate") {
        return Some(InboundProtocol::Ollama);
    }
    // Anthropic SDK 标准：POST /v1/messages
    if path_lower.ends_with("/v1/messages") || path_lower.contains("/anthropic/") {
        return Some(InboundProtocol::Anthropic);
    }
    // Gemini SDK 标准：POST /v1beta/models/{model}:generateContent
    if path_lower.contains(":generatecontent") || path_lower.contains("/gemini/") || path_lower.contains("/v1beta/") {
        return Some(InboundProtocol::Gemini);
    }
    if path_lower.ends_with("/v1/chat/completions") || path_lower.contains("/openai/") {
        return Some(InboundProtocol::OpenAI);
    }
    None
}

// ----------------------------------------------------------------
// 2) body 形状嗅探（打分制，路径识别不出时使用）
// ----------------------------------------------------------------
pub fn sniff_body(body: &Value) -> (InboundProtocol, u8) {
    let mut oai: u8 = 0;
    let mut ant: u8 = 0;
    let mut gem: u8 = 0;
    let mut ollama: u8 = 0;

    if body.get("contents").and_then(|v| v.as_array()).is_some() { gem += 60; }
    if body.get("parts").is_some() { gem += 20; }
    if body.get("generationConfig").is_some() { gem += 30; }
    if body.get("systemInstruction").is_some() { gem += 30; }

    if body.get("messages").and_then(|v| v.as_array()).is_some() {
        oai += 30;
        ant += 40; // Anthropic 也用 messages
        ollama += 30; // Ollama 也用 messages
    }
    // Ollama 特征：顶层 options 布尔/选项；或 message 里带 tool_calls；或 stream 为布尔
    if body.get("options").is_some() { ollama += 40; }
    if body.get("keep_alive").is_some() { ollama += 25; }
    if body.get("format").is_some() { ollama += 15; }
    // Anthropic 强制 max_tokens；OpenAI 可选
    if body.get("max_tokens").is_some() { ant += 30; }
    // Anthropic system 通常是顶层字符串；OpenAI 是 messages 里的 role=system
    if body.get("system").and_then(|v| v.as_str()).is_some() { ant += 30; }
    // Anthropic tools: {name, input_schema}；OpenAI tools: [{type:"function", function:{...}}]；Ollama 同 OpenAI function 形
    if let Some(tools) = body.get("tools").and_then(|v| v.as_array()) {
        if tools.iter().any(|t| t.get("input_schema").is_some()) { ant += 40; }
        if tools.iter().any(|t| t.get("function").is_some()) { oai += 40; ollama += 40; }
    }
    if body.get("model").is_some() { oai += 25; ant += 10; ollama += 20; }
    if body.get("stream").is_some() { oai += 5; ant += 5; ollama += 5; }

    let best = [(InboundProtocol::OpenAI, oai), (InboundProtocol::Anthropic, ant),
                (InboundProtocol::Gemini, gem), (InboundProtocol::Ollama, ollama)]
        .into_iter()
        .max_by_key(|(_, s)| *s)
        .unwrap_or((InboundProtocol::OpenAI, 0));
    best
}

// ----------------------------------------------------------------
// 3) 归一化：客户端协议 → 内部 IR (ChatCompletionRequest)
// ----------------------------------------------------------------
pub fn normalize_request(
    proto: InboundProtocol,
    body: &Value,
    model_hint: Option<&str>,   // Gemini 从 URL path 提取 model
) -> AppResult<ChatCompletionRequest> {
    match proto {
        InboundProtocol::OpenAI => serde_json::from_value(body.clone())
            .map_err(|e| AppError::Labeled { label: ErrorLabel::BadParam4xx, message: format!("openai body invalid: {e}") }),
        InboundProtocol::Anthropic => normalize_anthropic(body),
        InboundProtocol::Gemini => normalize_gemini(body, model_hint),
        InboundProtocol::Ollama => normalize_ollama(body),
    }
}

/// Anthropic Messages → 内部 IR
fn normalize_anthropic(b: &Value) -> AppResult<ChatCompletionRequest> {
    let model = b.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
    let mut messages: Vec<ChatMessage> = Vec::new();

    // system（字符串或 blocks）→ 首条 system 消息
    if let Some(sys) = b.get("system") {
        let text = match sys {
            Value::String(s) => s.clone(),
            Value::Array(parts) => parts.iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>().join("\n"),
            _ => String::new(),
        };
        if !text.is_empty() {
            messages.push(ChatMessage { role: "system".into(), content: MessageContent::Text(text), name: None, tool_calls: None, tool_call_id: None, extra: serde_json::json!({}) });
        }
    }

    if let Some(msgs) = b.get("messages").and_then(|v| v.as_array()) {
        for m in msgs {
            let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("user").to_string();
            // tool_use 块 → OpenAI tool_calls；tool_result 块 → 独立 tool 消息（宁滥不缺：完整还原，不摊平）
            let mut tool_calls: Vec<ToolCall> = Vec::new();
            let mut tool_results: Vec<(String, String)> = Vec::new(); // (tool_use_id, text)
            let content = match m.get("content") {
                Some(Value::String(s)) => MessageContent::Text(s.clone()),
                Some(Value::Array(parts)) => {
                    let mut cps: Vec<ContentPart> = Vec::new();
                    for p in parts {
                        let kind = p.get("type").and_then(|v| v.as_str()).unwrap_or("text");
                        match kind {
                            "text" => cps.push(ContentPart {
                                kind: "text".into(),
                                text: Some(p.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string()),
                                image_url: None,
                            }),
                            "image" => {
                                // Anthropic image.source {data(base64), media_type} → data URL
                                let src = p.get("source");
                                let data = src.and_then(|s| s.get("data")).and_then(|v| v.as_str()).unwrap_or("");
                                let mime = src.and_then(|s| s.get("media_type")).and_then(|v| v.as_str()).unwrap_or("image/png");
                                cps.push(ContentPart {
                                    kind: "image_url".into(),
                                    text: None,
                                    image_url: Some(ImageUrl { url: format!("data:{mime};base64,{data}"), detail: None }),
                                });
                            }
                            "tool_use" => {
                                let args = p.get("input").cloned().unwrap_or_else(|| json!({}));
                                tool_calls.push(ToolCall {
                                    id: p.get("id").and_then(|v| v.as_str()).map(String::from),
                                    kind: Some("function".into()),
                                    index: None,
                                    function: Some(ToolCallFn {
                                        name: p.get("name").and_then(|v| v.as_str()).map(String::from),
                                        arguments: Some(args.to_string()),
                                    }),
                                });
                            }
                            "tool_result" => {
                                // content 可能是字符串或 blocks 数组
                                let text = match p.get("content") {
                                    Some(Value::String(s)) => s.clone(),
                                    Some(Value::Array(arr)) => arr.iter()
                                        .filter_map(|x| x.get("text").and_then(|t| t.as_str()))
                                        .collect::<Vec<_>>().join(""),
                                    other => other.map(|v| v.to_string()).unwrap_or_default(),
                                };
                                tool_results.push((p.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("").to_string(), text));
                            }
                            _ => {}
                        }
                    }
                    MessageContent::MultiPart(cps)
                }
                _ => MessageContent::Text(String::new()),
            };
            if !tool_calls.is_empty() || !matches!(content, MessageContent::MultiPart(ref v) if v.is_empty()) || matches!(content, MessageContent::Text(ref s) if !s.is_empty()) {
                messages.push(ChatMessage { role: role.clone(), content, name: None, tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) }, tool_call_id: None, extra: serde_json::json!({}) });
            }
            // tool_result 拆成独立 tool 消息（顺序保持在当前消息之后）
            for (id, text) in tool_results {
                messages.push(ChatMessage { role: "tool".into(), content: MessageContent::Text(text), name: None, tool_calls: None, tool_call_id: Some(id), extra: serde_json::json!({}) });
            }
        }
    }

    // tools: {name, description, input_schema} → OpenAI function 形态
    let tools = b.get("tools").and_then(|v| v.as_array()).map(|ts| {
        ts.iter().filter_map(|t| {
            Some(ToolDef {
                kind: "function".into(),
                function: FunctionDef {
                    name: t.get("name").and_then(|v| v.as_str())?.to_string(),
                    description: t.get("description").and_then(|v| v.as_str()).map(String::from),
                    parameters: t.get("input_schema").cloned().unwrap_or(Value::Null),
                },
            })
        }).collect::<Vec<_>>()
    });

    // tool_choice: {type:auto|any|tool}
    let tool_choice = b.get("tool_choice").and_then(|tc| tc.get("type")).and_then(|v| v.as_str()).map(|t| match t {
        "auto" => ToolChoice::Str("auto".into()),
        "any"  => ToolChoice::Str("required".into()),
        "tool" => {
            let name = b.pointer("/tool_choice/name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            ToolChoice::Obj(json!({"type": "function", "function": {"name": name}}))
        }
        _ => ToolChoice::Str("auto".into()),
    });

    Ok(ChatCompletionRequest {
        model,
        messages,
        stream: b.get("stream").and_then(|v| v.as_bool()).unwrap_or(false),
        temperature: b.get("temperature").and_then(|v| v.as_f64()).map(|f| f as f32),
        top_p: b.get("top_p").and_then(|v| v.as_f64()).map(|f| f as f32),
        max_tokens: b.get("max_tokens").and_then(|v| v.as_u64()).map(|v| v.min(u32::MAX as u64) as u32),
        response_format: None,
        tools: tools.filter(|t| !t.is_empty()),
        tool_choice,
        extras: Value::Null,
    })
}

/// Gemini generateContent → 内部 IR
fn normalize_gemini(b: &Value, model_hint: Option<&str>) -> AppResult<ChatCompletionRequest> {
    let model = model_hint
        .or_else(|| b.get("model").and_then(|v| v.as_str()))
        .unwrap_or("unknown").to_string();
    let mut messages: Vec<ChatMessage> = Vec::new();

    // systemInstruction: {parts:[{text}]}
    if let Some(sys) = b.get("systemInstruction") {
        let text = sys.pointer("/parts/0/text").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if !text.is_empty() {
            messages.push(ChatMessage { role: "system".into(), content: MessageContent::Text(text), name: None, tool_calls: None, tool_call_id: None, extra: serde_json::json!({}) });
        }
    }

    if let Some(contents) = b.get("contents").and_then(|v| v.as_array()) {
        for c in contents {
            let raw_role = c.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let role = if raw_role == "model" { "assistant" } else { raw_role }.to_string();
            let mut parts: Vec<ContentPart> = Vec::new();
            if let Some(ps) = c.get("parts").and_then(|v| v.as_array()) {
                for p in ps {
                    if let Some(t) = p.get("text").and_then(|v| v.as_str()) {
                        parts.push(ContentPart { kind: "text".into(), text: Some(t.to_string()), image_url: None });
                    } else if let Some(inline) = p.get("inlineData") {
                        let mime = inline.get("mimeType").and_then(|v| v.as_str()).unwrap_or("image/png");
                        let data = inline.get("data").and_then(|v| v.as_str()).unwrap_or("");
                        parts.push(ContentPart {
                            kind: "image_url".into(),
                            text: None,
                            image_url: Some(ImageUrl { url: format!("data:{mime};base64,{data}"), detail: None }),
                        });
                    }
                }
            }
            let content = if parts.len() == 1 && parts[0].kind == "text" {
                MessageContent::Text(parts[0].text.clone().unwrap_or_default())
            } else {
                MessageContent::MultiPart(parts)
            };
            messages.push(ChatMessage { role, content, name: None, tool_calls: None, tool_call_id: None, extra: serde_json::json!({}) });
        }
    }

    let gen = b.get("generationConfig");
    Ok(ChatCompletionRequest {
        model,
        messages,
        stream: gen.and_then(|g| g.get("stream")).and_then(|v| v.as_bool()).unwrap_or(false),
        temperature: gen.and_then(|g| g.get("temperature")).and_then(|v| v.as_f64()).map(|f| f as f32),
        top_p: gen.and_then(|g| g.get("topP")).and_then(|v| v.as_f64()).map(|f| f as f32),
        max_tokens: gen.and_then(|g| g.get("maxOutputTokens")).and_then(|v| v.as_u64()).map(|v| v.min(u32::MAX as u64) as u32),
        response_format: None,
        tools: None,
        tool_choice: None,
        extras: Value::Null,
    })
}

/// Ollama /api/chat → 内部 IR（Ollama 客户端生态如 Continue / open-webui 等直接对接）
fn normalize_ollama(b: &Value) -> AppResult<ChatCompletionRequest> {
    let model = b.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
    let mut messages: Vec<ChatMessage> = Vec::new();
    if let Some(msgs) = b.get("messages").and_then(|v| v.as_array()) {
        for m in msgs {
            let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("user").to_string();
            let text = match m.get("content") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Array(arr)) => arr.iter()
                    .filter_map(|x| x.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>().join(""),
                other => other.map(|v| v.to_string()).unwrap_or_default(),
            };
            // Ollama 的 tool（角色）消息 content 是 tool 结果文本；assistant 可带 tool_calls
            let tool_calls = m.get("tool_calls").and_then(|v| v.as_array()).map(|tcs| {
                tcs.iter().filter_map(|tc| {
                    let f = tc.get("function")?;
                    Some(ToolCall {
                        id: tc.get("id").and_then(|v| v.as_str()).map(String::from),
                        kind: Some("function".into()),
                        index: None,
                        function: Some(ToolCallFn {
                            name: f.get("name").and_then(|v| v.as_str()).map(String::from),
                            arguments: Some(f.get("arguments").cloned().unwrap_or_else(|| json!({})).to_string()),
                        }),
                    })
                }).collect::<Vec<_>>()
            });
            let is_tool = role == "tool";
            messages.push(ChatMessage {
                role: if is_tool { "tool".into() } else { role },
                content: MessageContent::Text(text),
                name: None,
                tool_calls: tool_calls.filter(|t| !t.is_empty()),
                tool_call_id: if is_tool { m.get("tool_call_id").and_then(|v| v.as_str()).map(String::from) } else { None },
                extra: serde_json::json!({}),
            });
        }
    }
    // options: {temperature, top_p, num_predict} / 顶层 temperature/top_p
    let opts = b.get("options").unwrap_or(&Value::Null);
    let opt_get = |k: &str| opts.get(k).or_else(|| b.get(k));
    // tools：Ollama 用 OpenAI function 形 {type:"function", function:{name,description,parameters}} → 直接解析为 OpenAI 形
    let tools = b.get("tools").and_then(|v| v.as_array()).map(|ts| {
        ts.iter().filter_map(|t| {
            Some(ToolDef {
                kind: "function".into(),
                function: FunctionDef {
                    name: t.pointer("/function/name").and_then(|v| v.as_str())?.to_string(),
                    description: t.pointer("/function/description").and_then(|v| v.as_str()).map(String::from),
                    parameters: t.pointer("/function/parameters").cloned().unwrap_or(Value::Null),
                },
            })
        }).collect::<Vec<_>>()
    });
    Ok(ChatCompletionRequest {
        model,
        messages,
        stream: b.get("stream").and_then(|v| v.as_bool()).unwrap_or(false),
        temperature: opt_get("temperature").and_then(|v| v.as_f64()).map(|f| f as f32),
        top_p: opt_get("top_p").and_then(|v| v.as_f64()).map(|f| f as f32),
        max_tokens: opt_get("num_predict").and_then(|v| v.as_u64()).map(|v| v.min(u32::MAX as u64) as u32),
        response_format: None,
        tools: tools.filter(|t| !t.is_empty()),
        tool_choice: None,
        extras: Value::Null,
    })
}

// ----------------------------------------------------------------
// 4) 反归一化：内部 OpenAI 形响应 → 客户端协议形
// ----------------------------------------------------------------
pub fn denormalize_response(proto: InboundProtocol, openai: &Value, model: &str) -> Value {
    match proto {
        InboundProtocol::OpenAI => openai.clone(),
        InboundProtocol::Anthropic => denorm_anthropic(openai, model),
        InboundProtocol::Gemini => denorm_gemini(openai, model),
        InboundProtocol::Ollama => denorm_ollama(openai, model),
    }
}

/// OpenAI 形响应 → Ollama /api/chat message 形
fn denorm_ollama(o: &Value, model: &str) -> Value {
    let choice0 = o.pointer("/choices/0");
    let text = choice0
        .and_then(|c| c.pointer("/message/content"))
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            other => Some(other.to_string()),
        })
        .unwrap_or_default();
    let mut msg = json!({ "role": "assistant", "content": text });
    // OpenAI tool_calls → Ollama tool_calls（arguments 是对象）
    if let Some(tcs) = choice0.and_then(|c| c.pointer("/message/tool_calls")).and_then(|v| v.as_array()) {
        let arr: Vec<Value> = tcs.iter().filter_map(|tc| {
            let name = tc.pointer("/function/name").and_then(|v| v.as_str())?;
            let args: Value = tc.pointer("/function/arguments").and_then(|v| v.as_str())
                .and_then(|s| serde_json::from_str(s).ok()).unwrap_or_else(|| json!({}));
            Some(json!({ "function": { "name": name, "arguments": args } }))
        }).collect();
        if !arr.is_empty() { msg["tool_calls"] = json!(arr); }
    }
    let done = choice0.and_then(|c| c.get("finish_reason")).and_then(|v| v.as_str()).unwrap_or("stop") == "stop";
    json!({
        "model": model,
        "message": msg,
        "done": done,
        "prompt_eval_count": o.pointer("/usage/prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        "eval_count": o.pointer("/usage/completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        "created_at": chrono::Utc::now().to_rfc3339(),
    })
}

fn denorm_anthropic(o: &Value, model: &str) -> Value {
    let choice0 = o.pointer("/choices/0");
    let text = choice0
        .and_then(|c| c.pointer("/message/content"))
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            other => Some(other.to_string()),
        })
        .unwrap_or_default();
    // OpenAI tool_calls → Anthropic tool_use blocks（完整还原）
    let mut blocks: Vec<Value> = Vec::new();
    if !text.is_empty() { blocks.push(json!({ "type": "text", "text": text })); }
    if let Some(tcs) = choice0.and_then(|c| c.pointer("/message/tool_calls")).and_then(|v| v.as_array()) {
        for tc in tcs {
            let args: Value = tc.pointer("/function/arguments").and_then(|v| v.as_str())
                .and_then(|s| serde_json::from_str(s).ok()).unwrap_or_else(|| json!({}));
            blocks.push(json!({
                "type": "tool_use",
                "id": tc.get("id").cloned().unwrap_or(Value::String(uuid_like())),
                "name": tc.pointer("/function/name").cloned().unwrap_or(Value::String(String::new())),
                "input": args,
            }));
        }
    }
    if blocks.is_empty() { blocks.push(json!({ "type": "text", "text": "" })); }
    let finish = choice0
        .and_then(|c| c.get("finish_reason"))
        .and_then(|v| v.as_str())
        .unwrap_or("stop");
    let stop_reason = match finish {
        "length"    => "max_tokens",
        "tool_calls" => "tool_use",
        _           => "end_turn",
    };
    let inp = o.pointer("/usage/prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let out = o.pointer("/usage/completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    json!({
        "id": o.get("id").cloned().unwrap_or(Value::String(uuid_like())),
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": blocks,
        "stop_reason": stop_reason,
        "stop_sequence": Value::Null,
        "usage": { "input_tokens": inp, "output_tokens": out }
    })
}

fn denorm_gemini(o: &Value, model: &str) -> Value {
    let choice0 = o.pointer("/choices/0");
    let text = choice0
        .and_then(|c| c.pointer("/message/content"))
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            other => Some(other.to_string()),
        })
        .unwrap_or_default();
    let finish = choice0
        .and_then(|c| c.get("finish_reason"))
        .and_then(|v| v.as_str())
        .unwrap_or("stop");
    let fr = match finish {
        "length" => "MAX_TOKENS",
        _        => "STOP",
    };
    let inp = o.pointer("/usage/prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let out = o.pointer("/usage/completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    json!({
        "candidates": [{
            "content": { "role": "model", "parts": [{ "text": text }] },
            "finishReason": fr,
            "index": 0
        }],
        "usageMetadata": {
            "promptTokenCount": inp,
            "candidatesTokenCount": out,
            "totalTokenCount": inp + out
        },
        "modelVersion": model
    })
}

fn uuid_like() -> String {
    // 轻量 id（避免引入额外依赖）：时间戳 + 随机片
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("msg_{ts:032x}")
}

// ----------------------------------------------------------------
// 5) 错误体反归一化（客户端协议形错误响应）
// ----------------------------------------------------------------
pub fn error_body(proto: InboundProtocol, openai_err: &Value) -> Value {
    match proto {
        InboundProtocol::OpenAI => openai_err.clone(),
        InboundProtocol::Anthropic => json!({
            "type": "error",
            "error": {
                "type": "api_error",
                "message": openai_err.pointer("/error/message").cloned().unwrap_or(Value::String("internal".into()))
            }
        }),
        InboundProtocol::Gemini => json!({
            "error": {
                "code": 500,
                "message": openai_err.pointer("/error/message").cloned().unwrap_or(Value::String("internal".into())),
                "status": "INTERNAL"
            }
        }),
        InboundProtocol::Ollama => json!({
            "error": openai_err.pointer("/error/message").cloned().unwrap_or(Value::String("internal".into()))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sniff_prefers_anthropic_shape() {
        let body = json!({"model":"x","max_tokens":100,"system":"s","messages":[{"role":"user","content":"hi"}]});
        let (p, _) = sniff_body(&body);
        assert_eq!(p, InboundProtocol::Anthropic);
    }

    #[test]
    fn sniff_prefers_gemini_shape() {
        let body = json!({"contents":[{"role":"user","parts":[{"text":"hi"}]}],"generationConfig":{"temperature":0.5}});
        let (p, _) = sniff_body(&body);
        assert_eq!(p, InboundProtocol::Gemini);
    }

    #[test]
    fn anthropic_normalize_roundtrip() {
        let body = json!({
            "model":"claude-3","max_tokens":64,"stream":false,
            "system":"be nice",
            "messages":[{"role":"user","content":"hello"}]
        });
        let req = normalize_request(InboundProtocol::Anthropic, &body, None).unwrap();
        assert_eq!(req.messages.len(), 2); // system + user
        assert_eq!(req.max_tokens, Some(64));
    }

    #[test]
    fn gemini_denormalize_shape() {
        let oai = json!({
            "id":"1","object":"chat.completion","created":1,"model":"g",
            "choices":[{"index":0,"message":{"role":"assistant","content":"hey"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}
        });
        let out = denormalize_response(InboundProtocol::Gemini, &oai, "g");
        assert!(out.get("candidates").is_some());
        assert_eq!(out.pointer("/usageMetadata/totalTokenCount"), Some(&json!(5)));
    }
}
