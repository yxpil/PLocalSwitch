//! 按节点组隔离的 reqwest::Client 池（超时/连接上限独立）
use crate::config::{ConnPoolCfg, HttpConfig, UpstreamNode};
#[allow(unused_imports)] use crate::config::TimeoutConfig;
use dashmap::DashMap;
pub struct HttpPool { pub by_node: DashMap<String, reqwest::Client> }
impl HttpPool {
    pub fn new() -> Self { Self { by_node: DashMap::new() } }
    pub fn build(&self, node: &UpstreamNode, global: &HttpConfig) -> reqwest::Client {
        let t = node.timeouts_override.as_ref().unwrap_or(&global.timeouts);
        let pool = node.connect_pool.as_ref();
        let mut b = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_millis(t.connect_ms))
            .read_timeout(std::time::Duration::from_millis(t.read_ms))
            .use_rustls_tls().https_only(false);
        if let Some(p) = pool { b = b.pool_max_idle_per_host(p.max_idle_per_host); }
        b.build().expect("reqwest client build")
    }
}
impl Default for HttpPool { fn default() -> Self { Self::new() } }

pub use HttpPool as ReqwestPoolGroup;

impl ReqwestPoolGroup {
    pub fn from_cfg(_cfg: &crate::config::AppConfig) -> crate::error::AppResult<Self> {
        Ok(Self::new())
    }
}
