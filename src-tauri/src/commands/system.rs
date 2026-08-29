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
            let mut r = client.get(models_probe_url(&endpoint));
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

/// OpenAI 兼容探针 URL / 节点模型拉取已移至 router 模块（启动刷新共用）
use crate::router::models_probe_url;

/// 自动收集所有上游节点的真实模型（去重），供聊天页下拉；同时重建路由模型目录。
/// 返回条目：{id: "host|model" 或 "model", model, host, group}
#[tauri::command]
pub async fn list_upstream_models(state: State<'_, Arc<AppState>>) -> CommandResult<ApiResponse<Vec<serde_json::Value>>> {
    let app = state.inner().clone();
    let mut out = crate::router::refresh_model_catalog(&app).await;
    // AUTOMODE 虚拟模型置顶（设置开启时）：聊天页/下游都可见
    if app.cfg_swap.load().automode.enabled {
        out.insert(0, serde_json::json!({ "id": "AUTOMODE", "model": "AUTOMODE", "host": "", "group": "" }));
    }
    Ok(ApiResponse::ok(out))
}

/// 聊天（SSE 流式）：后端回环请求网关 stream:true，逐 chunk 通过 Channel 推给前端。
/// 事件：{type:"chunk", text, reasoning} / {type:"done"} / {type:"error", message}
#[tauri::command]
pub async fn gateway_chat_stream(
    state: State<'_, Arc<AppState>>,
    model: String,
    messages: Vec<serde_json::Value>,
    key: Option<String>,
    on_event: tauri::ipc::Channel<serde_json::Value>,
) -> CommandResult<ApiResponse<()>> {
    let app = state.inner().clone();
    let cfg = app.cfg_swap.load();
    let listen = cfg.http.listen.clone();
    let key = key.unwrap_or_else(|| cfg.billing.client_keys.first().map(|c| c.key.clone()).unwrap_or_default());
    let url = format!("http://{listen}/v1/chat/completions");
    let body = serde_json::json!({ "model": model, "messages": messages, "stream": true });
    // 回环请求必须禁用代理，避免被系统 HTTP_PROXY/HTTPS_PROXY 劫持导致连接失败
    let client = reqwest::Client::builder().no_proxy().build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let resp = client.post(&url)
        .header("Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send().await;
    match resp {
        Ok(r) if r.status().is_success() => {
            use futures::StreamExt;
            let mut stream = r.bytes_stream();
            let mut buf = String::new();
            while let Some(item) = stream.next().await {
                match item {
                    Ok(chunk) => {
                        buf.push_str(&String::from_utf8_lossy(&chunk));
                        // 按空行切分 SSE 事件（跨 chunk 分帧安全）
                        while let Some(pos) = buf.find("\n\n") {
                            let event: String = buf.drain(..pos + 2).collect();
                            for line in event.lines() {
                                let Some(data) = line.strip_prefix("data: ") else { continue };
                                if data.trim() == "[DONE]" {
                                    let _ = on_event.send(serde_json::json!({ "type": "done" }));
                                    continue;
                                }
                                // 流内错误事件（上游超时/断流等）：data: [ERROR] <msg>
                                if let Some(err) = data.strip_prefix("[ERROR] ") {
                                    let _ = on_event.send(serde_json::json!({ "type": "error", "message": err }));
                                    continue;
                                }
                                let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else { continue };
                                let text = v.pointer("/choices/0/delta/content").and_then(|c| c.as_str()).unwrap_or("").to_string();
                                let reasoning = v.pointer("/choices/0/delta/reasoning_content").and_then(|c| c.as_str()).unwrap_or("").to_string();
                                if !text.is_empty() || !reasoning.is_empty() {
                                    let _ = on_event.send(serde_json::json!({ "type": "chunk", "text": text, "reasoning": reasoning }));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = on_event.send(serde_json::json!({ "type": "error", "message": e.to_string() }));
                    }
                }
            }
            let _ = on_event.send(serde_json::json!({ "type": "done" }));
            Ok(ApiResponse::ok(()))
        }
        Ok(r) => {
            let status = r.status();
            let text = r.text().await.unwrap_or_default();
            let msg = format!("网关返回 {status}: {text}");
            let _ = on_event.send(serde_json::json!({ "type": "error", "message": msg }));
            Ok(ApiResponse::fail(msg))
        }
        Err(e) => {
            let msg = format!("网关请求失败: {e}");
            let _ = on_event.send(serde_json::json!({ "type": "error", "message": msg }));
            Ok(ApiResponse::fail(msg))
        }
    }
}
