//! 缓存 Prometheus 命中/未命中/淘汰指标
use crate::state::AppState;
use std::sync::Arc;
pub fn inc_hit(state: &Arc<AppState>, model: &str, kind: &str) {
    state.metrics.cache_hits_total.with_label_values(&[model, kind]).inc();
    let _ = (state, model, kind);
}
pub fn inc_miss(state: &Arc<AppState>, model: &str, kind: &str) {
    state.metrics.cache_miss_total.with_label_values(&[model, kind]).inc();
    let _ = (state, model, kind);
}
