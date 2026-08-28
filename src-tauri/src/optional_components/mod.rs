//! =============================================================
//!  10. 可选模块（默认未启用，feature 或 trait 显式加载）
//! =============================================================
pub mod redis_cache;         // 替换 cache_pool::backend::InMemoryBackend
pub mod webui_static;        // axum ServeDir 直接提供 React dist/ 静态页面（纯网关服务器场景）
pub mod ldap_auth;           // 企业内网：管理端 LDAP/SSO 登录
pub mod webhook_notify;      // 账单超阈值 / SSE 断流频发 → 飞书/钉钉/webhook 告警
