//! =============================================================
//!  8. 观测层（Trace 全局 ID / SubAttempt 链路 / Prometheus）
//! =============================================================
//!  - 每个客户端请求生成唯一 trace_id（uuid v7，时间有序）
//!  - 每次上游尝试产生 sub_attempt_id（ulid），与 trace_id 关联
//!  - 所有敏感字段（endpoint、token）入库前必须通过 masking 模块脱敏
//!  - 故障研判 human_readable_reason 仅对内，禁止在 gateway_api 错误响应中透传
//! =============================================================
pub mod masking;            // ✅ 交付物 3 & 4 脱敏工具函数（地址/Token 打码）
pub mod trace;              // ✅ GatewayTrace / SubAttempt 核心结构体（交付物 3）
pub mod fingerprint;        //   上游指纹识别记录（交付物 3）
pub mod audit_record;       //   对账记录（交付物 3）
pub mod metrics_registry;   //   Prometheus 全部指标（error_label 分桶）

use crate::state::AppState;
use std::sync::Arc;

/// 启动 metrics flush（pushgateway/本地 registry，按配置）
pub async fn spawn_metrics_flush_loop(_state: Arc<AppState>) {
    // TODO
}
