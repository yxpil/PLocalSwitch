//! 候选排序 + 截断（质量分 ≥ weight；得分不足回退纯 weight；按 primary 优先）
use crate::router::CandidateNode;
use crate::state::AppState;
use std::sync::Arc;
pub fn sort_and_trim(state: &Arc<AppState>, cands: &mut Vec<CandidateNode>) {
    // 「免费源优先」开启时：free 节点排前面（同优先级内仍按 质量×权重 降序）
    let prefer_free = state.cfg_swap.load().automode.prefer_free;
    cands.sort_by(|a, b| {
        if prefer_free {
            match b.free.cmp(&a.free) {
                std::cmp::Ordering::Equal => {}
                o => return o,
            }
        }
        let qa = (a.quality as f64).max(1.0) * a.weight;
        let qb = (b.quality as f64).max(1.0) * b.weight;
        qb.partial_cmp(&qa).unwrap_or(std::cmp::Ordering::Equal)
    });
    // 硬上限：每次请求最多 10 个候选（防止重试风暴）
    cands.truncate(10);
}
