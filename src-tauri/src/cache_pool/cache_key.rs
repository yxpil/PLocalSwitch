//! =============================================================
//!  交付物 5：缓存 Key 构造（AHasher 双 seed → u128）
//! =============================================================
use crate::models::{ChatCompletionRequest, MessageContent};
use ahash::AHasher;
use std::hash::{Hash, Hasher};

pub fn hash_request(req: &ChatCompletionRequest) -> Option<u128> {
    // 含 tool/function 调用 → 跳过缓存
    if req.tools.as_ref().map(|t| !t.is_empty()).unwrap_or(false) { return None; }
    if req.tool_choice.is_some() { return None; }

    let mut h1 = AHasher::default();
    let mut h2 = AHasher::default();
    h1.write_usize(0xCAFEBABE);
    h2.write_usize(0xDEADBEEF);

    req.model.hash(&mut h1); req.model.hash(&mut h2);
    for m in &req.messages {
        m.role.hash(&mut h1); m.role.hash(&mut h2);
        match &m.content {
            MessageContent::Text(s) => { s.hash(&mut h1); s.hash(&mut h2); }
            MessageContent::MultiPart(parts) => {
                for p in parts {
                    if p.kind == "text" {
                        if let Some(t) = &p.text { t.hash(&mut h1); t.hash(&mut h2); }
                    } else if p.kind == "image_url" {
                        // 图片：不缓存，跳过
                        return None;
                    }
                    // 其他 part 类型：忽略，降低碰撞噪声
                }
            }
        }
    }
    req.temperature.unwrap_or(1.0).to_bits().hash(&mut h1);
    req.top_p.unwrap_or(1.0).to_bits().hash(&mut h1);
    req.max_tokens.unwrap_or(0).hash(&mut h1);
    if let Some(rf) = &req.response_format {
        // ResponseFormat 是结构体（非 enum），直接哈希其规范化 JSON
        if let Ok(s) = serde_json::to_string(rf) { s.hash(&mut h1); }
    }
    // 同样一份给 h2
    req.temperature.unwrap_or(1.0).to_bits().hash(&mut h2);
    req.top_p.unwrap_or(1.0).to_bits().hash(&mut h2);
    req.max_tokens.unwrap_or(0).hash(&mut h2);
    if let Some(rf) = &req.response_format {
        if let Ok(s) = serde_json::to_string(rf) { s.hash(&mut h2); }
    }

    let lo = h1.finish();
    let hi = h2.finish();
    Some(((hi as u128) << 64) | (lo as u128))
}
