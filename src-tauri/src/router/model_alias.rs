//! 别名解析（model_aliases table 匹配 + glob 兜底）
use crate::config::{AppConfig, ModelAlias};
use crate::error::{AppError, AppResult, ErrorLabel};
pub struct ResolvedAlias { pub real_model: String, pub group: String, pub cache_enable: bool, pub ttl_seconds: Option<u64>, pub charge_on_cache_hit: bool }
pub fn resolve_alias(cfg: &AppConfig, client_model: &str) -> AppResult<ResolvedAlias> {
    // 1) 精确匹配（跳过已暂停 alias）
    if let Some(m) = cfg.model_aliases.iter().find(|a| a.enabled && a.alias == client_model) { return Ok(from(m)); }
    // 2) glob 简化匹配：通配符 * 结尾（跳过已暂停 alias）
    if let Some(m) = cfg.model_aliases.iter().find(|a| a.enabled && glob_match(&a.alias, client_model)) { return Ok(from(m)); }
    // 3) 上游真实模型名直连：客户端传的是真实模型名（如 deepseek-chat / llama3:latest）也能路由
    if let Some(m) = cfg.model_aliases.iter().find(|a| a.enabled && a.real_model == client_model) { return Ok(from(m)); }
    Err(AppError::Labeled { label: ErrorLabel::BadParam4xx, message: format!("model alias not found or paused: {client_model}") })
}
fn from(m: &ModelAlias) -> ResolvedAlias { ResolvedAlias { real_model: m.real_model.clone(), group: m.group.clone(), cache_enable: m.cache_enable, ttl_seconds: m.ttl_seconds, charge_on_cache_hit: m.charge_on_cache_hit } }
fn glob_match(pat: &str, s: &str) -> bool {
    if let Some(prefix) = pat.strip_suffix('*') { s.starts_with(prefix) } else { false } }
