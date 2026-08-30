//! =============================================================
//!  交付物 3：链路追踪核心结构体 GatewayTrace / SubAttempt
//! =============================================================
use crate::error::ErrorLabel;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;
use ulid::Ulid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub prompt_tokens:     u32,
    pub completion_tokens: u32,
    pub total_tokens:      u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::Display)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SubAttemptOutcome {
    Ok, FailedRetried, FailedTerminal, StreamAborted, CanceledByClient,
}

/// SubAttempt = 一次上游调用尝试
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAttempt {
    pub sub_attempt_id:   String,
    pub trace_id:         String,
    pub node_id:          String,
    pub node_group_id:    String,
    pub protocol:         String,
    pub masked_endpoint:  String,
    pub masked_token:     String,
    pub fingerprint_labels: Vec<String>,
    pub started_at:       u128,
    pub finished_at:      Option<u128>,
    pub latency_ms:       Option<u64>,
    pub ttft_ms:          Option<u64>,
    pub http_status_code: Option<u16>,
    pub outcome:          Option<SubAttemptOutcome>,
    pub error_label:      Option<ErrorLabel>,
    pub usage:            UsageSnapshot,
    pub retry_reason:     Option<String>,
}
impl SubAttempt {
    pub fn new(trace_id: impl Into<String>, node_id: impl Into<String>, group_id: impl Into<String>) -> Self {
        Self {
            sub_attempt_id: Ulid::new().to_string(),
            trace_id: trace_id.into(), node_id: node_id.into(), node_group_id: group_id.into(),
            protocol: String::new(), masked_endpoint: String::new(), masked_token: String::new(),
            fingerprint_labels: Vec::new(), started_at: now_ms(),
            finished_at: None, latency_ms: None, ttft_ms: None,
            http_status_code: None, outcome: None, error_label: None,
            usage: UsageSnapshot::default(), retry_reason: None,
        }
    }
    pub fn finish_ok(&mut self, status: u16, usage: UsageSnapshot) {
        let end = now_ms();
        self.finished_at = Some(end);
        self.latency_ms = Some((end - self.started_at) as u64);
        self.http_status_code = Some(status);
        self.outcome = Some(SubAttemptOutcome::Ok);
        self.usage = usage;
    }
    pub fn finish_fail(&mut self, label: ErrorLabel, status: Option<u16>, reason: SubAttemptOutcome) {
        let end = now_ms();
        self.finished_at = Some(end);
        self.latency_ms = Some((end - self.started_at) as u64);
        self.http_status_code = status;
        self.error_label = Some(label);
        self.outcome = Some(reason);
    }
}

/// GatewayTrace = 一次客户端请求全生命周期
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayTrace {
    pub trace_id:              String,
    pub received_at_ms:        u128,
    pub finished_at_ms:        Option<u128>,
    pub client_key_hash:       String,
    pub client_key_name:       Option<String>,
    pub remote_addr:           String,
    pub model_alias:           String,
    pub resolved_model:        String,
    pub node_group:            String,
    /// 实际服务本次请求的上游 host（成功时写入；链路追踪展示用，已脱敏只有域名）
    pub served_host:           String,
    pub is_stream:             bool,
    pub is_cached:             bool,
    pub final_status_code:     u16,
    pub final_error_label:     Option<ErrorLabel>,
    pub billed_usage:          UsageSnapshot,
    pub upstream_usage_sum:    UsageSnapshot,
    pub sub_attempt_ids:       Vec<String>,
    /// ⚠️ 仅运维侧：人类可读诊断，禁止 gateway_api 错误响应透传
    pub human_readable_reason: Option<String>,
    pub total_latency_ms:      Option<u64>,
}
impl GatewayTrace {
    pub fn new(model_alias: impl Into<String>, remote_addr: impl Into<String>) -> Self {
        Self {
            trace_id: Uuid::now_v7().to_string(),
            received_at_ms: now_ms(), finished_at_ms: None,
            client_key_hash: String::new(), client_key_name: None,
            remote_addr: remote_addr.into(),
            model_alias: model_alias.into(), resolved_model: String::new(), node_group: String::new(),
            served_host: String::new(),
            is_stream: false, is_cached: false, final_status_code: 0, final_error_label: None,
            billed_usage: UsageSnapshot::default(), upstream_usage_sum: UsageSnapshot::default(),
            sub_attempt_ids: Vec::new(), human_readable_reason: None, total_latency_ms: None,
        }
    }
    pub fn set_client_key(&mut self, key_cleartext: &str, name: Option<String>) {
        let h = blake3::hash(key_cleartext.as_bytes());
        self.client_key_hash = hex::encode(&h.as_bytes()[..16]);
        self.client_key_name = name;
    }
    pub fn close(&mut self, status: u16, label: Option<ErrorLabel>) {
        let end = now_ms();
        self.finished_at_ms = Some(end);
        self.final_status_code = status;
        self.final_error_label = label;
        self.total_latency_ms = Some((end - self.received_at_ms) as u64);
    }
}

pub fn now_ms() -> u128 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}
