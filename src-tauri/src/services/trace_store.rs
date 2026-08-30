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
                 model_alias TEXT, resolved_model TEXT, node_group TEXT, served_host TEXT,
                 is_stream INTEGER, is_cached INTEGER,
                 final_status_code INTEGER, final_error_label TEXT,
                 billed_prompt INTEGER, billed_completion INTEGER, billed_total INTEGER,
                 upstream_prompt INTEGER, upstream_completion INTEGER, upstream_total INTEGER,
                 total_latency_ms INTEGER, human_reason TEXT,
                 created_at INTEGER)"#,
            // 旧库迁移：v0.2.23 新增 served_host（实际服务上游 host），已存在则忽略报错
            r#"ALTER TABLE traces ADD COLUMN served_host TEXT"#,
            // v0.2.24 数据修复：v0.2.23 record() 漏 bind served_host 导致列错位，
            // 该批脏行 created_at 为 NULL（排序/统计窗口全部失效），直接清除
            r#"DELETE FROM traces WHERE created_at IS NULL"#,
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
        // v0.2.24：改用 trace_id 作主键 —— 旧 now_ms 毫秒时间戳并发时碰撞，
        // INSERT OR REPLACE 会互相覆盖导致账本/记录丢行（记录不完整）
        id: trace.trace_id.clone(),
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
    // v0.2.24 修复：此前漏 bind served_host 导致其后所有列左移错位
    // （final_status_code/created_at 变 NULL → 成功率统计恒不真实），必须与占位符一一对应
    let _ = sqlx::query(
        r#"INSERT OR REPLACE INTO traces
           (trace_id, received_at_ms, finished_at_ms, client_key_hash, client_key_name,
            model_alias, resolved_model, node_group, served_host, is_stream, is_cached,
            final_status_code, final_error_label,
            billed_prompt, billed_completion, billed_total,
            upstream_prompt, upstream_completion, upstream_total,
            total_latency_ms, human_reason, created_at)
           VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"#)
        .bind(&trace.trace_id)
        .bind(trace.received_at_ms as i64)
        .bind(trace.finished_at_ms.map(|x| x as i64))
        .bind(&trace.client_key_hash)
        .bind(&trace.client_key_name)
        .bind(&trace.model_alias)
        .bind(&trace.resolved_model)
        .bind(&trace.node_group)
        .bind(&trace.served_host)
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
    // 请求数/成功数以 traces.final_status_code 为准（2xx 才算成功，5xx/429/超时等失败不再被误计入）
    let count_row = sqlx::query(
        r#"SELECT
             COUNT(*) AS total,
             COALESCE(SUM(CASE WHEN final_status_code >= 200 AND final_status_code < 300 THEN 1 ELSE 0 END),0) AS ok
           FROM traces WHERE created_at >= ?"#)
        .bind(since).fetch_one(p).await;
    let (total, ok) = match count_row {
        Ok(r) => (
            r.try_get::<i64, _>("total").unwrap_or(0),
            r.try_get::<i64, _>("ok").unwrap_or(0),
        ),
        Err(e) => { tracing::error!(error = %e, "billing_summary 计数查询失败"); (0, 0) }
    };
    let row = sqlx::query(
        r#"SELECT
             COALESCE(SUM(prompt_tokens),0) AS ti,
             COALESCE(SUM(completion_tokens),0) AS to_,
             COALESCE(SUM(final_charge_cny),0) AS charge
           FROM client_ledger WHERE created_at_ms >= ?"#)
        .bind(since).fetch_one(p).await;
    match row {
        Ok(r) => {
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
        Err(e) => { tracing::error!(error = %e, "billing_summary 查询失败"); json!({"window_label": window, "requests_total": total.max(0) as u64, "requests_ok": ok.max(0) as u64, "tokens_input": 0, "tokens_output": 0, "total_charge_cny": 0.0}) }
    }
}

/// 行 → JSON（防御性读取：旧版本脏行可能存在 NULL/文本错位，get::<T> 严格解码会 panic
/// 导致 list_traces/export 全体失败，因此全部用 try_get 兜底）
fn trace_row_json(r: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    let s = |col: &str| r.try_get::<String, _>(col).unwrap_or_default();
    let i = |col: &str| r.try_get::<i64, _>(col).unwrap_or(0);
    json!({
        "trace_id": s("trace_id"),
        "model": s("model_alias"),
        "resolved_model": s("resolved_model"),
        "node_group": s("node_group"),
        "served_host": s("served_host"),
        "client_key_name": s("client_key_name"),
        "status": i("final_status_code"),
        "tokens": i("billed_total"),
        "latency": i("total_latency_ms"),
        "created_at": i("created_at"),
        "is_stream": i("is_stream") == 1,
    })
}

const TRACE_SELECT_COLS: &str = "trace_id, model_alias, resolved_model, node_group, served_host,
         client_key_name, is_stream, is_cached, final_status_code, billed_total,
         total_latency_ms, created_at";

/// 最近转发记录（导出 Excel 用，防御性读取）
/// limit = -1 时 SQL `LIMIT -1` 即无上限，导出全部记录（v0.2.27 起不再截断 2000 条）
pub async fn recent_traces(app: &AppState, limit: i64) -> Vec<serde_json::Value> {
    let DbPool::Sqlite(p) = &app.db else { return vec![] };
    let rows = sqlx::query(&format!(
        "SELECT {} FROM traces ORDER BY created_at DESC LIMIT ?",
        TRACE_SELECT_COLS))
        .bind(limit).fetch_all(p).await;
    match rows {
        Ok(rows) => rows.iter().map(trace_row_json).collect(),
        Err(e) => { tracing::error!(error = %e, "recent_traces 查询失败"); vec![] }
    }
}

/// 分页查询转发记录（链路追踪页）：返回 { items, total, page, page_size }
pub async fn paged_traces(app: &AppState, page: i64, page_size: i64) -> serde_json::Value {
    let DbPool::Sqlite(p) = &app.db else {
        return json!({"items": [], "total": 0, "page": 1, "page_size": page_size});
    };
    let page = page.max(1);
    let page_size = page_size.clamp(10, 500);
    let total: i64 = sqlx::query("SELECT COUNT(*) AS c FROM traces")
        .fetch_one(p).await.ok()
        .and_then(|r| r.try_get::<i64, _>("c").ok()).unwrap_or(0);
    let offset = (page - 1) * page_size;
    let rows = sqlx::query(&format!(
        "SELECT {} FROM traces ORDER BY created_at DESC, trace_id DESC LIMIT ? OFFSET ?",
        TRACE_SELECT_COLS))
        .bind(page_size).bind(offset).fetch_all(p).await;
    let items: Vec<serde_json::Value> = match rows {
        Ok(rows) => rows.iter().map(trace_row_json).collect(),
        Err(e) => { tracing::error!(error = %e, "paged_traces 查询失败"); vec![] }
    };
    json!({
        "items": items,
        "total": total.max(0),
        "page": page,
        "page_size": page_size,
    })
}

/// 批量删除转发记录：返回删除条数
pub async fn delete_traces(app: &AppState, ids: &[String]) -> u64 {
    let DbPool::Sqlite(p) = &app.db else { return 0 };
    if ids.is_empty() { return 0; }
    let Ok(json_ids) = serde_json::to_string(ids) else { return 0 };
    // json_each 展开参数列表，避免动态拼接占位符
    match sqlx::query("DELETE FROM traces WHERE trace_id IN (SELECT value FROM json_each(?))")
        .bind(&json_ids).execute(p).await
    {
        Ok(res) => res.rows_affected(),
        Err(e) => { tracing::error!(error = %e, "delete_traces 失败"); 0 }
    }
}

