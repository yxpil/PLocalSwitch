//! RPM / TPM 每 client_key 限流（滑动窗口，DashMap<String, WindowCounter>）
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct CounterWindow { pub start_ms: u128, pub rpm: AtomicU64, pub tpm: AtomicU64 }
pub struct RateLimiter { pub per_key: DashMap<String, CounterWindow> }
impl RateLimiter {
    pub fn new() -> Self { Self { per_key: DashMap::new() } }
    /// 返回 Ok(()) 表示通过；Err(reason) 限流
    pub fn check_pass(&self, key: &str, rpm: u32, tpm: u64, add_tokens_delta: u64, now_ms: u128) -> Result<(), &'static str> {
        // 窗口：60_000 ms；超期重置
        let mut entry = self.per_key.entry(key.to_string()).or_insert_with(|| CounterWindow { start_ms: now_ms, rpm: AtomicU64::new(0), tpm: AtomicU64::new(0) });
        if now_ms.saturating_sub(entry.start_ms) >= 60_000 {
            entry.start_ms = now_ms;
            entry.rpm.store(0, Ordering::Relaxed);
            entry.tpm.store(0, Ordering::Relaxed);
        }
        let r = entry.rpm.fetch_add(1, Ordering::Relaxed) + 1;
        if r > rpm as u64 { return Err("rpm"); }
        if add_tokens_delta > 0 {
            let t = entry.tpm.fetch_add(add_tokens_delta, Ordering::Relaxed) + add_tokens_delta;
            if t > tpm { return Err("tpm"); }
        }
        Ok(())
    }
}
impl Default for RateLimiter { fn default() -> Self { Self::new() } }
