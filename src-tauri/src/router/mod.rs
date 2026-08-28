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
pub async fn route_client_request(
    state:       &Arc<AppState>,
    client_model: &str,
    is_stream:   bool,
) -> AppResult<Vec<CandidateNode>> {
    use model_alias::resolve_alias;
    use group_selector::expand_candidates;
    use fallback_policy::sort_and_trim;

    // 先按别名/真实模型匹配；都不中则查“模型→上游组”目录（模型↔API 匹配）
    let resolved = match resolve_alias(&state.cfg, client_model) {
        Ok(r) => r,
        Err(_) => match state.node_runtime.model_catalog.get(client_model) {
            Some(g) => model_alias::ResolvedAlias {
                real_model: client_model.to_string(),
                group: g.value().clone(),
                cache_enable: false,
                ttl_seconds: None,
                charge_on_cache_hit: false,
            },
            None => return Err(crate::error::AppError::Labeled {
                label: crate::error::ErrorLabel::BadParam4xx,
                message: format!("model alias not found or paused: {client_model}"),
            }),
        },
    };
    let mut cands  = expand_candidates(state, &resolved, is_stream).await?;
    sort_and_trim(state, &mut cands);
    Ok(cands)
}
