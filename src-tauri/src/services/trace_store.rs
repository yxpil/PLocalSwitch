//! 链路追踪 + 双账本 持久化（SQLite）
//!  - traces        ：每次 GatewayTrace 最终结果一行（链路追踪页数据源）
//!  - client_ledger ：客户端计费账本（账本汇总页数据源：请求数/成功数/tokens/总费用）
//!
//! 说明：本模块是被转发引擎真正写库的入口。此前 billing_summary / list_traces 均返回 0，
//!       是因为从未调用这些写入函数。接入后账本与对账、链路追踪即可看到真实转发记录。
use crate::billing::ledger::ClientLedgerEntry;
use crate::config::BillingConfig;
use crate::observability::trace::GatewayTrace;
use crate::state::{AppState, DbPool};
use serde_json::json;
use sqlx::Row;

fn window_seconds(window: &str) -> i64 {
    match window { "30d" => 2592000, "7d" => 604800, _ => 86400 }
}

/// 启动时建表（幂等）
pub async fn init_schema(app: &AppState) {
    if let DbPool::Sqlite(p) = &app.db {
        for sql in [
            r#"CREATE TABLE IF NOT EXISTS traces (
                 trace_id TEXT PRIMARY KEY,
                 received_at_ms INTEGER, finished_at_ms INTEGER,
                 client_key_hash TEXT, client_key_name TEXT,
                 model_alias TEXT, resolved_model TEXT, node_group TEXT,
                 is_stream INTEGER, is_cached INTEGER,
                 final_status_code INTEGER, final_error_label TEXT,
                 billed_prompt INTEGER, billed_completion INTEGER, billed_total INTEGER,
                 upstream_prompt INTEGER, upstream_completion INTEGER, upstream_total INTEGER,
                 total_latency_ms INTEGER, human_reason TEXT,
                 created_at INTEGER)"#,
            r#"CREATE TABLE IF NOT EXISTS client_ledger (
                 id TEXT PRIMARY KEY, trace_id TEXT,
                 client_key_hash TEXT, client_key_name TEXT, model TEXT,
                 is_stream INTEGER, is_cached INTEGER, usage_source TEXT,
                 prompt_tokens INTEGER, completion_tokens INTEGER, total_tokens INTEGER,
                 price_input_cny REAL, price_output_cny REAL, price_total_cny REAL,
                 discount_rate REAL, final_charge_cny REAL,
                 created_at_ms INTEGER)"#,
        ] {
            let _ = sqlx::query(sql).execute(p).await;
        }
    } else {
        tracing::warn!("trace_store: 非 SQLite 后端暂不落库");
    }
}

/// 依据计费费率计算客户端费用
fn build_client_entry(cfg: &BillingConfig, trace: &GatewayTrace) -> ClientLedgerEntry {
    let model = trace.resolved_model.as_str();
    let rate = cfg.rates.iter().find(|r| r.model == model);
    let (pi, po) = rate.map(|r| (r.client_price_per_m_input, r.client_price_per_m_output)).unwrap_or((0.0, 0.0));
    let u = &trace.billed_usage;
    let price_input = pi * (u.prompt_tokens as f64) / 1e6;
    let price_output = po * (u.completion_tokens as f64) / 1e6;
    let price_total = price_input + price_output;
    ClientLedgerEntry {
        id: crate::observability::trace::now_ms().to_string(),
        trace_id: trace.trace_id.clone(),
        client_key_hash: trace.client_key_hash.clone(),
        client_key_name: trace.client_key_name.clone(),
        model: model.to_string(),
        is_stream: trace.is_stream,
        is_cached_hit: trace.is_cached,
        usage_source: "C".into(),
        usage: u.clone(),
        price_input_cny: price_input,
        price_output_cny: price_output,
        price_total_cny: price_total,
        discount_rate: 1.0,
        final_charge_cny: price_total,
        created_at_ms: trace.received_at_ms,
    }
}

/// 写库：一个已完成的请求 → traces + client_ledger
pub async fn record(app: &AppState, trace: &GatewayTrace) {
    let DbPool::Sqlite(p) = &app.db else { return };
    let cfg = app.cfg_swap.load();
    let e = build_client_entry(&cfg.billing, trace);
    let now = crate::observability::trace::now_ms() as i64;
    // traces
    let _ = sqlx::query(
        r#"INSERT OR REPLACE INTO traces
           (trace_id, received_at_ms, finished_at_ms, client_key_hash, client_key_name,
            model_alias, resolved_model, node_group, is_stream, is_cached,
            final_status_code, final_error_label,
            billed_prompt, billed_completion, billed_total,
            upstream_prompt, upstream_completion, upstream_total,
            total_latency_ms, human_reason, created_at)
           VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"#)
        .bind(&trace.trace_id)
        .bind(trace.received_at_ms as i64)
        .bind(trace.finished_at_ms.map(|x| x as i64))
        .bind(&trace.client_key_hash)
        .bind(&trace.client_key_name)
        .bind(&trace.model_alias)
        .bind(&trace.resolved_model)
        .bind(&trace.node_group)
        .bind(trace.is_stream as i64)
        .bind(trace.is_cached as i64)
        .bind(trace.final_status_code as i64)
        .bind(&trace.final_error_label.map(|l| l.to_string()))
        .bind(trace.billed_usage.prompt_tokens as i64)
        .bind(trace.billed_usage.completion_tokens as i64)
        .bind(trace.billed_usage.total_tokens as i64)
        .bind(trace.upstream_usage_sum.prompt_tokens as i64)
        .bind(trace.upstream_usage_sum.completion_tokens as i64)
        .bind(trace.upstream_usage_sum.total_tokens as i64)
        .bind(trace.total_latency_ms.map(|x| x as i64))
        .bind(&trace.human_readable_reason)
        .bind(now)
        .execute(p).await;
    // client_ledger
    let _ = sqlx::query(
        r#"INSERT OR REPLACE INTO client_ledger
           (id, trace_id, client_key_hash, client_key_name, model, is_stream, is_cached, usage_source,
            prompt_tokens, completion_tokens, total_tokens,
            price_input_cny, price_output_cny, price_total_cny, discount_rate, final_charge_cny, created_at_ms)
           VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"#)
        .bind(&e.id).bind(&e.trace_id).bind(&e.client_key_hash).bind(&e.client_key_name)
        .bind(&e.model).bind(e.is_stream as i64).bind(e.is_cached_hit as i64).bind(&e.usage_source)
        .bind(e.usage.prompt_tokens as i64).bind(e.usage.completion_tokens as i64).bind(e.usage.total_tokens as i64)
        .bind(e.price_input_cny).bind(e.price_output_cny).bind(e.price_total_cny)
        .bind(e.discount_rate).bind(e.final_charge_cny).bind(e.created_at_ms as i64)
        .execute(p).await;
}

/// 账本汇总（客户端计费）
pub async fn billing_summary(app: &AppState, window: &str) -> serde_json::Value {
    let DbPool::Sqlite(p) = &app.db else {
        return json!({"window_label": window, "requests_total": 0, "requests_ok": 0, "tokens_input": 0, "tokens_output": 0, "total_charge_cny": 0.0});
    };
    let since = (crate::observability::trace::now_ms() as i64) - window_seconds(window) * 1000;
    let row = sqlx::query(
        r#"SELECT
             COUNT(*) AS total,
             COALESCE(SUM(CASE WHEN total_tokens >= 0 THEN 1 ELSE 0 END),0) AS ok,
             COALESCE(SUM(prompt_tokens),0) AS ti,
             COALESCE(SUM(completion_tokens),0) AS to_,
             COALESCE(SUM(final_charge_cny),0) AS charge
           FROM client_ledger WHERE created_at_ms >= ?"#)
        .bind(since).fetch_one(p).await;
    match row {
        Ok(r) => {
            let total: i64 = r.try_get("total").unwrap_or(0);
            let ok: i64 = r.try_get("ok").unwrap_or(0);
            let ti: i64 = r.try_get("ti").unwrap_or(0);
            let to: i64 = r.try_get("to_").unwrap_or(0);
            let charge: f64 = r.try_get("charge").unwrap_or(0.0);
            json!({
                "window_label": window, "requests_total": total.max(0) as u64,
                "requests_ok": ok.max(0) as u64,
                "tokens_input": ti.max(0) as u64, "tokens_output": to.max(0) as u64,
                "total_charge_cny": charge.max(0.0),
            })
        }
        Err(e) => { tracing::error!(error = %e, "billing_summary 查询失败"); json!({"window_label": window, "requests_total": 0, "requests_ok": 0, "tokens_input": 0, "tokens_output": 0, "total_charge_cny": 0.0}) }
    }
}

/// 最近转发记录（链路追踪页）
pub async fn recent_traces(app: &AppState, limit: i64) -> Vec<serde_json::Value> {
    let DbPool::Sqlite(p) = &app.db else { return vec![] };
    let rows = sqlx::query(
        r#"SELECT trace_id, model_alias, resolved_model, node_group, is_stream, is_cached,
                  final_status_code, billed_total, total_latency_ms, created_at
           FROM traces ORDER BY created_at DESC LIMIT ?"#)
        .bind(limit).fetch_all(p).await;
    match rows {
        Ok(rows) => rows.into_iter().map(|r| {
            json!({
                "trace_id": r.get::<String, _>("trace_id"),
                "model": r.get::<String, _>("model_alias"),
                "resolved_model": r.get::<String, _>("resolved_model"),
                "node_group": r.get::<String, _>("node_group"),
                "status": r.get::<i64, _>("final_status_code"),
                "tokens": r.get::<i64, _>("billed_total"),
                "latency": r.get::<Option<i64>, _>("total_latency_ms"),
                "created_at": r.get::<i64, _>("created_at"),
                "is_stream": r.get::<i64, _>("is_stream") == 1,
            })
        }).collect(),
        Err(e) => { tracing::error!(error = %e, "recent_traces 查询失败"); vec![] }
    }
}

