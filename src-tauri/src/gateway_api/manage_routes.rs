//! =============================================================
//!  交付物 7：/manage/... 内部管理接口（axum handler 签名）
//!  · 账单：GET    /manage/billing/summary?client_key_hash=&window=24h|7d|30d
//!          GET    /manage/billing/rates
//!          GET    /manage/billing/client-keys
//!  · 追踪：GET    /manage/traces?model=&status=&limit=100&cursor=
//!          GET    /manage/traces/:trace_id         (含所有 SubAttempt)
//!          GET    /manage/sub_attempts/:sub_id     (单条明细)
//!  · 节点：GET    /manage/nodes                     (全部节点 + 质量分 + 临时 ban)
//!          POST   /manage/nodes/:node_id/reset     (手动清 0 质量 + 取消临时 ban)
//!  · 对账：GET    /manage/audit?alarm_only=1&limit=100
//!          GET    /manage/audit/discrepancy?model=&from_ms=&to_ms=
//!  · 缓存：POST   /manage/cache/purge?model=
//!          GET    /manage/cache/stats
//! =============================================================
use crate::error::AppError;
use crate::gateway_api::auth::AuthedClient;
use crate::gateway_api::error_resp::AppErrorResponse;
use crate::state::AppState;
use std::sync::Arc;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;

#[cfg(feature = "desktop-shell")]
use tauri::Manager;

/// 本机管理 token：菜单页通过 HTTP 控制网关启停时校验（仅本机操作，无对外安全问题）。
const MANAGE_TOKEN: &str = "pls-local-manage";

pub fn router(_app: Arc<AppState>) -> axum::Router<Arc<AppState>> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/billing/summary",     get(billing_summary_handler))
        .route("/billing/rates",       get(billing_rates_handler))
        .route("/billing/client-keys", get(client_keys_handler))
        .route("/traces",              get(traces_list_handler))
        .route("/traces/:trace_id",    get(trace_detail_handler))
        .route("/sub_attempts/:sub_id",get(sub_attempt_detail_handler))
        .route("/nodes",               get(nodes_list_handler))
        .route("/nodes/:node_id/reset",post(node_reset_handler))
        .route("/audit",               get(audit_list_handler))
        .route("/audit/discrepancy",   get(audit_discrepancy_handler))
        .route("/cache/purge",         post(cache_purge_handler))
        .route("/cache/stats",         get(cache_stats_handler))
        // 本机 HTTP 启停（菜单页用 token 控制，仅本机可操作）
        .route("/lifecycle",           get(lifecycle_status_handler).post(lifecycle_control_handler))
}

// ---------------- common params ----------------
#[derive(Debug, Deserialize)]
pub struct Window { #[serde(default)] pub window: String }
#[derive(Debug, Deserialize)]
pub struct ListQuery { #[serde(default)] pub model: String, #[serde(default)] pub status: String, #[serde(default)] pub limit: usize, #[serde(default)] pub cursor: String }
#[derive(Debug, Deserialize)]
pub struct AuditQuery { #[serde(default)] pub alarm_only: bool, #[serde(default)] pub limit: usize }
#[derive(Debug, Deserialize)]
pub struct AuditDiscQuery { #[serde(default)] pub model: String, #[serde(default)] pub from_ms: u128, #[serde(default)] pub to_ms: u128 }
#[derive(Debug, Deserialize)]
pub struct PurgeQuery { #[serde(default)] pub model: String }

// ---------------- handlers (stubs) ----------------

/// 计费账本汇总（mock response，真实走 DB）
pub async fn billing_summary_handler(
    State(_app): State<Arc<AppState>>, Query(_w): Query<Window>,
    _client: AuthedClient,
) -> Result<Response, AppErrorResponse> {
    Ok(Json(serde_json::json!({
        "client_key_hash": "", "window_label": _w.window,
        "requests_total": 0, "requests_ok": 0,
        "tokens_input": 0, "tokens_output": 0, "total_charge_cny": 0.0,
    })).into_response())
}

/// 费率列表（直接读 AppConfig.billing.rates）
pub async fn billing_rates_handler(
    State(app): State<Arc<AppState>>, _client: AuthedClient,
) -> Result<Response, AppErrorResponse> {
    let rates = &app.cfg_swap.load().billing.rates;
    Ok(Json(rates).into_response())
}

/// 网关自有 API Key 列表（**只返回名称/ID/配额，绝不返回 key 明文**）
pub async fn client_keys_handler(
    State(app): State<Arc<AppState>>, _client: AuthedClient,
) -> Result<Response, AppErrorResponse> {
    use crate::observability::masking::mask_token;
    let cfg = app.cfg_swap.load();
    let mask_cfg = &cfg.masking;
    let masked: Vec<serde_json::Value> = cfg.billing.client_keys.iter().map(|c| serde_json::json!({
        "name": c.name, "group": c.group,
        "key_masked": mask_token(&c.key, mask_cfg),
        "rpm": c.rpm, "tpm": c.tpm, "concurrency": c.concurrency,
        "balance_cny": c.balance_cny,
        "daily_hard_quota_tokens": c.daily_hard_quota_tokens,
        "total_hard_quota_tokens": c.total_hard_quota_tokens,
        "allow_overdraft": c.allow_overdraft, "enabled": c.enabled,
        "rate_plan": c.rate_plan,
    })).collect();
    Ok(Json(masked).into_response())
}

pub async fn traces_list_handler(
    State(_app): State<Arc<AppState>>, Query(_q): Query<ListQuery>,
    _client: AuthedClient,
) -> Result<Response, AppErrorResponse> { Ok(Json(Vec::<serde_json::Value>::new()).into_response()) }

pub async fn trace_detail_handler(
    State(_app): State<Arc<AppState>>, Path(_id): Path<String>,
    _client: AuthedClient,
) -> Result<Response, AppErrorResponse> { Ok(Json(serde_json::json!(null)).into_response()) }

pub async fn sub_attempt_detail_handler(
    State(_app): State<Arc<AppState>>, Path(_id): Path<String>,
    _client: AuthedClient,
) -> Result<Response, AppErrorResponse> { Ok(Json(serde_json::json!(null)).into_response()) }

/// 节点全景：全部 node_groups 展开成列表 + 质量分/临时ban状态
pub async fn nodes_list_handler(
    State(app): State<Arc<AppState>>, _client: AuthedClient,
) -> Result<Response, AppErrorResponse> {
    let cfg = app.cfg_swap.load();
    let now_ms = crate::router::group_selector::now_ms();
    let mut out = Vec::new();
    for g in &cfg.node_groups {
        for n in &g.nodes {
            let q = crate::node_quality::quality_of(&app, &n.id);
            let ban_until = app.node_runtime.temp_ban_until.get(&n.id).map(|x| *x as u128);
            let banned = ban_until.map(|t| t > now_ms).unwrap_or(false);
            out.push(serde_json::json!({
                "node_id": n.id, "group_id": g.id,
                "group_enabled": g.enabled, "enabled": n.enabled,
                "endpoint_masked": crate::observability::masking::mask_endpoint(&n.endpoint, &cfg.masking),
                "protocol_hints": n.protocol_hints, "weight": n.weight,
                "hard_disable": n.hard_disable,
                "primary": n.primary, "quality_score": q,
                "temp_ban_until_ms": ban_until, "temp_banned_now": banned,
            }));
        }
    }
    Ok(Json(out).into_response())
}

pub async fn node_reset_handler(
    State(app): State<Arc<AppState>>, Path(node_id): Path<String>,
    _client: AuthedClient,
) -> Result<Response, AppErrorResponse> {
    app.node_runtime.temp_ban_until.remove(&node_id);
    // TODO: quality sample_buffer 重置
    Ok(Json(serde_json::json!({"ok": true, "node_id": node_id})).into_response())
}

pub async fn audit_list_handler(
    State(_app): State<Arc<AppState>>, Query(_q): Query<AuditQuery>,
    _client: AuthedClient,
) -> Result<Response, AppErrorResponse> { Ok(Json(Vec::<serde_json::Value>::new()).into_response()) }

pub async fn audit_discrepancy_handler(
    State(_app): State<Arc<AppState>>, Query(_q): Query<AuditDiscQuery>,
    _client: AuthedClient,
) -> Result<Response, AppErrorResponse> {
    Ok(Json(serde_json::json!({
        "avg_prompt_discrepancy_pct": 0.0, "avg_completion_discrepancy_pct": 0.0,
        "alarm_count": 0, "records_total": 0,
    })).into_response())
}

pub async fn cache_purge_handler(
    State(_app): State<Arc<AppState>>, Query(_q): Query<PurgeQuery>,
    _client: AuthedClient,
) -> Result<Response, AppErrorResponse> { Ok(Json(serde_json::json!({"purged": true})).into_response()) }

pub async fn cache_stats_handler(
    State(app): State<Arc<AppState>>, _client: AuthedClient,
) -> Result<Response, AppErrorResponse> {
    if let Some(backend) = &app.cache_backend {
        let (ns, ss, mem) = backend.stats().await;
        Ok(Json(serde_json::json!({
            "non_stream_entries": ns, "stream_entries": ss, "estimated_mem_bytes": mem,
        })).into_response())
    } else {
        Ok(Json(serde_json::json!({
            "non_stream_entries": 0, "stream_entries": 0, "estimated_mem_bytes": 0,
            "note": "cache disabled in AppState.bootstrap",
        })).into_response())
    }
}

// ---------------- 本机 HTTP 启停控制（托盘菜单页直连网关，校验本机 token） ----------------
#[derive(Debug, Deserialize)]
pub struct LifecycleReq { pub action: String }

fn check_manage_token(headers: &HeaderMap) -> Result<(), AppErrorResponse> {
    let tok = headers.get("x-manage-token").and_then(|v| v.to_str().ok()).unwrap_or("");
    if tok == MANAGE_TOKEN { Ok(()) }
    else { Err(AppErrorResponse(AppError::Business("本机管理 token 无效".into()))) }
}

pub async fn lifecycle_status_handler(
    State(app): State<Arc<AppState>>, headers: HeaderMap,
) -> Result<Response, AppErrorResponse> {
    check_manage_token(&headers)?;
    let listen = app.cfg_swap.load().http.listen.clone();
    Ok(Json(serde_json::json!({
        "running": app.gateway_ctrl.is_running(),
        "listen": listen,
    })).into_response())
}

pub async fn lifecycle_control_handler(
    State(app): State<Arc<AppState>>, headers: HeaderMap, Json(body): Json<LifecycleReq>,
) -> Result<Response, AppErrorResponse> {
    check_manage_token(&headers)?;

    let mut handled = false;
    match body.action.as_str() {
        "stop" => { app.gateway_ctrl.request_stop(); handled = true; }
        "start" => {
            let st = app.clone();
            tokio::spawn(async move {
                let _ = crate::safety_runtime::spawn_axum_server(st.clone(), st.gateway_ctrl.clone()).await;
            });
            handled = true;
        }
        #[cfg(feature = "desktop-shell")]
        "show" => {
            if let Some(h) = app.app_handle.lock().ok().and_then(|g| g.clone()) {
                if let Some(w) = h.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
            handled = true;
        }
        #[cfg(feature = "desktop-shell")]
        "feedback" => {
            if let Some(_h) = app.app_handle.lock().ok().and_then(|g| g.clone()) {
                crate::open_url("https://yxpil.com/feedback");
            }
            handled = true;
        }
        #[cfg(feature = "desktop-shell")]
        "quit" => {
            let _ = app.gateway_ctrl.request_stop();
            if let Some(h) = app.app_handle.lock().ok().and_then(|g| g.clone()) {
                h.exit(0);
            }
            handled = true;
        }
        #[cfg(feature = "desktop-shell")]
        "hide" => { handled = true; }
        _ => {}
    }
    if !handled {
        return Err(AppErrorResponse(AppError::Business("未知生命周期操作".into())));
    }

    // 除退出外，操作完成后收起托盘菜单窗口
    #[cfg(feature = "desktop-shell")]
    if body.action.as_str() != "quit" {
        if let Some(h) = app.app_handle.lock().ok().and_then(|g| g.clone()) {
            if let Some(w) = h.get_webview_window("tray-popup") {
                let _ = w.hide();
            }
        }
    }

    Ok(Json(serde_json::json!({"ok": true})).into_response())
}
