//! =============================================================
//!  交付物 3：UsageTriple —— 三档 usage 采集源
//!    A = upstream_usage    （优先，上游返回的 usage 字段）
//!    B = local_tokenize    （无 A 时用，tiktoken/外部分词器本地重算）
//!    C = stream_delta_sum  （流式，chunk.delta.token 累加兜底）
//!  选择优先级：A > B > C
//! =============================================================
use crate::observability::trace::UsageSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageTriple {
    pub a: Option<UsageSnapshot>,  // 上游
    pub b: Option<UsageSnapshot>,  // 本地分词
    pub c: Option<UsageSnapshot>,  // 流式 delta
}
impl UsageTriple {
    pub fn pick(&self) -> UsageSnapshot {
        self.a.clone().or_else(|| self.b.clone()).or_else(|| self.c.clone())
            .unwrap_or_default()
    }
    pub fn sources_available(&self) -> u8 {
        let mut n = 0u8;
        if self.a.is_some() { n += 1; }
        if self.b.is_some() { n += 1; }
        if self.c.is_some() { n += 1; }
        n
    }
}
