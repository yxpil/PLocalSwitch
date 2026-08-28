//! =============================================================
//!  交付物 3：对账记录 AuditRecord (A 上游 usage vs B 本地分词 usage)
//! =============================================================
use super::trace::UsageSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub request_id:                  String,
    pub model:                       String,
    pub upstream_usage:              UsageSnapshot,
    pub local_usage:                 UsageSnapshot,
    pub prompt_discrepancy_percent:  f64,
    pub completion_discrepancy_percent: f64,
    pub alarm:                       bool,
    pub created_at_ms:               u128,
}
impl AuditRecord {
    pub fn compute(
        request_id: impl Into<String>, model: impl Into<String>,
        up: UsageSnapshot, local: UsageSnapshot, alarm_threshold_percent: f64,
    ) -> Self {
        let pd = discrepancy(up.prompt_tokens, local.prompt_tokens);
        let cd = discrepancy(up.completion_tokens, local.completion_tokens);
        Self {
            request_id: request_id.into(), model: model.into(),
            upstream_usage: up, local_usage: local,
            prompt_discrepancy_percent: pd, completion_discrepancy_percent: cd,
            alarm: pd > alarm_threshold_percent || cd > alarm_threshold_percent,
            created_at_ms: now_ms(),
        }
    }
}
fn discrepancy(a: u32, b: u32) -> f64 {
    let ma = a.max(b) as f64;
    if ma < 1.0 { return 0.0; }
    ((a as i64 - b as i64).unsigned_abs() as f64) / ma * 100.0
}
pub fn now_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}
