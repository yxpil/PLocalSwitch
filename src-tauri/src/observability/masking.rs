//! =============================================================
//!  脱敏工具（交付物 3 & 4）
//! =============================================================
use crate::config::MaskingConfig;
use serde_json::Value;
use url::Url;

pub fn mask_token(token: &str, cfg: &MaskingConfig) -> String {
    if !cfg.enabled { return token.to_string(); }
    let t = token.trim();
    if t.is_empty() { return String::new(); }
    let head = cfg.token_show_head.min(t.len());
    let tail = cfg.token_show_tail.min(t.len().saturating_sub(head));
    let mut out = String::with_capacity(head + tail + 16);
    out.push_str(&t[..head]);
    out.push_str("****************");
    out.push_str(&t[t.len() - tail..]);
    out
}

pub fn mask_endpoint(endpoint: &str, cfg: &MaskingConfig) -> String {
    if !cfg.enabled { return endpoint.to_string(); }
    let e = endpoint.trim();
    if e.is_empty() { return String::new(); }
    match Url::parse(e) {
        Ok(u) => {
            let scheme = u.scheme();
            let host = u.host_str().unwrap_or("");
            let port_str = u.port().map(|p| format!(":{p}")).unwrap_or_default();
            let segs: Vec<&str> = u.path_segments().map(|s| s.collect()).unwrap_or_default();
            let keep = cfg.url_preserve_path_segments.min(segs.len());
            let kept = segs[..keep].join("/");
            let tail_star = if segs.len() > keep { "/****" } else { "" };
            format!("{scheme}://{host}{port_str}/{kept}{tail_star}")
        }
        Err(_) => {
            let n = e.len();
            let cut = (n * 2) / 3;
            format!("{}****", &e[..cut.min(n)])
        }
    }
}

pub fn is_sensitive_header(name: &str, cfg: &MaskingConfig) -> bool {
    if !cfg.enabled { return false; }
    let n = name.to_ascii_lowercase();
    cfg.sensitive_headers.iter().any(|x| x.to_ascii_lowercase() == n)
        || n == "authorization" || n == "x-api-key" || n == "cookie"
}

pub fn mask_json_fields(value: &mut Value, cfg: &MaskingConfig) {
    if !cfg.enabled { return; }
    match value {
        Value::Object(obj) => {
            let keys: Vec<String> = obj.keys().cloned().collect();
            for k in keys {
                let hit = cfg.sensitive_body_fields.iter().any(|s| s == &k);
                if let Some(v) = obj.get_mut(&k) {
                    if hit { *v = Value::String("****".into()); }
                    else { mask_json_fields(v, cfg); }
                }
            }
        }
        Value::Array(arr) => { for v in arr.iter_mut() { mask_json_fields(v, cfg); } }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mock_cfg() -> MaskingConfig {
        MaskingConfig {
            enabled: true, sensitive_headers: vec![],
            sensitive_body_fields: vec!["api_key".into(), "token".into()],
            token_show_head: 4, token_show_tail: 4, url_preserve_path_segments: 2,
        }
    }
    #[test]
    fn token_mask_basic() {
        let r = mask_token("sk-1234567890abcdefghij", &mock_cfg());
        assert!(r.starts_with("sk-1") && r.ends_with("ghij") && r.contains("****************"));
    }
    #[test]
    fn endpoint_mask_preserves_host() {
        let r = mask_endpoint("https://api.deepseek.com/v1/chat/completions", &mock_cfg());
        assert!(r.starts_with("https://api.deepseek.com/v1/chat"));
        assert!(r.ends_with("****"));
    }
}
