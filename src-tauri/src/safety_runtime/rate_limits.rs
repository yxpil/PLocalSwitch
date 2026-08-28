//! 网关侧：全局 + per_node 的限流组骨架（区别于 gateway_api::rate_limit 按 client_key）
use crate::config::AppConfig;
pub struct RateLimitGroup { pub global_rpm: u32, #[allow(dead_code)] cfg: AppConfig }
impl RateLimitGroup {
    pub fn new(cfg: &AppConfig) -> Self { Self { global_rpm: 0, cfg: cfg.clone() } }
}
