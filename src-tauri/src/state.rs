//! =============================================================
//!  AppState 全局托管（网关 + 桌面壳共用）
//! =============================================================
use crate::billing::client_key_mgr::ClientKeyRegistry;
use crate::billing::tokenizer_pool::TokenizerPool;
use crate::cache_pool::backend::{CacheBackend, InMemoryBackend};
use crate::config::AppConfig;
use crate::error::AppResult;
use crate::flex_adapter::protocol_sniffer::SniffCache;
use crate::gateway_api::rate_limit::RateLimiter;
use crate::safety_runtime::connection_pool::ReqwestPoolGroup;
use crate::safety_runtime::semaphores::SemaphoreGroup;
use crate::safety_runtime::rate_limits::RateLimitGroup;
use arc_swap::ArcSwap;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::Arc;

pub static APP_DIRS: Lazy<AppDirs> = Lazy::new(AppDirs::new);

/// MetricsRegistry = MetricsHandles（单 pub use 行，避免 use 私有名再 pub re-export 违反可见性）
pub use crate::observability::metrics_registry::MetricsHandles as MetricsRegistry;

// axum 0.7 extractor 需要：从共享 State(&Arc<AppState>) 克隆一份 Arc 出来给 AuthedClient
impl axum::extract::FromRef<Arc<AppState>> for AppState {
    fn from_ref(input: &Arc<AppState>) -> Self {
        // 不允许真的 move AppState（含 tokio::RwLock / DashMap / trait object）；
        // 这里我们实际 extractor 只用到 *app 的引用，返回一个 panic，让 extractor 写法改为 State<Arc<AppState>>。
        // 真正实现：返回 Arc::clone 的 Arc<AppState>，但 AppState 不是 Arc，所以我们在 auth.rs 改用 State<Arc<AppState>> 提取。
        // 这个 impl 仅为满足 trait bound：返回 panic 不走这条路径。
        let _ = input;
        panic!("AppState::from_ref: impossible — auth extractor uses State<Arc<AppState>> instead");
    }
}

pub struct AppState {
    pub cfg:          Arc<AppConfig>,
    pub cfg_swap:     Arc<ArcSwap<AppConfig>>,
    pub started_at:   i64,
    pub request_counter: std::sync::atomic::AtomicU64,

    // ==== 网关核心硬上限受控 ====
    pub db:           DbPool,
    pub http_pools:   ReqwestPoolGroup,
    pub semaphores:   SemaphoreGroup,
    pub rate_limits:  RateLimitGroup,
    /// 按 client_key RPM/TPM 限流器（gateway_api::auth handler 内调用）
    pub per_key_rate_limits: RateLimiter,
    /// trait object：可替换 Redis 后端
    pub cache_backend: Option<Arc<dyn CacheBackend>>,
    /// 具体内存实现（读取统计/写直接 put/get 用）
    pub cache:        Arc<InMemoryBackend>,
    pub tokenizers:   Arc<TokenizerPool>,
    pub metrics:      Arc<MetricsRegistry>,
    pub node_runtime: Arc<NodeRuntime>,
    /// 网关自有 API Key 注册表（读多写少）
    pub client_keys:  tokio::sync::RwLock<ClientKeyRegistry>,
    /// 网关服务启停控制句柄（桌面面板一键 start/stop）
    pub gateway_ctrl: Arc<GatewayCtrl>,
    /// 桌面壳 AppHandle：托盘菜单通过 HTTP 控制“显示主窗口/退出/打开反馈”时使用（纯网关模式为 None）
    #[cfg(feature = "desktop-shell")]
    pub app_handle: std::sync::Mutex<Option<tauri::AppHandle>>,
}

/// 网关服务生命周期控制（面板总控）
pub struct GatewayCtrl {
    pub running:  std::sync::atomic::AtomicBool,
    pub listen:   String,
    state:        std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl GatewayCtrl {
    pub fn new(listen: String) -> Self {
        Self {
            running: std::sync::atomic::AtomicBool::new(false),
            listen,
            state: std::sync::Mutex::new(None),
        }
    }
    pub fn is_running(&self) -> bool { self.running.load(std::sync::atomic::Ordering::Relaxed) }
    /// 记录本次运行实例的 shutdown 发送端
    pub fn register(&self, tx: tokio::sync::oneshot::Sender<()>) {
        *self.state.lock().unwrap() = Some(tx);
        self.running.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    /// 触发优雅停止；返回是否确实在运行
    pub fn request_stop(&self) -> bool {
        if let Some(tx) = self.state.lock().unwrap().take() {
            let _ = tx.send(());
            self.running.store(false, std::sync::atomic::Ordering::Relaxed);
            true
        } else {
            false
        }
    }
}

pub struct NodeRuntime {
    pub capabilities: DashMap<String, (crate::flex_adapter::capability_cache::Capabilities, u128)>,
    pub proto_sniff:  SniffCache,
    pub temp_ban_until: DashMap<String, u64>,
    /// 模型目录：上游真实模型名 → 服务它的节点组 id（供“模型↔API”匹配路由）
    pub model_catalog: DashMap<String, String>,
}

pub enum DbPool {
    Sqlite(sqlx::SqlitePool),
    Postgres(sqlx::PgPool),
    None,
}

impl AppState {
    pub fn bump_request(&self) -> u64 {
        self.request_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
    }
    pub fn uptime_seconds(&self) -> i64 { chrono::Utc::now().timestamp() - self.started_at }

    pub async fn bootstrap(cfg: Arc<AppConfig>) -> AppResult<Self> {
        // 1. DB
        let db = match cfg.db.backend.as_str() {
            "sqlite" => {
                let path = &cfg.db.sqlite_path;
                if let Some(parent) = std::path::Path::new(path).parent() {
                    tokio::fs::create_dir_all(parent).await.ok();
                }
                let opts = sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(cfg.db.pool_max_open)
                    .min_connections(cfg.db.pool_max_idle.min(1));
                let p = opts.connect(&format!("sqlite:{path}?mode=rwc")).await
                    .map_err(crate::error::AppError::Sqlx)?;
                DbPool::Sqlite(p)
            }
            "postgres" => {
                let opts = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(cfg.db.pool_max_open)
                    .min_connections(cfg.db.pool_max_idle.min(1));
                let p = opts.connect(&cfg.db.postgres_url).await
                    .map_err(crate::error::AppError::Sqlx)?;
                DbPool::Postgres(p)
            }
            _ => { tracing::warn!("db backend disabled (memory only)"); DbPool::None },
        };

        let http_pools   = ReqwestPoolGroup::from_cfg(&cfg)?;
        let semaphores   = SemaphoreGroup::new(
            cfg.http.global_concurrency_limit,
            cfg.http.per_client_key_concurrency_limit,
        );
        let rate_limits  = RateLimitGroup::new(&cfg);
        let per_key_rate_limits = RateLimiter::new();
        let mem_cache    = Arc::new(InMemoryBackend::new(cfg.cache_pool.in_memory.clone()));
        let cache_backend: Option<Arc<dyn CacheBackend>> = match cfg.cache_pool.implementation.as_str() {
            "memory" => Some(mem_cache.clone()),
            _ => None,
        };
        let tokenizers   = Arc::new(TokenizerPool::new(&cfg.billing.tokenizers));
        let metrics      = Arc::new(MetricsRegistry::new().expect("metrics registry init"));
        let node_runtime = Arc::new(NodeRuntime {
            capabilities:   DashMap::new(),
            proto_sniff:    SniffCache::new(),
            temp_ban_until: DashMap::new(),
            model_catalog:  DashMap::new(),
        });
        let client_keys = tokio::sync::RwLock::new(ClientKeyRegistry::from_cfg(&cfg.billing.client_keys));
        let listen = cfg.http.listen.clone();

        Ok(Self {
            cfg: cfg.clone(),
            cfg_swap: Arc::new(ArcSwap::new(cfg)),
            started_at: chrono::Utc::now().timestamp(),
            request_counter: std::sync::atomic::AtomicU64::new(0),
            db, http_pools, semaphores, rate_limits, per_key_rate_limits,
            cache_backend, cache: mem_cache, tokenizers, metrics, node_runtime, client_keys,
            gateway_ctrl: Arc::new(GatewayCtrl::new(listen)),
            #[cfg(feature = "desktop-shell")]
            app_handle: std::sync::Mutex::new(None),
        })
    }
}

// ===== 应用目录 =====
pub struct AppDirs {
    pub data_dir:   std::path::PathBuf,
    pub config_dir: std::path::PathBuf,
    pub logs_dir:   std::path::PathBuf,
    pub cache_dir:  std::path::PathBuf,
}
impl AppDirs {
    fn new() -> Self {
        let proj = directories::ProjectDirs::from("com", "plocalswitch", "PLocalSwitch");
        if let Some(p) = proj {
            Self {
                data_dir:   p.data_dir().to_path_buf(),
                config_dir: p.config_dir().to_path_buf(),
                logs_dir:   p.data_local_dir().join("logs"),
                cache_dir:  p.cache_dir().to_path_buf(),
            }
        } else {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let r = cwd.join(".plocalswitch");
            Self {
                data_dir:   r.join("data"),
                config_dir: r.join("config"),
                logs_dir:   r.join("logs"),
                cache_dir:  r.join("cache"),
            }
        }
    }
}
