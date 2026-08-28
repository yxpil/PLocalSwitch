//! =============================================================
//!  9. 自身运行时质量保障（并发/超时/连接池/背压/内存/优雅关闭）
//! =============================================================
//!  约束实现：
//!   1. 全局并发上限 Semaphore（gateway_api 层叠加 1 次，此处 DB/上游请求再叠加）
//!   2. 每节点独立 reqwest Client（连接池最大连接数 + 超时隔离 connect/read/stream_read）
//!   3. 请求体大小限制（gateway_api 层已加，此处为 DB 查询分页兜底）
//!   4. 客户端断开 → abort 上游 reqwest::Response stream（abort_handle）
//!   5. 缓存池/样本缓冲/日志缓冲：全部 max_entries 硬上限
//!   6. 背压：队列不超过上限，超了直接 429
//!   7. 优雅关闭：SIGINT/SIGTERM + 窗口关闭触发
//! =============================================================
pub mod connection_pool;       // 按 node_group 命名空间隔离 reqwest::Client
pub mod semaphores;            // 全局 + per_client_key 并发令牌桶
pub mod rate_limits;           // 全局 / 每节点限流组骨架
pub mod timeouts;              // 超时矩阵配置（连接/读取/流式）
pub mod backpressure;          // 429 快速失败
pub mod shutdown;              // 信号监听 + 优雅关闭

use crate::error::{AppError, AppResult};
use crate::state::{AppState, GatewayCtrl};
use std::sync::Arc;

/// 启动 axum 服务，shutdown 发送端登记到 GatewayCtrl，供面板一键 stop / restart
/// 不再返回值；停止时通过 ctrl.request_stop() 触发
pub async fn spawn_axum_server(state: Arc<AppState>, ctrl: Arc<GatewayCtrl>) -> AppResult<()> {
    // 若已在运行，直接忽略
    if ctrl.is_running() {
        return Err(AppError::Business("网关已在运行".into()));
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let c = ctrl.clone();
    let h = tokio::spawn(async move {
        if let Err(e) = crate::gateway_api::serve_forever(state, rx).await {
            tracing::error!("axum server fatal: {e:?}");
            c.running.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    });
    ctrl.register(tx);
    let _ = h; // 避免 "unused"
    Ok(())
}

/// 纯网关服务器模式下的终止信号等待（SIGINT / SIGTERM / Ctrl-C）
pub async fn wait_for_termination() {
    #[cfg(unix)]
    {
        let mut int = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).expect("SIGINT");
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM");
        tokio::select! {
            _ = int.recv() => tracing::warn!("SIGINT received"),
            _ = term.recv() => tracing::warn!("SIGTERM received"),
        }
    }
    #[cfg(not(unix))]
    { tokio::signal::ctrl_c().await.expect("Ctrl-C"); tracing::warn!("Ctrl-C received"); }
}
