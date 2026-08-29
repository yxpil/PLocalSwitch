//! =============================================================
//!  config/AppConfig：读取 gateway.yaml（serde_yaml）并提供热更新 ArcSwap
//!  100% 对应上一条 `src-tauri/config/gateway.yaml` 字段
//! =============================================================
use crate::error::AppResult;
use crate::state::APP_DIRS;
use serde::{Deserialize, Serialize};

// 配置根结构（按 yaml 分节）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: String,
    pub app: AppMeta,
    pub http: HttpConfig,
    pub db: DbConfig,
    pub metrics: MetricsConfig,
    pub cors: CorsConfig,
    pub model_aliases: Vec<ModelAlias>,
    pub node_groups: Vec<NodeGroup>,
    pub flex_adapter: FlexAdapterConfig,
    pub cache_pool: CachePoolConfig,
    pub billing: BillingConfig,
    pub node_quality: NodeQualityConfig,
    pub policy: PolicyConfig,
    pub masking: MaskingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMeta {
    pub name: String,
    pub env:  String,
    pub timezone: String,
    pub log_level: String,
    pub privacy: PrivacyConfig,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    pub store_payload_text: bool,
    pub masking: bool,
    pub mask_token_head_tail: [usize; 2],
    pub mask_url_path_segment_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub listen: String,
    #[serde(default)] pub admin_listen: Option<String>,
    #[serde(default)] pub request_body_max_bytes: usize,
    #[serde(default)] pub global_concurrency_limit: usize,
    #[serde(default)] pub per_client_key_concurrency_limit: usize,
    #[serde(default)] pub client_disconnect_aborts_upstream: bool,
    #[serde(default)] pub timeouts: TimeoutConfig,
    /// 上游代理开关。用户处于地区受限网络时，某些上游（Claude/Grok 等）必须走代理才能访问。
    /// 该字段由设置页「网络 → 代理配置」读写，而非手动改配置文件。
    #[serde(default)] pub proxy_enabled: bool,
    /// HTTP(S) 代理（如 http://127.0.0.1:7890）。空则不使用。
    #[serde(default)] pub proxy: Option<String>,
    /// SOCKS5 代理（如 socks5://127.0.0.1:1080）。当 proxy 为空时兜底。
    #[serde(default)] pub proxy_socks: Option<String>,
    /// 走代理时跳过的主机列表（glob，如 api.deepseek.com/localhost），空则全部走代理。
    #[serde(default)] pub proxy_no_proxy: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimeoutConfig {
    #[serde(default)] pub connect_ms: u64,
    #[serde(default)] pub read_ms:    u64,
    #[serde(default)] pub stream_read_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    pub backend: String,
    #[serde(default)] pub sqlite_path: String,
    #[serde(default)] pub postgres_url: String,
    pub pool_max_open: u32,
    pub pool_max_idle: u32,
    pub migrate_on_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub expose_at: String,
    pub process_collector: bool,
    pub per_client_key_labels: bool,
    pub per_node_labels: bool,
    pub per_error_label: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    pub allow_origins: Vec<String>,
    pub allow_methods: Vec<String>,
    pub allow_headers: Vec<String>,
    /// gw: 是否允许携带凭证（cookie/Authorization）。开启后 allow_origins 不能是 "*"
    #[serde(default)] pub allow_credentials: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAlias {
    pub alias:      String,
    pub real_model: String,
    pub group:      String,
    #[serde(default)] pub cache_enable: bool,
    #[serde(default)] pub ttl_seconds: Option<u64>,
    #[serde(default)] pub charge_on_cache_hit: bool,
    /// gw8b：暂停/播放开关（false = 暂停，路由直接跳过）
    #[serde(default = "default_true")] pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeGroup {
    pub id: String,
    #[serde(default)] pub description: Option<String>,
    #[serde(default = "default_lb")] pub load_balance: String,
    /// gw8b：整组暂停/播放
    #[serde(default = "default_true")] pub enabled: bool,
    pub nodes: Vec<UpstreamNode>,
}
fn default_lb() -> String { "round_robin".into() }
fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamNode {
    pub id: String,
    pub endpoint: String,
    #[serde(default)] pub api_keys: Vec<String>,
    pub protocol_hints: Vec<String>,
    #[serde(default = "default_true")] pub enabled: bool,
    #[serde(default = "default_weight")] pub weight: f64,
    #[serde(default)] pub hard_disable: bool,
    #[serde(default)] pub connect_pool: Option<ConnPoolCfg>,
    #[serde(default)] pub timeouts_override: Option<TimeoutConfig>,
    #[serde(default)] pub primary: Option<bool>,
    #[serde(default)] pub aws_region: Option<String>,
    #[serde(default)] pub ak: Option<String>,
    #[serde(default)] pub sk: Option<String>,
}
fn default_weight() -> f64 { 1.0 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnPoolCfg {
    pub max_idle_per_host: usize,
    pub pool_idle_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlexAdapterConfig {
    pub sniff_attempts_per_node: u32,
    pub global_max_sub_attempts: u32,
    pub sniff_remember_ttl_seconds: u64,
    pub flexible_parse_alert_on_fallback: bool,
    pub stream_lock_after_first_byte: bool,
    pub capability: CapabilityConfig,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityConfig {
    pub probe_interval_seconds: u64,
    pub probe_prompt: String,
    pub probe_priority_nodes_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachePoolConfig {
    pub implementation: String,
    pub in_memory: MemCacheCfg,
    pub redis: RedisCfg,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemCacheCfg {
    pub max_entries_non_stream: usize,
    pub max_entries_stream: usize,
    pub max_total_memory_mb: usize,
    pub evict_interval_seconds: u64,
    pub hash_key_algo: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisCfg {
    pub url: String,
    #[serde(default)] pub username: String,
    #[serde(default)] pub password: String,
    pub db: u8,
    pub default_ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingConfig {
    pub currency: String,
    pub rates: Vec<ModelRate>,
    pub client_keys: Vec<ClientKey>,
    pub audit: AuditConfig,
    pub tokenizers: Vec<TokenizerBinding>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRate {
    pub model: String,
    pub upstream_cost_per_m_input:  f64,
    pub upstream_cost_per_m_output: f64,
    pub client_price_per_m_input:   f64,
    pub client_price_per_m_output:  f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientKey {
    pub key: String,
    pub name: String,
    #[serde(default)] pub group: String,
    #[serde(default = "d60")]  pub rpm: u32,
    #[serde(default = "d100k")] pub tpm: u64,
    /// 并发上限；0 = 不限（实际限流走全局 per_client_key_concurrency_limit）
    #[serde(default)] pub concurrency: u32,
    #[serde(default)] pub balance_cny: f64,
    #[serde(default)] pub daily_hard_quota_tokens: u64,
    #[serde(default)] pub total_hard_quota_tokens: u64,
    #[serde(default)] pub allow_overdraft: bool,
    #[serde(default = "d_true")] pub enabled: bool,
    #[serde(default = "d_plan")] pub rate_plan: String,
}
fn d60() -> u32 { 60 }
fn d100k() -> u64 { 100_000 }
fn d_true() -> bool { true }
fn d_plan() -> String { "default".into() }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub discrepancy_alarm_percent: f64,
    pub override_billing_when_discrepancy: bool,
    pub override_prefer: String, // upstream|local
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizerBinding {
    pub model_glob: String,
    pub provider: String,                // tiktoken|external
    #[serde(default)] pub tiktoken_encoding: Option<String>,
    #[serde(default)] pub vocab_file:     Option<String>,
    #[serde(default)] pub merges_file:    Option<String>,
    #[serde(default)] pub bpe_file:       Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeQualityConfig {
    pub min_samples: u32,
    pub scoring_weights: ScoringWeights,
    pub labels: QualityLabels,
    pub autotrim: AutoTrimCfg,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub success_rate:      f32,
    pub latency_p99:       f32,
    pub ttft:              f32,
    pub error_counts:      f32,
    pub token_discrepancy: f32,
    pub sse_abnormal_rate: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityLabels {
    pub excellent: ScoreRange,
    pub good:      ScoreRange,
    pub normal:    ScoreRange,
    pub poor:      ScoreRange,
    pub fault:     ScoreRange,
}

/// 分数区间：兼容 "90..100"（字符串）与 [90,100]（数组）两种写法
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreRange(std::ops::RangeInclusive<u8>);

impl ScoreRange {
    pub fn contains(&self, v: &u8) -> bool { self.0.contains(v) }
}

impl serde::Serialize for ScoreRange {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let (a, b) = (self.0.start(), self.0.end());
        s.serialize_str(&format!("{a}..{b}"))
    }
}

impl<'de> serde::Deserialize<'de> for ScoreRange {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = ScoreRange;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("score range like \"90..100\" or [90, 100]")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<ScoreRange, E> {
                let (a, b) = v.split_once("..").ok_or_else(|| E::custom(format!("bad range '{v}'")))?;
                let a: u8 = a.trim().parse().map_err(serde::de::Error::custom)?;
                let b: u8 = b.trim().parse().map_err(serde::de::Error::custom)?;
                Ok(ScoreRange(a..=b))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<ScoreRange, A::Error> {
                let a: u8 = seq.next_element()?.ok_or_else(|| serde::de::Error::custom("range needs [start, end]"))?;
                let b: u8 = seq.next_element()?.ok_or_else(|| serde::de::Error::custom("range needs [start, end]"))?;
                Ok(ScoreRange(a..=b))
            }
        }
        d.deserialize_any(V)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTrimCfg {
    pub enabled: bool,
    pub temporary_ban_seconds_when_fault: u64,
    pub demote_weight_when_poor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub retry_on: RetryOnCfg,
    pub analysis_history_window_seconds: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryOnCfg {
    pub network_connect_refused: bool,
    pub dns_fail: bool,
    pub connect_timeout: bool,
    pub read_timeout: bool,
    pub tls_error: bool,
    pub http_429: bool,
    pub http_5xx: bool,
    pub auth_401_403: bool,
    pub bad_param_4xx: bool,
    pub sse_premature_close: bool,
    pub json_parse_fail: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskingConfig {
    pub enabled: bool,
    pub sensitive_headers: Vec<String>,
    pub sensitive_body_fields: Vec<String>,
    pub token_show_head: usize,
    pub token_show_tail: usize,
    pub url_preserve_path_segments: usize,
}

impl AppConfig {
    /// 加载顺序：1) env PLS_GATEWAY_CONFIG 覆盖 → 2) ./config/gateway.yaml → 3) 默认内置
    pub async fn load_or_default() -> AppResult<Self> {
        let path = if let Ok(p) = std::env::var("PLS_GATEWAY_CONFIG") {
            std::path::PathBuf::from(p)
        } else {
            // 默认写到用户配置目录（目标机上 CARGO_MANIFEST_DIR 不存在，无法读写）
            APP_DIRS.config_dir.join("gateway.yaml")
        };
        if !path.exists() {
            tracing::warn!("config file not found: {} → using baked-in default", path.display());
            return Ok(Self::default_inline());
        }
        let txt = tokio::fs::read_to_string(&path).await?;
        let cfg: Self = serde_yaml::from_str(&txt)
            .map_err(|e| crate::error::AppError::Config(format!("gateway.yaml parse fail: {e}")))?;
        Ok(cfg)
    }

    fn default_inline() -> Self {
        // 使用最小可用默认值（保证能启动），实际生产请一定用 gateway.yaml
        let def = include_str!("../config/gateway.yaml");
        serde_yaml::from_str(def).expect("bundled gateway.yaml must be valid")
    }

    /// 解析 gateway.yaml 的磁盘路径（env 覆盖 → CARGO_MANIFEST_DIR/config/gateway.yaml）
    pub fn disk_path() -> std::path::PathBuf {
        if let Ok(p) = std::env::var("PLS_GATEWAY_CONFIG") {
            std::path::PathBuf::from(p)
        } else {
            APP_DIRS.config_dir.join("gateway.yaml")
        }
    }
}

/// 同步把配置写回磁盘 gateway.yaml（桌面壳 IPC 用）
pub fn save_to_disk(cfg: &AppConfig) -> AppResult<AppConfig> {
    let path = AppConfig::disk_path();
    let txt = serde_yaml::to_string(cfg)
        .map_err(|e| crate::error::AppError::Config(format!("gateway.yaml serialize fail: {e}")))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(crate::error::AppError::Io)?;
    }
    std::fs::write(&path, txt).map_err(crate::error::AppError::Io)?;
    tracing::info!("gateway.yaml saved → {}", path.display());
    Ok(cfg.clone())
}

/// 重置为 bundled 默认配置并写回磁盘
pub fn reset_to_default() -> AppResult<AppConfig> {
    let def = include_str!("../config/gateway.yaml");
    let cfg: AppConfig = serde_yaml::from_str(def)
        .map_err(|e| crate::error::AppError::Config(format!("bundled gateway.yaml invalid: {e}")))?;
    save_to_disk(&cfg)?;
    Ok(cfg)
}
