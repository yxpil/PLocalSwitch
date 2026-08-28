//! =============================================================
//!  7. 节点质量评估（0-100，样本不足保护；仅路由辅助权重，不强制阻断）
//! =============================================================
//!  评估项：
//!    - 业务成功率（排除客户端 400 错误）
//!    - P50/P95/P99 延迟、TTFT
//!    - 429 / 5xx / SSE 断流 / JSON 解析失败 / 重试触发频次
//!    - token 对账平均偏差率（越大越扣分）
//!    - SSE 不合格、SSE 异常断开累计计数
//!  最小样本阈值（min_samples = 30）：不达标不参与打分，防止抖动误判。
//! =============================================================
pub mod scoring;
pub mod sample_buffer;       // 环形定长样本缓冲（DashMap<String, RingBuf<Sample>>）
pub mod label_classifier;    // 根据分数输出 优秀/良好/一般/较差/故障 5 档
pub mod autotrim;            // 低质量自动降权 / 临时摘除（可在配置关闭）

use crate::error::AppResult;
use crate::state::AppState;
use std::sync::Arc;

/// 启动后台打分任务（每 15s 一次重新计算）
pub async fn spawn_quality_scoring_loop(_state: Arc<AppState>) {
    // TODO
}

/// 查询某节点最新质量分（0..=100）
pub fn quality_of(_state: &Arc<AppState>, _node_id: &str) -> Option<u8> {
    None // TODO
}
