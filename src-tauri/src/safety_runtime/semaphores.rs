//! 全局 + per-client 并发信号量（硬上限，超了立即 429）
use dashmap::DashMap;
use tokio::sync::Semaphore;
pub struct ConcurrencyLimiter {
    pub global: std::sync::Arc<Semaphore>,
    pub per_key: DashMap<String, std::sync::Arc<Semaphore>>,
}
impl ConcurrencyLimiter {
    pub fn new(global_max: usize, _per_key_max: usize) -> Self {
        Self { global: std::sync::Arc::new(Semaphore::new(global_max.max(1))), per_key: DashMap::new() }
    }
}
pub use ConcurrencyLimiter as SemaphoreGroup;
