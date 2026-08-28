//! =============================================================
//!  交付物 3：缓存条目 NonStream / Stream
//!  · NonStreamEntry = 成功的非流式 ChatCompletionResponse（可直接回客户端）
//!  · StreamEntry    = 序列化后的原始 SSE 字节序列（客户端重放）
//!  · 都附带 created_at + size_bytes，方便 reclaim 统计内存
//! =============================================================
use crate::observability::trace::UsageSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonStreamEntry {
    pub key_hash:       u128,
    pub model:          String,
    pub created_at_ms:  u128,
    pub ttl_ms:         Option<u64>,
    /// OpenAI v1 /v1/chat/completions 200 OK response JSON bytes
    pub response_json:  Vec<u8>,
    /// 命中是否需要对客户端收费（由 ModelAlias.charge_on_cache_hit 决定，写缓存时记住）
    pub charge_on_hit:  bool,
    /// 原始 usage，命中时直接用于账本
    pub billed_usage:   UsageSnapshot,
    /// 预估占用字节（response_json + 字段）
    pub size_bytes:     usize,
}

impl NonStreamEntry {
    pub fn new(
        key_hash: u128, model: impl Into<String>, ttl_ms: Option<u64>,
        response_json: Vec<u8>, charge_on_hit: bool, usage: UsageSnapshot,
    ) -> Self {
        let m: String = model.into();
        let size = response_json.len() + m.len() + std::mem::size_of::<Self>();
        Self {
            key_hash, model: m, created_at_ms: now(), ttl_ms,
            response_json, charge_on_hit, billed_usage: usage, size_bytes: size,
        }
    }
    pub fn is_expired(&self) -> bool {
        match self.ttl_ms {
            None => false,
            Some(t) => now() - self.created_at_ms >= t as u128,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEntry {
    pub key_hash:       u128,
    pub model:          String,
    pub created_at_ms:  u128,
    pub ttl_ms:         Option<u64>,
    /// 原始 SSE 字节（含 data: ... / event: done 的全部行），可直接 pipe 给 response body
    pub sse_bytes:      Vec<u8>,
    pub charge_on_hit:  bool,
    /// 由 delta 累加得到的最终 usage
    pub billed_usage:   UsageSnapshot,
    pub size_bytes:     usize,
}
impl StreamEntry {
    pub fn new(
        key_hash: u128, model: impl Into<String>, ttl_ms: Option<u64>,
        sse_bytes: Vec<u8>, charge_on_hit: bool, usage: UsageSnapshot,
    ) -> Self {
        let m: String = model.into();
        let size = sse_bytes.len() + m.len() + std::mem::size_of::<Self>();
        Self {
            key_hash, model: m, created_at_ms: now(), ttl_ms,
            sse_bytes, charge_on_hit, billed_usage: usage, size_bytes: size,
        }
    }
    pub fn is_expired(&self) -> bool {
        match self.ttl_ms { None => false, Some(t) => now() - self.created_at_ms >= t as u128 }
    }
}

fn now() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}
