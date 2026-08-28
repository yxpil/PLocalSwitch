//! =============================================================
//!  交付物 3：双账本（Upstream / Client）
//!  · 账本 ① UpstreamLedgerEntry：每次 SubAttempt 都写（含失败/重试）→ 采购成本
//!  · 账本 ② ClientLedgerEntry  ：仅 GatewayTrace 最终结果写 → 客户端计费（永不包含重试开销）
//! =============================================================
use crate::observability::trace::UsageSnapshot;
use serde::{Deserialize, Serialize};

/// 账本 ①：上游采购成本账本（SubAttempt 粒度）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamLedgerEntry {
    pub id:              String,        // ULID
    pub sub_attempt_id:  String,
    pub trace_id:        String,
    pub node_id:         String,
    pub node_group_id:   String,
    pub model:           String,
    pub success:         bool,
    pub usage:           UsageSnapshot,
    pub cost_input_cny:  f64,
    pub cost_output_cny: f64,
    pub cost_total_cny:  f64,
    pub created_at_ms:   u128,
}

/// 账本 ②：客户端计费账本（Trace 粒度，多档来源 pick 后写入）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientLedgerEntry {
    pub id:                 String,
    pub trace_id:           String,
    pub client_key_hash:    String,
    pub client_key_name:    Option<String>,
    pub model:              String,
    pub is_stream:          bool,
    pub is_cached_hit:      bool,
    pub usage_source:       String,   // "A" | "B" | "C"
    pub usage:              UsageSnapshot,
    pub price_input_cny:    f64,
    pub price_output_cny:   f64,
    pub price_total_cny:    f64,
    pub discount_rate:      f64,      // 0..=1, 1.0 = 无折扣，0.8 = 8折
    pub final_charge_cny:   f64,
    pub created_at_ms:      u128,
}

/// 账本聚合查询：返回指定 client_key 的 24h/7d/30d 汇总
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientSummary {
    pub client_key_hash: String,
    pub window_label:    String,   // "24h" | "7d" | "30d"
    pub requests_total:  u64,
    pub requests_ok:     u64,
    pub tokens_input:    u64,
    pub tokens_output:   u64,
    pub total_charge_cny: f64,
}
