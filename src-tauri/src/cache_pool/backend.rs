//! =============================================================
//!  交付物 5：CacheBackend trait + InMemoryBackend（mini-moka sync 双实例）
//! =============================================================
use super::cache_entry::{NonStreamEntry, StreamEntry};
use crate::config::MemCacheCfg;
use crate::error::AppResult;
use async_trait::async_trait;
use mini_moka::sync::Cache as MokaCache;
use std::sync::Arc;

#[async_trait]
pub trait CacheBackend: Send + Sync + std::fmt::Debug {
    async fn get_non_stream(&self, k: &u128) -> AppResult<Option<NonStreamEntry>>;
    async fn put_non_stream(&self, k: u128, e: NonStreamEntry) -> AppResult<()>;
    async fn get_stream(&self, k: &u128) -> AppResult<Option<StreamEntry>>;
    async fn put_stream(&self, k: u128, e: StreamEntry) -> AppResult<()>;
    /// 返回 (non_stream_count, stream_count, estimated_mem_bytes)
    async fn stats(&self) -> (u64, u64, usize);
}

/// 双 mini-moka sync 实例
#[derive(Clone)]
pub struct InMemoryBackend {
    ns:  MokaCache<u128, Arc<NonStreamEntry>>,
    ss:  MokaCache<u128, Arc<StreamEntry>>,
    cfg: MemCacheCfg,
}

impl std::fmt::Debug for InMemoryBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryBackend")
            .field("ns_entries", &self.ns.entry_count())
            .field("ss_entries", &self.ss.entry_count())
            .finish()
    }
}

impl InMemoryBackend {
    pub fn new(cfg: MemCacheCfg) -> Self {
        let max_entries_ns = cfg.max_entries_non_stream.max(64) as u64;
        let max_entries_ss = cfg.max_entries_stream.max(32) as u64;
        // NOTE: 原方案使用 weigher + max_weight 按字节数硬上限；
        // mini-moka 0.10 sync API 未提供 max_weight / insert_with_ttl / run_pending_tasks 公共方法，
        // 回退为：仅按条目数 max_capacity 硬上限；条目级 TTL 通过 entry.is_expired() 手动判断（读取时校验）。
        let ns = MokaCache::builder().max_capacity(max_entries_ns).build();
        let ss = MokaCache::builder().max_capacity(max_entries_ss).build();
        Self { ns, ss, cfg }
    }

    pub fn config(&self) -> &MemCacheCfg { &self.cfg }
}

#[async_trait]
impl CacheBackend for InMemoryBackend {
    async fn get_non_stream(&self, k: &u128) -> AppResult<Option<NonStreamEntry>> {
        let Some(v) = self.ns.get(k) else { return Ok(None); };
        if v.is_expired() { self.ns.invalidate(k); return Ok(None); }
        Ok(Some(v.as_ref().clone()))
    }
    async fn put_non_stream(&self, k: u128, e: NonStreamEntry) -> AppResult<()> {
        // TTL 过期判断由 get() 端执行（is_expired）
        self.ns.insert(k, Arc::new(e));
        Ok(())
    }
    async fn get_stream(&self, k: &u128) -> AppResult<Option<StreamEntry>> {
        let Some(v) = self.ss.get(k) else { return Ok(None); };
        if v.is_expired() { self.ss.invalidate(k); return Ok(None); }
        Ok(Some(v.as_ref().clone()))
    }
    async fn put_stream(&self, k: u128, e: StreamEntry) -> AppResult<()> {
        self.ss.insert(k, Arc::new(e));
        Ok(())
    }
    async fn stats(&self) -> (u64, u64, usize) {
        let ns = self.ns.entry_count();
        let ss = self.ss.entry_count();
        // 粗略估算：平均每条 ~256 字节
        let mem = (ns + ss) as usize * 256;
        (ns, ss, mem)
    }
}
