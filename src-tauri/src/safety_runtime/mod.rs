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

/// 启动 axum 服务（守护式）：shutdown 发送端登记到 GatewayCtrl，供面板一键 stop / restart。
/// 服务进程异常退出（端口占用/致命错误）时自动按指数退避重启（500ms 起，封顶 8s）；
/// 用户主动停止（request_stop）或关闭 auto_restart 开关时不重启；
/// 若期间有新实例接管（代数不符），旧守护任务静默退场。
pub async fn spawn_axum_server(state: Arc<AppState>, ctrl: Arc<GatewayCtrl>) -> AppResult<()> {
    // 若已在运行，直接忽略
    if ctrl.is_running() {
        return Err(AppError::Business("网关已在运行".into()));
    }
    // 第一代通道同步注册：保证本函数返回后 is_running() 立即为 true
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut my_gen = ctrl.register(tx);
    let c = ctrl.clone();
    tokio::spawn(async move {
        let mut rx = rx;
        let mut backoff_ms: u64 = 500;
        loop {
            let res = crate::gateway_api::serve_forever(state.clone(), rx).await;
            if let Err(e) = &res {
                tracing::error!("axum server fatal: {e:?}");
            }
            // 已被更新的运行实例接管（手动重启/再次启动）→ 静默退场，不碰 running
            if c.generation_id() != my_gen { return; }
            // 用户主动停止 → 保持停止
            if c.take_stop_requested() {
                c.running.store(false, std::sync::atomic::Ordering::Relaxed);
                tracing::info!("网关已停止（用户操作，不自动重启）");
                return;
            }
            // 自动重启开关关闭 → 保持停止
            if !c.auto_restart_enabled() {
                c.running.store(false, std::sync::atomic::Ordering::Relaxed);
                return;
            }
            // 异常退出 → 指数退避自动拉起
            c.running.store(false, std::sync::atomic::Ordering::Relaxed);
            let n = c.restarts.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            tracing::warn!("网关异常退出，自动重启第 {n} 次（{backoff_ms}ms 后重试）");
            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms * 2).min(8000);
            // 退避等待期间可能有新实例启动或用户要求停止 → 让位
            if c.generation_id() != my_gen || c.take_stop_requested() {
                c.running.store(false, std::sync::atomic::Ordering::Relaxed);
                return;
            }
            let (ntx, nrx) = tokio::sync::oneshot::channel();
            my_gen = c.register(ntx);
            rx = nrx;
        }
    });
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
