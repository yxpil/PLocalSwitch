//! system 模块命令：应用信息、心跳、系统信息
#![cfg(feature = "desktop-shell")]

use crate::error::CommandResult;
use crate::models::{ApiResponse, SystemInfo};
use crate::state::AppState;
use std::sync::Arc;
use tauri::State;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub identifier: &'static str,
}

/// 获取应用基础信息
#[tauri::command]
pub fn get_app_info(state: State<'_, Arc<AppState>>) -> CommandResult<ApiResponse<AppInfo>> {
    let _ = state.bump_request();
    Ok(ApiResponse::ok(AppInfo {
        name: "PLocalSwitch",
        version: env!("CARGO_PKG_VERSION"),
        identifier: "com.plocalswitch.app",
    }))
}

/// 简单心跳命令
#[tauri::command]
pub fn ping(state: State<'_, Arc<AppState>>, msg: Option<String>) -> CommandResult<ApiResponse<String>> {
    let count = state.bump_request();
    let reply = format!("pong #{} - {}", count, msg.unwrap_or_else(|| "hello".into()));
    tracing::debug!("ping 响应: {}", reply);
    Ok(ApiResponse::ok(reply))
}

/// 获取系统运行信息
#[tauri::command]
pub fn get_system_info(state: State<'_, Arc<AppState>>) -> CommandResult<ApiResponse<SystemInfo>> {
    let count = state.bump_request();
    Ok(ApiResponse::ok(SystemInfo {
        app_version: env!("CARGO_PKG_VERSION").into(),
        rust_version: env!("CARGO_PKG_RUST_VERSION").into(),
        os: format!("{} {}", std::env::consts::OS, std::env::consts::FAMILY),
        arch: std::env::consts::ARCH.into(),
        uptime: state.uptime_seconds(),
        request_count: count,
    }))
}

/// 最近转发记录（读取 DB 中的真实 trace）
#[tauri::command]
pub async fn list_traces(state: State<'_, Arc<AppState>>) -> CommandResult<ApiResponse<Vec<serde_json::Value>>> {
    let _ = state.bump_request();
    let app = state.inner().clone();
    let traces = crate::services::trace_store::recent_traces(&app, 50).await;
    Ok(ApiResponse::ok(traces))
}

/// 账本汇总（读取 DB 中的客户端计费账本）
#[tauri::command]
pub async fn billing_summary(state: State<'_, Arc<AppState>>, window: Option<String>) -> CommandResult<ApiResponse<serde_json::Value>> {
    let _ = state.bump_request();
    let app = state.inner().clone();
    let w = window.unwrap_or_else(|| "24h".into());
    let s = crate::services::trace_store::billing_summary(&app, &w).await;
    Ok(ApiResponse::ok(s))
}

/// 聊天：由 Rust 后端内部请求网关（绕过 WebView 的 CORS/PNA 限制），返回助手回复
#[tauri::command]
pub async fn gateway_chat(
    state: State<'_, Arc<AppState>>,
    model: String,
    messages: Vec<serde_json::Value>,
    key: Option<String>,
) -> CommandResult<ApiResponse<serde_json::Value>> {
    let app = state.inner().clone();
    let cfg = app.cfg_swap.load();
    let listen = cfg.http.listen.clone();
    let key = key.unwrap_or_else(|| cfg.billing.client_keys.first().map(|c| c.key.clone()).unwrap_or_default());
    let url = format!("http://{listen}/v1/chat/completions");
    let body = serde_json::json!({ "model": model, "messages": messages, "stream": false });
    // 回环请求必须禁用代理，避免被系统 HTTP_PROXY/HTTPS_PROXY 劫持导致连接失败
    let client = reqwest::Client::builder().no_proxy().build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let resp = client.post(&url)
        .header("Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send().await;
    match resp {
        Ok(r) => {
            let status = r.status();
            let text = r.text().await.unwrap_or_default();
            if status.is_success() {
                let v: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
                let content = v.pointer("/choices/0/message/content").and_then(|c| c.as_str()).unwrap_or("").to_string();
                Ok(ApiResponse::ok(serde_json::json!({ "content": content, "raw": v })))
            } else {
                Ok(ApiResponse::fail(format!("网关返回 {status}: {text}")))
            }
        }
        Err(e) => Ok(ApiResponse::fail(format!("网关请求失败: {e}"))),
    }
}

/// 测试单个上游节点连通性/鉴权（保存前用）。返回 {ok, status, message, bodies}
#[tauri::command]
pub async fn test_node(endpoint: String, api_key: String, protocol: String) -> CommandResult<ApiResponse<serde_json::Value>> {
    let endpoint = endpoint.trim().trim_end_matches('/').to_string();
    if endpoint.is_empty() {
        return Ok(ApiResponse::fail("endpoint 为空"));
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let proto = protocol.parse::<crate::router::ProtocolKind>().unwrap_or(crate::router::ProtocolKind::OpenAI);

    let send = match proto {
        crate::router::ProtocolKind::Ollama => client.get(format!("{endpoint}/api/tags")).send().await,
        crate::router::ProtocolKind::Anthropic => {
            client.post(format!("{endpoint}/v1/messages"))
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&serde_json::json!({"model": "probe", "max_tokens": 1, "messages": [{"role": "user", "content": "hi"}]}))
                .send().await
        }
        crate::router::ProtocolKind::Gemini => {
            let mut r = client.get(format!("{endpoint}/v1beta/models"));
            if !api_key.is_empty() { r = r.header("x-goog-api-key", &api_key); }
            r.send().await
        }
        _ => {
            let mut r = client.get(format!("{endpoint}/v1/models"));
            if !api_key.is_empty() { r = r.header("Authorization", format!("Bearer {api_key}")); }
            r.send().await
        }
    };

    match send {
        Ok(r) => {
            let status = r.status().as_u16();
            let body = r.text().await.unwrap_or_default();
            // 200/400/404 都说明端点可达且鉴权格式被接受；401/403=key 无效；5xx=服务端问题
            let ok = status < 500 && status != 401 && status != 403;
            let reason = match status {
                200 => "✓ 连通正常（鉴权通过）",
                401 => "鉴权失败：API key 无效或未授权",
                403 => "鉴权失败：无权限（403）",
                400 | 404 => "端点可达、鉴权已接受（400/404 可能模型名/路径问题，可继续测试）",
                500..=599 => "上游服务端错误",
                _ => "端点上未预期的响应",
            };
            Ok(ApiResponse::ok(serde_json::json!({
                "ok": ok, "status": status, "message": format!("HTTP {status} {reason}"),
                "bodies": body.chars().take(300).collect::<String>(),
            })))
        }
        Err(e) => Ok(ApiResponse::fail(format!("连接失败：{e}"))),
    }
}

/// 从单个上游节点拉取真实模型列表：Ollama /api/tags、其余 OpenAI 兼容 /v1/models
async fn fetch_node_models(client: &reqwest::Client, endpoint: &str, key: &str, proto: &str) -> Vec<String> {
    let url = if proto == "ollama" { format!("{endpoint}/api/tags") } else { format!("{endpoint}/v1/models") };
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

/// 自动收集所有上游节点的真实模型（去重），供聊天页下拉；并发拉取避免慢
#[tauri::command]
pub async fn list_upstream_models(state: State<'_, Arc<AppState>>) -> CommandResult<ApiResponse<Vec<serde_json::Value>>> {
    let app = state.inner().clone();
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
            let c = client.clone();
            tasks.push(async move { (group, fetch_node_models(&c, &endpoint, &key, &proto).await) });
        }
    }
    // 重建“模型→上游组”目录：每个模型绑定到真正服务它的组，供路由按模型匹配 API
    app.node_runtime.model_catalog.clear();
    let results = futures::future::join_all(tasks).await;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<serde_json::Value> = Vec::new();
    // 1) 上游真实模型（来自 /v1/models 或 /api/tags）
    for (group, ids) in results {
        for id in ids {
            seen.insert(id.clone());
            app.node_runtime.model_catalog.insert(id.clone(), group.clone());
            out.push(serde_json::json!({ "id": id, "group": group.clone() }));
        }
    }
    // 2) 别名真实模型（即使上游 /v1/models 不返回别名，如 deepseek-chat，也能路由）
    for a in &cfg.model_aliases {
        if a.enabled {
            app.node_runtime.model_catalog.entry(a.real_model.clone()).or_insert_with(|| a.group.clone());
            if seen.insert(a.real_model.clone()) {
                out.push(serde_json::json!({ "id": a.real_model.clone(), "group": a.group.clone() }));
            }
        }
    }
    Ok(ApiResponse::ok(out))
}
