//! 节点能力探测缓存（toolcall/multimodal/stream/window size）
use crate::models::ChatCompletionRequest;
use crate::router::CandidateNode;
use crate::state::AppState;
use std::sync::Arc;

/// 启动后台探测任务（每个 node_group 周期性探测 capability 缓存）
pub async fn spawn_probe_loop(_state: Arc<AppState>) {
    // TODO: 对每个 UpstreamNode 定期探测（首 token 延迟/支持字段），写 state.node_runtime.capabilities
}

pub async fn apply_capability_constraints(_state: &Arc<AppState>, _c: &[CandidateNode], _req: &mut ChatCompletionRequest) {
    // TODO: 查 state.node_runtime.capabilities；不支持字段从 req 中剥离
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Capabilities {
    pub tool_calls: bool,
    pub multimodal_image: bool,
    pub native_stream: bool,
    pub response_format_json_schema: bool,
}
