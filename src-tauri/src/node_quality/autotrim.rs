//! 低质量自动降权 / 临时摘除（写入 state.node_runtime.temp_ban_until）
use crate::config::AutoTrimCfg;
use crate::node_quality::scoring::{QualityScore, QualityTier};
use crate::state::AppState;
use std::sync::Arc;
pub fn apply_if_enabled(state: &Arc<AppState>, cfg: &AutoTrimCfg, score: &QualityScore) {
    if !cfg.enabled { return; }
    let tier = score.tier(&state.cfg_swap.load().node_quality.labels);
    match tier {
        QualityTier::Fault => {
            let until = crate::router::group_selector::now_ms() + (cfg.temporary_ban_seconds_when_fault as u128) * 1000;
            state.node_runtime.temp_ban_until.insert(score.node_id.clone(), until as u64);
        }
        QualityTier::Poor => { /* TODO: weight *= demote_weight_when_poor，写权重覆盖表 */ }
        _ => { state.node_runtime.temp_ban_until.remove(&score.node_id); }
    }
}
