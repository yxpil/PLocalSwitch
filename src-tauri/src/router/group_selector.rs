//! 组内候选展开（权重/主备/临时 ban 过滤）
use crate::error::AppResult;
use crate::router::{CandidateNode, ProtocolKind};
use crate::state::AppState;
use crate::observability::masking::{mask_endpoint, mask_token};
use super::model_alias::ResolvedAlias;
use std::sync::Arc;
pub async fn expand_candidates(state: &Arc<AppState>, r: &ResolvedAlias, _stream: bool) -> AppResult<Vec<CandidateNode>> {
    let cfg = state.cfg_swap.load();
    // gw8b：整组暂停 → 直接空候选（上层 400 no routing target）
    let Some(group) = cfg.node_groups.iter().find(|g| g.enabled && g.id == r.group) else { return Ok(vec![]); };
    let mask_cfg = &cfg.masking;
    let mut out = Vec::new();
    for n in &group.nodes {
        // gw8b：单节点暂停（手动 enabled=false）与 hard_disable 同级过滤
        if !n.enabled || n.hard_disable { continue; }
        // 临时 ban 检查（仅当 autotrim 开启 + temp_ban_until 没过期时跳过）
        let banned = state.node_runtime.temp_ban_until.get(&n.id).map(|t| *t as u128 > now_ms()).unwrap_or(false);
        if banned { continue; }
        let proto = n.protocol_hints.first().and_then(|p| p.parse().ok()).unwrap_or(ProtocolKind::OpenAI);
        let key = n.api_keys.first().cloned().unwrap_or_default();
        out.push(CandidateNode {
            node_id: n.id.clone(), group_id: group.id.clone(), real_model: r.real_model.clone(),
            endpoint: n.endpoint.clone(), protocol: proto,
            candidate_protocols: crate::flex_adapter::protocol_sniffer::candidate_sequence(&n.protocol_hints),
            weight: n.weight, quality: crate::node_quality::quality_of(state, &n.id).unwrap_or(50),
            // 免费源自动识别：大多数免费 API 的模型名或端点自带 free 关键字
            free: {
                let m = r.real_model.to_ascii_lowercase();
                let e = n.endpoint.to_ascii_lowercase();
                m.contains("free") || e.contains("free")
            },
            api_key_name: mask_token(&key, mask_cfg),
            _api_key: key,
        });
    }
    Ok(out)
}
pub fn now_ms() -> u128 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0) }
