//! =============================================================
//!  交付物 3：节点质量分 QualityScore（0..=100）
//!  · 样本不足（< min_samples）时 sample_sufficient=false，路由权重回退到配置 weight
//!  · 分 6 大维度按 ScoringWeights 加权
//! =============================================================
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::Display)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum QualityTier { Excellent, Good, Normal, Poor, Fault }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityScore {
    pub node_id:           String,
    pub sample_count:      u32,
    pub sample_sufficient: bool,
    pub total:             u8,        // 0..=100
    pub success_rate:      f32,       // 0..=1
    pub latency_p99_ms:    u64,
    pub ttft_p99_ms:       u64,
    pub error_1k:          u32,       // 每 1k 请求 429+5xx+SSE异常+JSON 解析失败次数
    pub token_disc_pct:    f32,       // 平均 token 对账偏差率%
    pub sse_abnormal_rate: f32,       // 0..=1
    pub updated_at_ms:     u128,
}

impl QualityScore {
    /// 线性加权算总分（输入各项 0..=100 子分），不足样本强制返回 0+sample_sufficient=false
    pub fn compute(
        node_id: impl Into<String>, samples: u32, min_samples: u32,
        weights: &crate::config::ScoringWeights,
        sub_success: f32, sub_p99: f32, sub_ttft: f32,
        sub_err: f32, sub_disc: f32, sub_sse: f32,
    ) -> Self {
        let sum_w = weights.success_rate + weights.latency_p99 + weights.ttft
            + weights.error_counts + weights.token_discrepancy + weights.sse_abnormal_rate;
        let w = |x: f32| if sum_w <= 0.0 { 1.0 / 6.0 } else { x / sum_w };
        let raw = sub_success * w(weights.success_rate)
            + sub_p99   * w(weights.latency_p99)
            + sub_ttft  * w(weights.ttft)
            + sub_err   * w(weights.error_counts)
            + sub_disc  * w(weights.token_discrepancy)
            + sub_sse   * w(weights.sse_abnormal_rate);
        let total = if samples < min_samples { 0 } else { (raw.clamp(0.0, 100.0)).round() as u8 };
        Self {
            node_id: node_id.into(), sample_count: samples,
            sample_sufficient: samples >= min_samples,
            total,
            success_rate: if samples < min_samples { 0.0 } else { (sub_success / samples as f32) * 100.0 },
            latency_p99_ms: ((100.0 - sub_p99).max(0.0) * 50.0) as u64, // 演示：子分100 = 0ms；子分0 = 5s
            ttft_p99_ms:    ((100.0 - sub_ttft).max(0.0) * 20.0) as u64,
            error_1k:       ((100.0 - sub_err).max(0.0) * 10.0) as u32,
            token_disc_pct: (100.0 - sub_disc).max(0.0),
            sse_abnormal_rate: (100.0 - sub_sse).max(0.0) / 100.0,
            updated_at_ms: now(),
        }
    }
    pub fn tier(&self, labels: &crate::config::QualityLabels) -> QualityTier {
        let t = self.total;
        match () {
            _ if labels.excellent.contains(&t) => QualityTier::Excellent,
            _ if labels.good.contains(&t)      => QualityTier::Good,
            _ if labels.normal.contains(&t)    => QualityTier::Normal,
            _ if labels.poor.contains(&t)      => QualityTier::Poor,
            _ => QualityTier::Fault,
        }
    }
}

fn now() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}
