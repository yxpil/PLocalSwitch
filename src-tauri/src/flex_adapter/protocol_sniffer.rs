//! =============================================================
//!  交付物 4：protocol_sniffer（仅非流式）多候选协议嗅探 + 成功记忆缓存
//!  · 节点首次请求时，按 node.protocol_hints 顺序依次尝试 ProtocolKind 全部候选
//!  · 嗅探成功后写 state.node_runtime.proto_sniff DashMap (node_id → (ProtocolKind, expire_until))
//!  · 下次同一 node_id 在 expire_until 之前直接跳过嗅探，用已记忆协议
//!  · 失败计数达 sniff_attempts_per_node 则认为 hints 错 → 标记 Unknown，走最后 fallback
//! =============================================================
use crate::config::FlexAdapterConfig;
use crate::error::{AppError, AppResult, ErrorLabel};
use crate::router::ProtocolKind;
use crate::state::AppState;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub type SniffCache = DashMap<String, CachedProtocol>;

#[derive(Debug, Clone)]
pub struct CachedProtocol {
    pub protocol:     ProtocolKind,
    pub expire_at_ms: u128,
}

/// 返回某 node_id 的嗅探结果：
///   - Some(protocol)：命中缓存或已嗅探成功
///   - None：需要立即跑嗅探（调用 run_sniff）
pub fn try_cached(state: &Arc<AppState>, node_id: &str) -> Option<ProtocolKind> {
    let v = state.node_runtime.proto_sniff.get(node_id)?;
    if now_ms() < v.expire_at_ms { Some(v.protocol) } else { None }
}

/// 写缓存：成功后调用，记忆 sniff_remember_ttl_seconds 秒
pub fn remember(state: &Arc<AppState>, node_id: impl Into<String>, proto: ProtocolKind, cfg: &FlexAdapterConfig) {
    let until = now_ms() + (cfg.sniff_remember_ttl_seconds as u128) * 1000;
    state.node_runtime.proto_sniff.insert(node_id.into(), CachedProtocol { protocol: proto, expire_at_ms: until });
}

/// 嗅探候选序列：按 hints 的协议名 → ProtocolKind；末尾兜底 = [OpenAICompat, AnthropicCompat, GeminiCompat]
pub fn candidate_sequence(hints: &[String]) -> Vec<ProtocolKind> {
    use ProtocolKind::*;
    let mut out: Vec<ProtocolKind> = hints.iter().filter_map(|h| h.parse().ok()).collect();
    // 保证唯一
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|p| seen.insert(*p));
    for fallback in [OpenAI, Anthropic, Gemini] {
        if !seen.contains(&fallback) { out.push(fallback); }
    }
    out
}

/// 嗅探执行伪代码（真实在 attempt_loop 里内嵌对每个 protocol 调 adapter.execute_once）
pub async fn run_sniff(
    state: &Arc<AppState>, node_id: &str, endpoint: &str, keys: &[String],
    hints: &[String], cfg: &FlexAdapterConfig,
    probe_once: impl Fn(ProtocolKind) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), AppError>> + Send>>,
) -> AppResult<ProtocolKind> {
    let candidates = candidate_sequence(hints);
    let max = cfg.sniff_attempts_per_node.max(1) as usize;
    let mut tries = 0usize;
    for p in candidates.into_iter().take(max) {
        tries += 1;
        let _ = (state, node_id, endpoint, keys); // 实际调用 adapter_for(p).execute_once()
        match probe_once(p).await {
            Ok(()) => { remember(state, node_id, p, cfg); return Ok(p); }
            Err(e) => {
                // 只有 5xx/连接/TLS/404/解析失败 才继续下一个协议；BadParam4xx 立即视为协议成功但请求非法（提前 break）
                if matches!(e.label(), ErrorLabel::BadParam4xx) { return Ok(p); }
            }
        }
    }
    let _ = tries;
    Err(AppError::Labeled {
        label: ErrorLabel::SchemaMismatch,
        message: format!("protocol sniff exhausted for node {node_id}"),
    })
}

pub fn now_ms() -> u128 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0) }
