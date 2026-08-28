//! =============================================================
//!  交付物 4：故障研判 → human_readable_reason
//!  规则：
//!    1. 当前错误标签 + 该标签的解释说明
//!    2. 近 analysis_history_window_seconds 内 相同 client_key_hash + node_id
//!       的同类 error_label 历史次数、成功率
//!    3. 是否命中 policy.retry_on，已触发过几次重试
//!  仅用于 trace 入库 + 运维侧 trace viewer；禁止 gateway_api 错误响应透传
//! =============================================================
use crate::config::PolicyConfig;
use crate::error::ErrorLabel;
use crate::observability::trace::{GatewayTrace, SubAttempt};

/// 输入：trace / 当前所有 sub 尝试 / policy → 拼接人类可读诊断
pub fn build_human_reason(
    trace:   &GatewayTrace,
    subs:    &[SubAttempt],
    policy:  &PolicyConfig,
    recent_stats: &RecentWindowStats,
) -> String {
    // 第一部分：最后一次失败细节
    let last = subs.iter().rev().find(|s| s.error_label.is_some());
    let head = match last {
        Some(s) => {
            let lbl = s.error_label.unwrap();
            let ep = if s.masked_endpoint.is_empty() { "?" } else { s.masked_endpoint.as_str() };
            format!(
                "[FAILED] label={lbl} node={}({ep}) status={:?} retry_allowed={}",
                s.node_id, s.http_status_code,
                lbl.is_candidate_retry(&policy.retry_on),
            )
        }
        None => "[FAILED] no upstream attempt executed.".into(),
    };

    // 第二部分：整体 attempt 统计
    let total = subs.len();
    let failed = subs.iter().filter(|s| matches!(s.outcome, Some(crate::observability::trace::SubAttemptOutcome::FailedRetried | crate::observability::trace::SubAttemptOutcome::FailedTerminal | crate::observability::trace::SubAttemptOutcome::StreamAborted))).count();
    let mid = format!("attempts={total} failed={failed} trace_id={}", trace.trace_id);

    // 第三部分：近 N 秒该 node_id + error_label 的窗口统计（用于运维判断 节点抖动 vs 偶发）
    let tail = if recent_stats.window_seconds > 0 {
        format!(
            " | recent_{}s same node+label: count={} ok={} timeout={}",
            recent_stats.window_seconds,
            recent_stats.same_label_total,
            recent_stats.same_label_success,
            recent_stats.same_label_timeout,
        )
    } else { String::new() };

    // 第四部分：max_attempts 上限提醒
    let cap = format!(
        " | policy: retries_on=[{}] window={}s",
        active_retry_flags(&policy.retry_on).join(","),
        policy.analysis_history_window_seconds,
    );

    format!("{head} | {mid}{tail}{cap}")
}

/// 近 analysis_history_window_seconds 窗口内相同 client + node + label 的聚合统计
#[derive(Debug, Clone, Default)]
pub struct RecentWindowStats {
    pub window_seconds:     u64,
    pub same_label_total:   u32,
    pub same_label_success: u32,
    pub same_label_timeout: u32,
}

/// 哪些 retry 开关打开（仅诊断展示，不影响逻辑）
pub fn active_retry_flags(p: &crate::config::RetryOnCfg) -> Vec<&'static str> {
    let mut out = Vec::new();
    if p.network_connect_refused { out.push("connect_refused"); }
    if p.dns_fail               { out.push("dns_fail"); }
    if p.connect_timeout        { out.push("connect_timeout"); }
    if p.read_timeout           { out.push("read_timeout"); }
    if p.tls_error              { out.push("tls_error"); }
    if p.http_429               { out.push("429"); }
    if p.http_5xx               { out.push("5xx"); }
    if p.auth_401_403           { out.push("auth_401_403"); }
    if p.bad_param_4xx          { out.push("bad_param_4xx"); }
    if p.sse_premature_close    { out.push("sse_close"); }
    if p.json_parse_fail        { out.push("json_parse"); }
    out
}

/// 给出失败时建议的网关操作（仅运维日志，不直接控制行为）
pub fn triage_action(label: ErrorLabel) -> &'static str {
    use ErrorLabel::*;
    match label {
        Auth401403        => "[ACTION] 检查对应节点 api_keys 是否过期/已吊销，考虑从 node_groups 临时摘除",
        NetworkConnectRefused | DnsFail | TlsError | ConnectTimeout
                          => "[ACTION] 核对 endpoint 可达性、TLS 证书、DNS；命中 autotrim.temporary_ban_seconds_when_fault 临时摘除",
        Http429           => "[ACTION] 对应节点上游限流，降低该节点权重或加大 node_group 分散比例",
        Upstream5xx       => "[ACTION] 上游提供商 5xx，如持续 3 min 以上考虑降权",
        SsePrematureClose | SseFormatInvalid | SseMidDrop
                          => "[ACTION] 检查流式上游稳定度，若同节点比例 >5% 开启临时 ban",
        JsonParseFail | SchemaMismatch
                          => "[ACTION] flexible_parser flex 模式仍失败，检查上游协议版本是否变动",
        BadParam4xx       => "[ACTION] 客户端请求本身非法，不重试，不扣除客户端",
        ReadTimeout       => "[ACTION] 加大 read_ms 或切换 read 更快的节点",
        Internal | Unknown => "[ACTION] 网关内部错误，检查最近 cargo build / 配置变更",
    }
}
