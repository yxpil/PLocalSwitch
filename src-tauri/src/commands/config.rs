//! config 模块命令：加载 / 保存 / 重置网关配置（gateway.yaml）
#![cfg(feature = "desktop-shell")]

use crate::billing::client_key_mgr::ClientKeyRegistry;
use crate::config::AppConfig;
use crate::error::CommandResult;
use crate::models::ApiResponse;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

/// 代理配置（设置页「网络 → 代理配置」）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySetting {
    pub enable: bool,
    pub http:   String,
    pub socks:  String,
    pub bypass: String,
}

fn join_bypass(v: &[String]) -> String { v.join(",") }
fn split_bypass(s: &str) -> Vec<String> {
    s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

/// 读取当前配置（从 ArcSwap 内存读）
#[tauri::command]
pub fn load_config(state: State<'_, Arc<AppState>>) -> CommandResult<ApiResponse<AppConfig>> {
    let _ = state.bump_request();
    let cfg = (*state.cfg_swap.load_full()).clone();
    Ok(ApiResponse::ok(cfg))
}

/// 保存配置（写回内存 + 磁盘 gateway.yaml）
#[tauri::command]
pub async fn save_config(state: State<'_, Arc<AppState>>, cfg: AppConfig) -> CommandResult<ApiResponse<AppConfig>> {
    let _ = state.bump_request();
    let mut cfg = cfg;
    // 保护：前端可能未携带/清空 model_aliases 或 node_groups，空数组不覆盖已有非空配置，
    // 避免一次界面保存就把模型别名和上游节点组清空。
    let cur = (*state.cfg_swap.load_full()).clone();
    if cfg.model_aliases.is_empty() && !cur.model_aliases.is_empty() {
        cfg.model_aliases = cur.model_aliases;
    }
    if cfg.node_groups.is_empty() && !cur.node_groups.is_empty() {
        cfg.node_groups = cur.node_groups;
    }
    let cfg = crate::config::save_to_disk(&cfg)?;
    state.cfg_swap.store(std::sync::Arc::new(cfg.clone()));
    // 热更新鉴权注册表：新增/修改的 client key 立即生效，无需重启网关
    let reg = ClientKeyRegistry::from_cfg(&cfg.billing.client_keys);
    *state.client_keys.write().await = reg;
    tracing::info!("网关配置已保存并热更新");
    Ok(ApiResponse::ok(cfg))
}

/// 重置配置：重新从 bundled gateway.yaml 加载
#[tauri::command]
pub async fn reset_config(state: State<'_, Arc<AppState>>) -> CommandResult<ApiResponse<AppConfig>> {
    let _ = state.bump_request();
    let cfg = crate::config::reset_to_default()?;
    state.cfg_swap.store(std::sync::Arc::new(cfg.clone()));
    let reg = ClientKeyRegistry::from_cfg(&cfg.billing.client_keys);
    *state.client_keys.write().await = reg;
    tracing::info!("网关配置已重置为默认");
    Ok(ApiResponse::ok(cfg))
}

/// 读取代理设置（设置页「网络」）
#[tauri::command]
pub fn get_proxy_settings(state: State<'_, Arc<AppState>>) -> CommandResult<ApiResponse<ProxySetting>> {
    let _ = state.bump_request();
    let cfg = (*state.cfg_swap.load_full()).clone();
    Ok(ApiResponse::ok(ProxySetting {
        enable: cfg.http.proxy_enabled,
        http:   cfg.http.proxy.clone().unwrap_or_default(),
        socks:  cfg.http.proxy_socks.clone().unwrap_or_default(),
        bypass: join_bypass(&cfg.http.proxy_no_proxy),
    }))
}

/// 保存代理设置：写回配置内存 + 磁盘 gateway.yaml，并实时应用到上游 http client。
#[tauri::command]
pub fn set_proxy_settings(state: State<'_, Arc<AppState>>, setting: ProxySetting) -> CommandResult<ApiResponse<ProxySetting>> {
    let _ = state.bump_request();
    let mut cfg = (*state.cfg_swap.load_full()).clone();
    cfg.http.proxy_enabled = setting.enable;
    cfg.http.proxy = if setting.http.trim().is_empty() { None } else { Some(setting.http.trim().to_string()) };
    cfg.http.proxy_socks = if setting.socks.trim().is_empty() { None } else { Some(setting.socks.trim().to_string()) };
    cfg.http.proxy_no_proxy = split_bypass(&setting.bypass);

    let cfg = crate::config::save_to_disk(&cfg)?;
    state.cfg_swap.store(std::sync::Arc::new(cfg.clone()));
    // 实时生效：重建共享上游 http client（走新代理）
    crate::backend_adapters::apply_upstream_proxy(
        cfg.http.proxy_enabled,
        cfg.http.proxy.clone(),
        cfg.http.proxy_socks.clone(),
        cfg.http.proxy_no_proxy.clone(),
    );
    tracing::info!("代理配置已保存并实时应用 (enable={}, http={:?})", cfg.http.proxy_enabled, cfg.http.proxy);
    Ok(ApiResponse::ok(ProxySetting {
        enable: cfg.http.proxy_enabled,
        http:   cfg.http.proxy.clone().unwrap_or_default(),
        socks:  cfg.http.proxy_socks.clone().unwrap_or_default(),
        bypass: join_bypass(&cfg.http.proxy_no_proxy),
    }))
}
