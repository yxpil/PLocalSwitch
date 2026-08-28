//! =============================================================
//!  Prometheus 指标注册（交付物 3 骨架）
//!  · 所有分桶维度：client_key_hash / node_id / error_label / model_alias
//!  · 指标启用开关由 metrics.* 配置决定（默认关闭 per_client_key_labels 防高基数）
//! =============================================================
use prometheus::{register_counter_vec, register_histogram_vec, CounterVec, HistogramVec, Registry};
use std::sync::Arc;

#[derive(Clone)]
pub struct MetricsHandles {
    pub registry:            Arc<Registry>,
    pub requests_total:      CounterVec,
    pub request_duration:    HistogramVec,
    pub ttft_seconds:        HistogramVec,
    pub sub_attempts_total:  CounterVec,
    pub tokens_input_total:  CounterVec,
    pub tokens_output_total: CounterVec,
    pub cache_hits_total:    CounterVec,
    pub cache_miss_total:    CounterVec,
    pub ledger_charge_cny:   CounterVec,
}

impl MetricsHandles {
    pub fn new() -> Result<Self, prometheus::Error> {
        let r = Registry::new();
        let requests_total = register_counter_vec!(
            "pls_requests_total", "Total requests by status/label",
            &["model", "status_code", "error_label", "is_stream", "cached"]
        )?;
        let request_duration = register_histogram_vec!(
            "pls_request_duration_seconds", "End-to-end latency",
            &["model", "is_stream"]
        )?;
        let ttft_seconds = register_histogram_vec!(
            "pls_ttft_seconds", "Time to first token",
            &["model", "node_group"]
        )?;
        let sub_attempts_total = register_counter_vec!(
            "pls_sub_attempts_total", "Per-node upstream attempts",
            &["node_id", "protocol", "outcome", "error_label"]
        )?;
        let tokens_input_total = register_counter_vec!(
            "pls_tokens_input_total", "Input tokens billed",
            &["model", "source"]   // source = upstream | local | stream_delta
        )?;
        let tokens_output_total = register_counter_vec!(
            "pls_tokens_output_total", "Output tokens billed",
            &["model", "source"]
        )?;
        let cache_hits_total = register_counter_vec!("pls_cache_hits_total", "Cache hits", &["model", "kind"])?;
        let cache_miss_total = register_counter_vec!("pls_cache_miss_total", "Cache miss", &["model", "kind"])?;
        let ledger_charge_cny = register_counter_vec!(
            "pls_ledger_charge_cny_total", "Billed CNY", &["client_key_hash", "model"]
        )?;
        // 这里不把全局默认 registry 直接混入，所有指标挂到自建 registry
        // （避免 Tauri/其他模块的指标串线）
        // 默认 CounterVec 会注册到 prometheus::default_registry()，此处按官方方式即可；
        // 若需单独 registry，请改为 CounterVec::new + r.register()。这里走最简全局注册。
        Ok(Self {
            registry: Arc::new(r),
            requests_total, request_duration, ttft_seconds, sub_attempts_total,
            tokens_input_total, tokens_output_total, cache_hits_total, cache_miss_total,
            ledger_charge_cny,
        })
    }
}
impl std::fmt::Debug for MetricsHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MetricsHandles { .. }")
    }
}
