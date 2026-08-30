//! =============================================================
//!  commands::gateway —— Tauri 桌面壳通过 IPC 控制网关服务启停
//!  （桌面 UI → Rust → 网关运行时生命周期控制）
//! =============================================================
#![cfg(feature = "desktop-shell")]
use crate::error::{CommandResult, ErrorLabel};
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct GatewayStatus {
    pub running: bool,
    pub listen: String,
    pub uptime_seconds: i64,
    pub requests_total: u64,
    pub active_requests: u32,
    pub nodes_total: usize,
    pub nodes_fault: usize,
    pub auto_restart: bool,
    pub restarts: u64,
    pub error_counters: std::collections::BTreeMap<ErrorLabel, u64>,
}

/// 查询网关运行状态（供面板总控展示）
#[tauri::command]
pub async fn gateway_status(
    state: tauri::State<'_, std::sync::Arc<crate::state::AppState>>,
) -> CommandResult<ApiResponse<GatewayStatus>> {
    let _ = state.bump_request();
    let ctrl = &state.gateway_ctrl;
    let cfg = state.cfg_swap.load();
    let nodes_total = cfg.node_groups.iter().map(|g| g.nodes.len()).sum();
    Ok(ApiResponse::ok(GatewayStatus {
        running: ctrl.is_running(),
        listen: ctrl.listen.clone(),
        uptime_seconds: state.uptime_seconds(),
        requests_total: state.request_counter.load(std::sync::atomic::Ordering::Relaxed),
        active_requests: 0,
        nodes_total,
        nodes_fault: 0,
        auto_restart: ctrl.auto_restart_enabled(),
        restarts: ctrl.restart_count(),
        error_counters: Default::default(),
    }))
}

/// 启动网关服务（幂等：已在运行则忽略）
#[tauri::command]
pub async fn gateway_start(
    state: tauri::State<'_, std::sync::Arc<crate::state::AppState>>,
) -> CommandResult<ApiResponse<bool>> {
    let _ = state.bump_request();
    let app = (*state).clone();
    match crate::safety_runtime::spawn_axum_server(app.clone(), app.gateway_ctrl.clone()).await {
        Ok(_) => { tracing::info!("网关服务已启动"); Ok(ApiResponse::ok(true)) }
        Err(e) => {
            // 已在运行视为成功
            if app.gateway_ctrl.is_running() { Ok(ApiResponse::ok(true)) }
            else { Err(e) }
        }
    }
}

/// 停止网关服务（优雅关闭）
#[tauri::command]
pub async fn gateway_stop(
    state: tauri::State<'_, std::sync::Arc<crate::state::AppState>>,
) -> CommandResult<ApiResponse<bool>> {
    let _ = state.bump_request();
    let stopped = state.gateway_ctrl.request_stop();
    if stopped { tracing::info!("网关服务已优雅停止"); }
    Ok(ApiResponse::ok(stopped))
}

/// 优雅重启（stop → wait → start）
#[tauri::command]
pub async fn restart_graceful(
    state: tauri::State<'_, std::sync::Arc<crate::state::AppState>>,
) -> CommandResult<ApiResponse<bool>> {
    let _ = state.bump_request();
    let app = (*state).clone();
    let _ = app.gateway_ctrl.request_stop();
    // 等待端口释放
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    crate::safety_runtime::spawn_axum_server(app.clone(), app.gateway_ctrl.clone()).await
        .map(|_| ApiResponse::ok(true))
        .or_else(|e| {
            if app.gateway_ctrl.is_running() { Ok(ApiResponse::ok(true)) } else { Err(e) }
        })
}

/// 查询 / 切换网关崩溃自动重启开关（enable=None 表示仅查询）
#[tauri::command]
pub fn gateway_auto_restart(
    state: tauri::State<'_, std::sync::Arc<crate::state::AppState>>,
    enable: Option<bool>,
) -> CommandResult<ApiResponse<bool>> {
    let _ = state.bump_request();
    if let Some(on) = enable {
        state.gateway_ctrl.set_auto_restart(on);
        tracing::info!("网关自动重启已{}", if on { "开启" } else { "关闭" });
    }
    Ok(ApiResponse::ok(state.gateway_ctrl.auto_restart_enabled()))
}
