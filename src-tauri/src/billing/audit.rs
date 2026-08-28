//! =============================================================
//!  交付物 3：billing::audit（回写 observability::audit_record）
//! =============================================================
pub use crate::observability::audit_record::AuditRecord;
use super::counter::UsageTriple;

impl AuditRecord {
    /// 从 UsageTriple（A vs B）构造对账记录
    pub fn from_triple(
        request_id: impl Into<String>, model: impl Into<String>,
        triple: &UsageTriple, alarm_pct: f64,
    ) -> Option<Self> {
        let a = triple.a.clone()?;
        let b = triple.b.clone().unwrap_or_default();
        Some(Self::compute(request_id, model, a, b, alarm_pct))
    }
}
