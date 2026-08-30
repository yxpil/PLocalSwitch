//! 错误日志记录工具（v0.2.25）
//!  - 网关转发链路上的失败（路由失败 / 非流式候选链失败 / 流式候选链失败 / 流中断）
//!    统一落库到 error_logs 表，供「链路追踪 → 错误日志」面板查询
//!  - 脱敏原则：message 入库前经 scrub() 打码（bearer token / sk- 密钥 / 长凭证串），
//!    host 级明细允许保留（与 ChainFailed 对客户端的透明度约定一致），绝不存完整 token/URL 查询串
//!  - 硬上限：超过 MAX_KEEP 条自动裁剪到 TRIM_TO 条，禁止磁盘无限制膨胀
use crate::state::{AppState, DbPool};
use serde_json::json;
use sqlx::Row;

const MAX_KEEP: i64 = 5000;
const TRIM_TO: i64 = 4000;

/// 启动时建表（幂等）
pub async fn init_schema(app: &AppState) {
    if let DbPool::Sqlite(p) = &app.db {
        for sql in [
            r#"CREATE TABLE IF NOT EXISTS error_logs (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 ts_ms INTEGER, level TEXT, context TEXT,
                 label TEXT, message TEXT, trace_id TEXT)"#,
            r#"CREATE INDEX IF NOT EXISTS idx_error_logs_ts ON error_logs(ts_ms)"#,
        ] {
            let _ = sqlx::query(sql).execute(p).await;
        }
    }
}

/// 凭证打码：bearer token / sk- 开头密钥 / 40+ 位长串（可能是密钥）一律打码
fn scrub(s: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut mask_next = false;
    for tok in s.split_whitespace() {
        if mask_next {
            out.push("***".into());
            mask_next = false;
            continue;
        }
        let lower = tok.to_lowercase();
        if lower == "bearer" || lower == "authorization:" || lower == "x-api-key:" {
            out.push(tok.to_string());
            mask_next = true;
        } else if lower.contains("sk-") || tok.len() > 48 {
            out.push("***".into());
        } else {
            out.push(tok.to_string());
        }
    }
    out.join(" ")
}

/// 从 AppError 生成可入库的脱敏 message（候选链明细 host 级保留，其余按标签级）
pub fn sanitize_message(e: &crate::error::AppError) -> String {
    match e {
        crate::error::AppError::ChainFailed { detail } => scrub(detail),
        _ => {
            let label = e.label();
            crate::error::client_message_for_label_pub(&label).to_string()
        }
    }
}

/// 记录一条错误日志（fire-and-forget，失败静默不影响转发主链路）
pub async fn record(app: &AppState, context: &str, label: &str, message: &str, trace_id: &str) {
    let DbPool::Sqlite(p) = &app.db else { return };
    let now = crate::observability::trace::now_ms() as i64;
    let _ = sqlx::query(
        "INSERT INTO error_logs (ts_ms, level, context, label, message, trace_id) VALUES (?,?,?,?,?,?)")
        .bind(now).bind("error").bind(context).bind(label).bind(scrub(message)).bind(trace_id)
        .execute(p).await;
    // 硬上限保护：超过 MAX_KEEP 才裁剪到 TRIM_TO（平时不跑 DELETE，减少写放大）
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM error_logs")
        .fetch_one(p).await.unwrap_or(0);
    if count > MAX_KEEP {
        let _ = sqlx::query(
            "DELETE FROM error_logs WHERE id IN (SELECT id FROM error_logs ORDER BY ts_ms DESC, id DESC LIMIT -1 OFFSET ?)")
            .bind(TRIM_TO).execute(p).await;
    }
}

/// 行 → JSON（防御性读取）
fn row_json(r: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    let s = |col: &str| r.try_get::<String, _>(col).unwrap_or_default();
    let i = |col: &str| r.try_get::<i64, _>(col).unwrap_or(0);
    json!({
        "id": i("id"),
        "ts_ms": i("ts_ms"),
        "level": s("level"),
        "context": s("context"),
        "label": s("label"),
        "message": s("message"),
        "trace_id": s("trace_id"),
    })
}

/// 分页查询错误日志：返回 { items, total, page, page_size }
pub async fn paged_logs(app: &AppState, page: i64, page_size: i64) -> serde_json::Value {
    let DbPool::Sqlite(p) = &app.db else {
        return json!({"items": [], "total": 0, "page": 1, "page_size": page_size});
    };
    let page = page.max(1);
    let page_size = page_size.clamp(10, 500);
    let total: i64 = sqlx::query("SELECT COUNT(*) AS c FROM error_logs")
        .fetch_one(p).await.ok()
        .and_then(|r| r.try_get::<i64, _>("c").ok()).unwrap_or(0);
    let offset = (page - 1) * page_size;
    let rows = sqlx::query(
        "SELECT id, ts_ms, level, context, label, message, trace_id
         FROM error_logs ORDER BY ts_ms DESC, id DESC LIMIT ? OFFSET ?")
        .bind(page_size).bind(offset).fetch_all(p).await;
    let items: Vec<serde_json::Value> = match rows {
        Ok(rows) => rows.iter().map(row_json).collect(),
        Err(e) => { tracing::error!(error = %e, "error_logs 查询失败"); vec![] }
    };
    json!({ "items": items, "total": total.max(0), "page": page, "page_size": page_size })
}

/// 清空全部错误日志：返回删除条数
pub async fn clear_logs(app: &AppState) -> u64 {
    let DbPool::Sqlite(p) = &app.db else { return 0 };
    sqlx::query("DELETE FROM error_logs").execute(p).await
        .map(|r| r.rows_affected()).unwrap_or(0)
}

/// 按 id 批量删除：返回删除条数
pub async fn delete_logs(app: &AppState, ids: &[i64]) -> u64 {
    let DbPool::Sqlite(p) = &app.db else { return 0 };
    if ids.is_empty() { return 0; }
    let Ok(json_ids) = serde_json::to_string(ids) else { return 0 };
    sqlx::query("DELETE FROM error_logs WHERE id IN (SELECT value FROM json_each(?))")
        .bind(&json_ids).execute(p).await
        .map(|r| r.rows_affected()).unwrap_or(0)
}
