//! =============================================================
//!  交付物 4：retry_controller —— attempt_chain 非流式真实执行
//!  核心循环：遍历候选节点 → 每个节点按 cached/sniffed protocol 调 adapter
//!            → 真实发起上游 HTTP 请求 → 解析响应 → 成功即返回
//!            → 失败按 label 决定是否继续下一个节点 / 是否终止
//! =============================================================
use crate::backend_adapters::adapter_for;
use crate::error::{AppError, ErrorLabel};
use crate::flex_adapter::retry_analysis::{build_human_reason, RecentWindowStats};
use crate::models::{ChatCompletionRequest, ChatCompletionResponse};
use crate::observability::trace::{GatewayTrace, SubAttempt, SubAttemptOutcome, UsageSnapshot};
use crate::router::{CandidateNode, ProtocolKind};
use crate::state::AppState;
use std::sync::Arc;

pub enum AttemptOutcome {
    /// 成功：最终响应 + 最后一个 ok 的 SubAttempt
    Ok(ChatCompletionResponse, SubAttempt),
    /// 全失败：所有产生的 SubAttempt + 最终 error + 运维侧诊断
    AllFailed(Vec<SubAttempt>, AppError, String),
}

/// 真实发起一次上游尝试（不负责 retry 决策；只负责请求 + 解析 + 成功时写 sub）
async fn execute_one(
    c:    &CandidateNode,
    req:  &ChatCompletionRequest,
    proto: ProtocolKind,
    sub:  &mut SubAttempt,
) -> Result<ChatCompletionResponse, AppError> {
    let adapter = adapter_for(proto);
    let rb = adapter.translate_request(req, c).await?;
    let resp = rb.send().await?; // reqwest::Error -> AppError::Reqwest
    let status = resp.status();
    sub.http_status_code = Some(status.as_u16());
    if !status.is_success() {
        let label = match status.as_u16() {
            401 | 403 => ErrorLabel::Auth401403,
            429        => ErrorLabel::Http429,
            400 | 404 | 405 => ErrorLabel::BadParam4xx,
            406..=499  => ErrorLabel::BadParam4xx,
            500..=599  => ErrorLabel::Upstream5xx,
            _          => ErrorLabel::Unknown,
        };
        return Err(AppError::Labeled { label, message: format!("upstream status {}", status.as_u16()) });
    }
    let bytes = resp.bytes().await?;
    let parsed = adapter.parse_response_body(bytes)?;
    let u = parsed.usage.clone().unwrap_or_default();
    sub.finish_ok(status.as_u16(), UsageSnapshot {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
    });
    Ok(parsed)
}

#[allow(unused_variables)]
pub async fn attempt_chain(
    state:      &Arc<AppState>,
    trace:      &mut GatewayTrace,
    candidates: &[CandidateNode],
    req:        ChatCompletionRequest,
    is_stream:  bool,
) -> AttemptOutcome {
    let cfg = &state.cfg_swap.load().flex_adapter;
    let max_subs = cfg.global_max_sub_attempts.max(1);
    let policy = state.cfg_swap.load().policy.clone();
    let mut subs: Vec<SubAttempt> = Vec::new();
    let mut last_err: Option<AppError> = None;

    // ------- 流式：由 flex_adapter::execute_stream 单独处理（此处仅兜底）-------
    if is_stream {
        let err = AppError::Labeled { label: ErrorLabel::Internal, message: "stream via attempt_chain not supported".into() };
        let hr = build_human_reason(trace, &subs, &policy, &RecentWindowStats::default());
        return AttemptOutcome::AllFailed(subs, err, hr);
    }

    // ------- 非流式：真实多轮候选执行 -------
    for c in candidates {
        if subs.len() as u32 >= max_subs { break; }
        let masked = c.to_masked(&state.cfg.masking);
        let mut sub = SubAttempt::new(&trace.trace_id, &c.node_id, &c.group_id);
        sub.masked_endpoint = masked.endpoint.clone();
        sub.masked_token   = masked.api_key.clone();
        let proto = crate::flex_adapter::protocol_sniffer::try_cached(state, &c.node_id).unwrap_or(c.protocol);
        sub.protocol = proto.to_string();

        match execute_one(c, &req, proto, &mut sub).await {
            Ok(resp) => return AttemptOutcome::Ok(resp, sub),
            Err(e) => {
                let lbl = e.label();
                let status = sub.http_status_code;
                let outcome = if matches!(lbl, ErrorLabel::BadParam4xx | ErrorLabel::Auth401403) {
                    SubAttemptOutcome::FailedTerminal
                } else {
                    SubAttemptOutcome::FailedRetried
                };
                sub.finish_fail(lbl, status, outcome);
                last_err = Some(e);
                subs.push(sub);
                // 客户端错误立即终止，不浪费重试配额
                if matches!(lbl, ErrorLabel::BadParam4xx | ErrorLabel::Auth401403) { break; }
            }
        }
    }

    // 走到这里 = 没一个节点成功 → AllFailed，拼 human_reason
    let hr = build_human_reason(trace, &subs, &policy, &RecentWindowStats::default());
    let _act = last_err.as_ref().map(|e| crate::flex_adapter::retry_analysis::triage_action(e.label()));
    AttemptOutcome::AllFailed(subs, last_err.unwrap_or_else(|| AppError::Labeled {
        label: ErrorLabel::Unknown, message: "attempt exhausted without final error".into(),
    }), hr)
}
