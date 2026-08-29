//! =============================================================
//!  路由层（交付物 1：模型别名 → 后端节点组 → 权重/主备/降级）
//! =============================================================
//!  输入：客户端请求别名（如 `gpt-4o-mini`）
//!  输出：候选 [`CandidateNode`] 列表，按路由策略排序
//!
//!  参与要素：
//!    • cfg.model_aliases  别名 → 真实模型 & 目标组名
//!    • cfg.node_groups    节点组（含若干 upstream_node，权重/主备）
//!    • node_quality       实时得分（低分降权、低于阈值自动摘除）
//!    • cache key 命中时可跳过下游请求（在 cache_pool 模块拦截）
//! =============================================================
pub mod model_alias;
pub mod group_selector;
pub mod fallback_policy;

use crate::error::AppResult;
use crate::state::AppState;
use std::sync::Arc;

/// 路由解析后的上游候选节点（含脱敏地址/协议/真实模型名）
#[derive(Clone)]
pub struct CandidateNode {
    pub node_id:      String,
    pub group_id:     String,
    pub real_model:   String,
    pub endpoint:     String,       // 内部使用，落盘前必须脱敏
    pub protocol:     ProtocolKind, // 硬编码适配器要走哪条
    pub candidate_protocols: Vec<ProtocolKind>, // 非流式嗅探时的顺序（柔性层用）
    pub weight:       f64,
    pub quality:      u8,           // 0..=100
    pub api_key_name: String,       // 脱敏 key 前缀，用于 observability
    pub free:         bool,         // 免费源标记（AUTOMODE 免费优先排序用）
    #[doc(hidden)] pub _api_key:   String, // 明文，仅请求过程内存中存在，严禁落盘
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, strum::EnumString, strum::Display, serde::Serialize, serde::Deserialize)]
#[strum(serialize_all = "snake_case")]
pub enum ProtocolKind {
    OpenAI,               // OpenAI Chat-Completions
    #[strum(serialize = "openai_response")]
    OpenAIResponse,       // OpenAI Responses API（/v1/responses，如 grok）
    Anthropic,            // Anthropic Messages
    Gemini,               // Google Gemini generateContent
    BedrockConverse,      // AWS Bedrock Converse (SigV4)
    CohereV2,             // Cohere v2
    Qianfan,              // 百度千帆 (Ernie)
    DashScope,            // 阿里 DashScope (Qwen)
    Spark,                // 讯飞星火
    Hunyuan,              // 腾讯混元
    Ollama,               // Ollama Native /api/chat
    Vllm,                 // vLLM OpenAI 兼容服务
    Tgi,                  // Text-Generation-Inference
    CustomOpenAICompat,   // 各类"魔改 OpenAI"非标推理服务（flex 嗅探兜底）
}

impl CandidateNode {
    /// 对外/落盘/日志版本：脱敏后的结构体（需传入 MaskingConfig 上下文）
    pub fn to_masked(&self, cfg: &crate::config::MaskingConfig) -> MaskedCandidateNode {
        use crate::observability::masking::{mask_endpoint, mask_token};
        MaskedCandidateNode {
            node_id:    self.node_id.clone(),
            group_id:   self.group_id.clone(),
            real_model: self.real_model.clone(),
            endpoint:   mask_endpoint(&self.endpoint, cfg),
            protocol:   self.protocol,
            api_key:    mask_token(&self._api_key, cfg),
        }
    }
}
#[derive(Clone, serde::Serialize)]
pub struct MaskedCandidateNode {
    pub node_id:    String,
    pub group_id:   String,
    pub real_model: String,
    pub endpoint:   String,
    pub protocol:   ProtocolKind,
    pub api_key:    String,
}

/// 路由入口：解析 client_model 并返回排序后的候选节点列表
///
/// client_model 支持「host|model」源定向格式（如 `integrate.api.nvidia.com|moonshotai/kimi-k3`）：
/// 多个上游提供同名模型时，前端按「源 · 模型」展示，带前缀即可定向到指定源测试。
pub async fn route_client_request(
    state:       &Arc<AppState>,
    client_model: &str,
    is_stream:   bool,
) -> AppResult<Vec<CandidateNode>> {
    use model_alias::resolve_alias;
    use group_selector::expand_candidates;
    use fallback_policy::sort_and_trim;

    // 拆分「host|model」源定向前缀（无前缀则原样）
    let (prefer_host, client_model): (Option<String>, String) = match client_model.split_once('|') {
        Some((h, m)) if !h.is_empty() && !m.is_empty()
            => (Some(h.trim().to_ascii_lowercase()), m.trim().to_string()),
        _ => (None, client_model.trim().to_string()),
    };

    // AUTOMODE 虚拟模型：全目录「源×模型」自动尝试降级（设置中可关）
    if client_model.eq_ignore_ascii_case("automode") {
        let enabled = state.cfg_swap.load().automode.enabled;
        if !enabled {
            return Err(crate::error::AppError::Labeled {
                label: crate::error::ErrorLabel::BadParam4xx,
                message: "AUTOMODE is disabled in settings".into(),
            });
        }
        let cands = automode_candidates(state, is_stream).await?;
        if cands.is_empty() {
            return Err(crate::error::AppError::Labeled {
                label: crate::error::ErrorLabel::BadParam4xx,
                message: "AUTOMODE: no available models (catalog empty, visit chat page to refresh)".into(),
            });
        }
        return Ok(cands);
    }

    // 先按别名/真实模型匹配；都不中则查「模型→上游组」目录（组合键 host|model）。
    // 别名解析读 cfg_swap（跟随热更新）；guard 在当前语句后立即释放。
    let resolved = match {
        let cfg = state.cfg_swap.load();
        resolve_alias(&cfg, &client_model)
    } {
        Ok(r) => r,
        Err(_) => {
            let groups = catalog_groups(state, &client_model);
            if groups.is_empty() {
                return Err(crate::error::AppError::Labeled {
                    label: crate::error::ErrorLabel::BadParam4xx,
                    message: format!("model alias not found or paused: {client_model}"),
                });
            }
            model_alias::ResolvedAlias {
                real_model: client_model.clone(),
                group: groups[0].clone(),
                cache_enable: false,
                ttl_seconds: None,
                charge_on_cache_hit: false,
            }
        }
    };
    let mut cands = Vec::new();
    // 主路径（扁平模型路由）：按「模型目录」直接找到真正服务该模型的所有节点组，
    // 多节点同模型自动合并候选（上层按权重/轮询选择）。本地网关无需手工配置分组。
    {
        let groups: Vec<String> = catalog_groups(state, &resolved.real_model);
        for gid in groups {
            let fb = model_alias::ResolvedAlias {
                real_model: resolved.real_model.clone(),
                group: gid,
                cache_enable: false,
                ttl_seconds: None,
                charge_on_cache_hit: false,
            };
            let v = expand_candidates(state, &fb, is_stream).await?;
            cands.extend(v);
        }
        // 同一节点可能出现在多个组 → 按 node_id 去重，保留首个
        let mut seen = std::collections::HashSet::new();
        cands.retain(|c| seen.insert(c.node_id.clone()));
    }
    // 兜底 1：目录未收录（如上游 /v1/models 不可用）时，按别名配置的分组展开
    if cands.is_empty() {
        cands = expand_candidates(state, &resolved, is_stream).await?;
    }
    // 兜底 2（宁滥不缺）：别名分组也没有可用节点时，回退到任一「启用且有节点」的组，
    // 尽量让请求能发出去
    if cands.is_empty() {
        let cfg = state.cfg_swap.load();
        for g in cfg.node_groups.iter().filter(|g| g.enabled && g.nodes.iter().any(|n| n.enabled && !n.hard_disable)) {
            let fb = model_alias::ResolvedAlias {
                real_model: resolved.real_model.clone(),
                group: g.id.clone(),
                cache_enable: false,
                ttl_seconds: None,
                charge_on_cache_hit: false,
            };
            let v = expand_candidates(state, &fb, is_stream).await?;
            if !v.is_empty() { cands = v; break; }
        }
    }
    // 源定向过滤：仅保留 endpoint 命中偏好 host 的候选（全不命中则忽略偏好，走原候选）
    if let Some(h) = &prefer_host {
        let filtered: Vec<CandidateNode> = cands.iter()
            .filter(|c| endpoint_host_matches(&c.endpoint, h))
            .cloned()
            .collect();
        if !filtered.is_empty() { cands = filtered; }
    }
    sort_and_trim(state, &mut cands);
    Ok(cands)
}

/// endpoint 的 host 部分是否命中偏好源（宽松匹配，兼容协议前缀/端口差异）
fn endpoint_host_matches(endpoint: &str, prefer: &str) -> bool {
    let host = endpoint_host(endpoint);
    !host.is_empty() && (host == prefer || host.starts_with(prefer) || prefer.starts_with(&host))
}

/// 从 endpoint 提取 host[:port]（去协议、去路径，小写）
fn endpoint_host(endpoint: &str) -> String {
    let e = endpoint.trim().trim_end_matches('/');
    let rest = e.split_once("://").map(|(_, r)| r).unwrap_or(e);
    rest.split('/').next().unwrap_or("").to_ascii_lowercase()
}

/// AUTOMODE：把模型目录里每个「源×模型」条目展开成候选（各带自己的真实模型名），
/// 交给柔性层重试链自动降级——源越多越稳。同一 (节点,模型) 去重；
/// 候选爆炸防护：均匀抽样至多 24 个（保持源分布，不集中单点）。
async fn automode_candidates(state: &Arc<AppState>, is_stream: bool) -> AppResult<Vec<CandidateNode>> {
    let snapshot: Vec<(Vec<String>, String)> = state.node_runtime.model_catalog.iter()
        .map(|e| {
            let key = e.key();
            let model = key.rsplit('|').next().unwrap_or(key).to_string();
            (e.value().clone(), model)
        })
        .collect();
    let mut cands: Vec<CandidateNode> = Vec::new();
    for (groups, model) in snapshot {
        for gid in groups {
            let fb = model_alias::ResolvedAlias {
                real_model: model.clone(),
                group: gid,
                cache_enable: false,
                ttl_seconds: None,
                charge_on_cache_hit: false,
            };
            let v = crate::router::group_selector::expand_candidates(state, &fb, is_stream).await.unwrap_or_default();
            cands.extend(v);
        }
    }
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    cands.retain(|c| seen.insert((c.node_id.clone(), c.real_model.clone())));
    if cands.len() > 24 {
        let step = cands.len() as f64 / 24.0;
        cands = (0..24).map(|i| cands[((i as f64) * step) as usize].clone()).collect();
    }
    // 质量优先排序：好源先试，失败再降级
    crate::router::fallback_policy::sort_and_trim(state, &mut cands);
    Ok(cands)
}

/// 模型目录查询：目录 key 为组合键 `host|model`（同名模型按源隔离，互不覆盖）。
/// - 传入含 `|` 的定向名 → 精确匹配该源
/// - 传入纯模型名 → 精确匹配（无源条目）+ 收集所有 `xxx|model` 条目（同名多源全合并）
fn catalog_groups(state: &Arc<AppState>, model: &str) -> Vec<String> {
    let cat = &state.node_runtime.model_catalog;
    if let Some(g) = cat.get(model) {
        let v = g.value().clone();
        if !v.is_empty() { return v; }
    }
    if !model.contains('|') {
        let suffix = format!("|{model}");
        let mut out: Vec<String> = Vec::new();
        for e in cat.iter() {
            if e.key().ends_with(&suffix) {
                for gid in e.value() {
                    if !out.contains(gid) { out.push(gid.clone()); }
                }
            }
        }
        return out;
    }
    Vec::new()
}

/// 拉取所有启用节点的真实模型列表，重建「模型 → 上游组」目录。
/// 启动时与聊天页刷新共用；同时返回「模型×源」列表（id 形如 `host|model`，供前端展示与源定向路由）。
pub async fn refresh_model_catalog(app: &Arc<AppState>) -> Vec<serde_json::Value> {
    let cfg = app.cfg_swap.load();
    let client = reqwest::Client::builder().no_proxy()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(6))
        .build().unwrap_or_else(|_| reqwest::Client::new());

    let mut tasks = Vec::new();
    for g in &cfg.node_groups {
        for n in &g.nodes {
            if !n.enabled || n.hard_disable { continue; }
            let proto = n.protocol_hints.first().cloned().unwrap_or_default();
            let endpoint = n.endpoint.trim_end_matches('/').to_string();
            let key = n.api_keys.first().cloned().unwrap_or_default();
            let group = g.id.clone();
            // host 由节点自己携带：同组多节点（如 manual 组混装 packyapi/deepseek 官方/智谱）时
            // 各模型标注真实来源，绝不按「组内第一个节点」反查（那会把整组模型张冠李戴）
            let host = endpoint_host(&endpoint);
            let c = client.clone();
            tasks.push(async move { (group, host, fetch_node_models(&c, &endpoint, &key, &proto).await) });
        }
    }
    // 重建目录：每个模型绑定到真正服务它的组（可多组，路由自动轮询）
    app.node_runtime.model_catalog.clear();
    let results = futures::future::join_all(tasks).await;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<serde_json::Value> = Vec::new();
    // 1) 上游真实模型（来自 /v1/models 或 /api/tags），按「源」展开为 host|model 条目
    for (group, host, ids) in results {
        for id in ids {
            // 组合键 host|model：同名模型按源隔离（如 packyapi 与 deepseek 官方都叫 deepseek-v4-flash）
            let cat_key = if host.is_empty() { id.clone() } else { format!("{host}|{id}") };
            let mut e = app.node_runtime.model_catalog.entry(cat_key).or_default();
            if !e.contains(&group) { e.push(group.clone()); }
            drop(e);
            let key = if host.is_empty() { id.clone() } else { format!("{host}|{id}") };
            if seen.insert(key.clone()) {
                out.push(serde_json::json!({ "id": key, "model": id, "host": host, "group": group }));
            }
        }
    }
    // 2) 别名真实模型（即使上游 /v1/models 不返回，如 deepseek-chat，也能路由；不带源前缀）
    for a in &cfg.model_aliases {
        if a.enabled {
            app.node_runtime.model_catalog.entry(a.real_model.clone()).or_insert_with(|| vec![a.group.clone()]);
            if seen.insert(a.real_model.clone()) {
                out.push(serde_json::json!({ "id": a.real_model, "model": a.real_model, "host": "", "group": a.group }));
            }
        }
    }
    // 排序：别名（host 空）优先，其余按「源域名 + 模型名」字母序——
    // 避免上游模型数百条时官方源条目沉底（如 api.deepseek.com 被按配置顺序压在 NVIDIA 后面）
    out.sort_by(|a, b| {
        let ha = a["host"].as_str().unwrap_or_default();
        let hb = b["host"].as_str().unwrap_or_default();
        ha.cmp(hb).then_with(|| {
            a["model"].as_str().unwrap_or_default().cmp(b["model"].as_str().unwrap_or_default())
        })
    });
    out
}

/// OpenAI 兼容探针 URL：带路径 endpoint（如智谱 /api/paas/v4）追加 /models，纯域名补 /v1/models
pub(crate) fn models_probe_url(endpoint: &str) -> String {
    let e = endpoint.trim_end_matches('/');
    if e.ends_with("/models") { return e.to_string(); }
    let has_path = e.split_once("://").map(|(_, rest)| rest.contains('/')).unwrap_or(false);
    if has_path { format!("{e}/models") } else { format!("{e}/v1/models") }
}

/// 从单个上游节点拉取真实模型列表：Ollama /api/tags、其余 OpenAI 兼容 /v1/models
pub(crate) async fn fetch_node_models(client: &reqwest::Client, endpoint: &str, key: &str, proto: &str) -> Vec<String> {
    let url = if proto == "ollama" { format!("{}/api/tags", endpoint.trim_end_matches('/')) } else { models_probe_url(endpoint) };
    let mut req = client.get(&url);
    if !key.is_empty() { req = req.header("Authorization", format!("Bearer {key}")); }
    let Ok(resp) = req.send().await else { return vec![]; };
    if !resp.status().is_success() { return vec![]; }
    let Ok(text) = resp.text().await else { return vec![]; };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return vec![]; };
    let mut out = Vec::new();
    if proto == "ollama" {
        if let Some(arr) = v.get("models").and_then(|m| m.as_array()) {
            for m in arr { if let Some(name) = m.get("name").and_then(|x| x.as_str()) { out.push(name.to_string()); } }
        }
    } else {
        if let Some(arr) = v.get("data").and_then(|m| m.as_array()) {
            for m in arr { if let Some(id) = m.get("id").and_then(|x| x.as_str()) { out.push(id.to_string()); } }
        }
    }
    out
}
