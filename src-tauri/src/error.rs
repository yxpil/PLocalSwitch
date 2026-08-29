//! =============================================================
//!  统一错误类型 + 网关错误标签枚举（13 个分类，对应 Prometheus 分桶 + 研判规则）
//! =============================================================
use thiserror::Error;

/// ✅ 交付物 3 & 4：错误标签（全量枚举，用于研判/Prometheus/回写 trace）
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
    serde::Serialize, serde::Deserialize,
    strum::EnumString, strum::Display, strum::EnumIter,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ErrorLabel {
    NetworkConnectRefused,
    DnsFail,
    ConnectTimeout,
    ReadTimeout,
    TlsError,
    Http429,
    Auth401403,
    BadParam4xx,
    Upstream5xx,
    SsePrematureClose,
    SseFormatInvalid,
    SseMidDrop,
    JsonParseFail,
    SchemaMismatch,
    Internal,
    Unknown,
}
impl ErrorLabel {
    /// 是否属于 "可重试非流式" 的集合（结合 policy 判断）
    pub fn is_candidate_retry(&self, p: &crate::config::RetryOnCfg) -> bool {
        use ErrorLabel::*;
        match self {
            NetworkConnectRefused => p.network_connect_refused,
            DnsFail               => p.dns_fail,
            ConnectTimeout        => p.connect_timeout,
            ReadTimeout           => p.read_timeout,
            TlsError              => p.tls_error,
            Http429               => p.http_429,
            Auth401403            => p.auth_401_403,
            BadParam4xx           => p.bad_param_4xx,
            Upstream5xx           => p.http_5xx,
            SsePrematureClose     => p.sse_premature_close,
            JsonParseFail         => p.json_parse_fail,
            SseFormatInvalid | SseMidDrop | SchemaMismatch | Internal | Unknown => false,
        }
    }
    /// 是否流式锁死后仍可安全重试？→ 原则 4：一律 false
    pub fn stream_ever_retry(&self) -> bool { false }
}

/// 统一错误
#[derive(Debug, Error)]
pub enum AppError {
    #[error("IO 错误: {0}")]     Io(#[from] std::io::Error),
    #[error("Serde JSON: {0}")]  SerdeJson(#[from] serde_json::Error),
    #[error("YAML: {0}")]        SerdeYaml(#[from] serde_yaml::Error),
    #[cfg(feature = "desktop-shell")]
    #[error("Tauri: {0}")]       Tauri(#[from] tauri::Error),
    #[error("配置: {0}")]        Config(String),
    #[error("业务: {0}")]        Business(String),
    #[error("路径非法: {0}")]    InvalidPath(String),
    #[error("文件未找到: {0}")]  FileNotFound(String),
    #[error("Reqwest: {0}")]     Reqwest(#[from] reqwest::Error),
    #[error("SQLx: {0}")]        Sqlx(#[from] sqlx::Error),
    #[error("Axum: {0}")]        Axum(String),

    /// 网关自身：附标签的错误（所有上游/网络错误都走这一路，便于观测/重试/研判）
    #[error("label={label:?} {message}")]
    Labeled { label: ErrorLabel, message: String },

    /// AUTOMODE/多候选链全失败：detail 为已尝试源的 host 级清单（只含 host+状态码，绝无 token）
    /// 应用户要求必须告知「试过哪些链接」，to_openai_error 会把它放进 message。
    #[error("候选链失败: {detail}")]
    ChainFailed { detail: String },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl AppError {
    /// 错误分类（从具体形态推断）
    pub fn label(&self) -> ErrorLabel {
        use AppError::*;
        use ErrorLabel::*;
        match self {
            Labeled { label, .. } => *label,
            ChainFailed { .. } => Upstream5xx,
            Reqwest(e) => reqwest_to_label(e),
            Sqlx(_)    => Internal,
            Io(e)      => io_to_label(e),
            Config(_)  => Internal,
            Business(_)=> BadParam4xx,
            InvalidPath(_) | FileNotFound(_) => BadParam4xx,
            _ => Unknown,
        }
    }
    /// 对外（客户端）OpenAI 错误体（绝对不泄露上游地址/token/诊断）
    pub fn to_openai_error(&self) -> serde_json::Value {
        use AppError::*;
        // 超时 / 429 特判（更贴近客户端体验）
        if let Reqwest(e) = self {
            if e.is_timeout() {
                return serde_json::json!({"error":{"message":"Upstream request timed out.","type":"gateway_error","code":"timeout"}});
            }
            if e.status() == Some(reqwest::StatusCode::TOO_MANY_REQUESTS) {
                return serde_json::json!({"error":{"message":"Rate limited.","type":"gateway_error","code":"rate_limit_exceeded"}});
            }
        }
        // 其余统一按 label 分类，保证与 HTTP 状态码映射（error_resp.rs）完全一致，
        // 避免出现 503 却带 internal_error 的矛盾响应。
        let lbl = self.label();
        // 候选链全失败：用户明确要求透出「试过哪些源」（host 级+状态码，无 token/路径查询串）
        if let AppError::ChainFailed { detail } = self {
            return serde_json::json!({
                "error": {
                    "message": detail,
                    "type":    "gateway_error",
                    "code":    "upstream_chain_failed",
                }
            });
        }
        let (code, message) = (lbl.to_string().clone(), client_message_for_label(&lbl));
        serde_json::json!({
            "error": {
                "message": message,
                "type":    "gateway_error",
                "code":    code,
            }
        })
    }
}

/// 标签 → 客户端安全消息（pub 供流式错误脱敏复用）
pub fn client_message_for_label_pub(l: &ErrorLabel) -> &'static str {
    client_message_for_label(l)
}

fn client_message_for_label(l: &ErrorLabel) -> &'static str {
    use ErrorLabel::*;
    match l {
        BadParam4xx       => "Invalid request parameters.",
        Auth401403        => "Invalid gateway API key.",
        Http429           => "Too many requests — throttle.",
        NetworkConnectRefused | DnsFail | ConnectTimeout | TlsError | ReadTimeout
                          => "All upstream endpoints unavailable.",
        Upstream5xx       => "Upstream provider returned server error.",
        SsePrematureClose | SseFormatInvalid | SseMidDrop
                          => "Stream closed abnormally by upstream.",
        JsonParseFail | SchemaMismatch
                          => "Upstream returned unrecognized response.",
        Internal | Unknown => "Internal gateway error.",
    }
}

fn io_to_label(e: &std::io::Error) -> ErrorLabel {
    use std::io::ErrorKind::*;
    match e.kind() {
        ConnectionRefused => ErrorLabel::NetworkConnectRefused,
        TimedOut          => ErrorLabel::ConnectTimeout,
        _                 => ErrorLabel::Internal,
    }
}
fn reqwest_to_label(e: &reqwest::Error) -> ErrorLabel {
    use ErrorLabel::*;
    if let Some(s) = e.status() {
        match s.as_u16() {
            401 | 403 => Auth401403,
            429 => Http429,
            400 | 404 | 405 | 406..=499 => BadParam4xx,
            500..=599 => Upstream5xx,
            _ => Unknown,
        }
    } else if e.is_connect() {
        NetworkConnectRefused
    } else if e.is_timeout() {
        use std::error::Error as _StdErr;
        if _StdErr::source(e).map(|x| x.to_string().contains("timed out before connect")).unwrap_or(false) {
            ConnectTimeout
        } else {
            ReadTimeout
        }
    } else if e.is_body() || e.is_decode() {
        JsonParseFail
    } else if e.to_string().to_lowercase().contains("dns") {
        DnsFail
    } else if e.to_string().to_lowercase().contains("tls")
           || e.to_string().to_lowercase().contains("cert")
           || e.to_string().to_lowercase().contains("handshake") {
        TlsError
    } else {
        Unknown
    }
}

// ===== IPC & API 序列化（保持不泄密）=====
impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // IPC 返回给管理 UI 时也走 OpenAI 式安全字段（脱敏）
        let payload = self.to_openai_error();
        payload.serialize(s)
    }
}

pub type AppResult<T> = anyhow::Result<T, AppError>;
pub type CommandResult<T> = Result<T, AppError>;
