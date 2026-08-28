//! =============================================================
//!  6. 计费统计 + 分词器对账（双账本隔离）
//! =============================================================
//!  账本 ① 上游真实成本账本：每次 SubAttempt 写入 upstream_ledger，
//!         无论失败/成功/重试全部累加，用于内部采购成本核算。
//!  账本 ② 客户端计费账本：仅按客户端请求最终结果写入 client_ledger：
//!         · 完全失败无输出 → 0
//!         · 非流式成功：按 A usage（优先上游）+ B 分词器本地值对账偏差后写
//!         · 流式中途断开：按已输出 delta token 累加扣费
//!         · 多次重试：客户端只计 1 次，绝不把内部重试开销叠加给用户
//! =============================================================
pub mod tokenizer_pool;      // 外部分词器文件(bpe/vocab/merges)/tiktoken 绑定，每个模型独立实例
pub mod counter;             // 三档优先级：上游usage → 本地分词 → 流式delta累加
pub mod audit;               // 对账：A vs B 偏差率，阈值告警 metrics，写 audit_records
pub mod ledger;              // 双账本（upstream_ledger / client_ledger）写入 DB + 聚合
pub mod pricing;             // 每模型采购价/售价，费率分组，client_key 余额/配额
pub mod client_key_mgr;      // 网关自有 API Key（RPM/TPM/最大并发/余额/硬配额/透支开关）

use crate::config::AuditConfig;
use crate::error::AppResult;
use crate::observability::trace::UsageSnapshot;

/// 计算 A(上游usage) 与 B(本地分词) 的对账偏差率 + 写 audit_records
pub async fn commit_audit_record(
    request_id:     &str,
    model:          &str,
    upstream_usage: UsageSnapshot,   // A
    local_usage:    UsageSnapshot,   // B
    audit_cfg:      &AuditConfig,
) -> AppResult<audit::AuditRecord> {
    Ok(audit::AuditRecord::compute(
        request_id, model, upstream_usage, local_usage,
        audit_cfg.discrepancy_alarm_percent,
    ))
}
