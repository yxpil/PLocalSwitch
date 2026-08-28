//! =============================================================
//!  交付物 3：上游服务栈指纹识别（仅统计标签，禁止业务路由判断）
//! =============================================================
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, strum::Display, Default)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UpstreamStack { JavaSpring, Go, Rust, NodeJs, Python, Cloudflare, Nginx, #[default] Unknown }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FingerprintResult {
    pub stack: UpstreamStack,
    pub confidence: u8,
    pub signals: Vec<String>,
}

pub fn sniff(headers: &BTreeMap<String, String>, body_snippet: Option<&str>) -> FingerprintResult {
    let mut signals: Vec<String> = Vec::new();
    let mut scores: BTreeMap<UpstreamStack, u8> = BTreeMap::new();
    let add = |m: &mut BTreeMap<UpstreamStack, u8>, s, n| { *m.entry(s).or_insert(0) += n; };
    for (k, v) in headers {
        let v_low = v.to_ascii_lowercase();
        if k == "server" {
            signals.push(format!("server:{v}"));
            if v_low.contains("openresty") || v_low.contains("nginx") { add(&mut scores, UpstreamStack::Nginx, 25); }
            if v_low.contains("cloudflare") { add(&mut scores, UpstreamStack::Cloudflare, 50); }
            if v_low.contains("caddy")      { add(&mut scores, UpstreamStack::Go, 20); }
        }
        if k == "x-powered-by" {
            signals.push(format!("x-powered-by:{v}"));
            if v_low.contains("express") || v_low.contains("node") { add(&mut scores, UpstreamStack::NodeJs, 60); }
            if v_low.contains("servlet") || v_low.contains("jsp")  { add(&mut scores, UpstreamStack::JavaSpring, 60); }
            if v_low.contains("next.js")                           { add(&mut scores, UpstreamStack::NodeJs, 70); }
        }
        if k == "x-envoy-upstream-service-time" { add(&mut scores, UpstreamStack::Go, 10); }
        if k == "cf-ray"                        { add(&mut scores, UpstreamStack::Cloudflare, 90); }
    }
    if let Some(b) = body_snippet {
        let bl = b.to_ascii_lowercase();
        if bl.contains("whitelabel error") || bl.contains("spring boot") { add(&mut scores, UpstreamStack::JavaSpring, 80); signals.push("body:spring_whitelabel".into()); }
        if bl.contains("traceback") || bl.contains("fastapi")            { add(&mut scores, UpstreamStack::Python, 50);     signals.push("body:python_stacktrace".into()); }
        if bl.contains("invalid json body") && bl.contains("axum")        { add(&mut scores, UpstreamStack::Rust, 70);       signals.push("body:axum_parse".into()); }
    }
    let (stack, confidence) = scores.into_iter()
        .max_by_key(|(_, v)| *v).map(|(k, v)| (k, v.min(100)))
        .unwrap_or((UpstreamStack::Unknown, 0));
    FingerprintResult { stack, confidence, signals }
}
