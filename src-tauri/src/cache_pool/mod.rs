//! =============================================================
//!  5. 独立缓存池（交付物 5）
//! =============================================================
//!  设计要点：
//!    1. 双实例（非流式/流式独立）；tool_call 默认跳过缓存
//!    2. mini-moka 作为 sync 缓存，最大条目数 + 预估内存占用双上限
//!    3. Key 构造：xxhash(model + messages text + temperature + top_p + ...)
//!    4. 每模型独立 TTL 开关、命中是否对客户端收费开关
//!    5. 后台 reclaim 协程定时 evict，不在请求路径做重淘汰
//!    6. 预留 CacheBackend trait，之后可替换 RedisImpl
//! =============================================================
pub mod cache_key;
pub mod backend;           // trait + InMemory impl + Redis stub (optional_components)
pub mod cache_entry;       // 流式：Vec<u8> 字节序列；非流式：ChatCompletionResponse
pub mod metrics;           // 命中/未命中/淘汰/内存占用 Prometheus metrics

use crate::error::AppResult;
use crate::state::AppState;
use std::sync::Arc;

/// 启动后台淘汰任务（每 30 秒触发 mini-moka.run_pending_tasks 并打印指标）
pub async fn spawn_reclaim_loop(_state: Arc<AppState>) {
    // TODO: tokio::spawn(async move { loop { ... } });
}

/// 非流式查询：命中 → 返回 (entry, billing_treatment_enum)
pub async fn try_get_non_stream(state: &Arc<AppState>, key: &u128) -> AppResult<Option<cache_entry::NonStreamEntry>> {
    if let Some(backend) = &state.cache_backend {
        backend.get_non_stream(key).await
    } else {
        Ok(None)
    }
}

/// 非流式写入（仅当请求非 tool_use 且模型配置允许缓存）
pub async fn put_non_stream(state: &Arc<AppState>, key: u128, entry: cache_entry::NonStreamEntry) -> AppResult<()> {
    if let Some(backend) = &state.cache_backend {
        backend.put_non_stream(key, entry).await
    } else {
        Ok(())
    }
}
