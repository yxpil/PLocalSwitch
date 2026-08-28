//! 候选排序 + 截断（质量分 ≥ weight；得分不足回退纯 weight；按 primary 优先）
use crate::router::CandidateNode;
use crate::state::AppState;
use std::sync::Arc;
pub fn sort_and_trim(_state: &Arc<AppState>, cands: &mut Vec<CandidateNode>) {
    cands.sort_by(|a, b| {
        let qa = (a.quality as f64).max(1.0) * a.weight;
        let qb = (b.quality as f64).max(1.0) * b.weight;
        qb.partial_cmp(&qa).unwrap_or(std::cmp::Ordering::Equal)
    });
    // 硬上限：每次请求最多 10 个候选（防止重试风暴）
    cands.truncate(10);
}
