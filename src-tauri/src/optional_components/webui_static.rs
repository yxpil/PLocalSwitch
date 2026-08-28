//! 纯服务器模式下：axum 直接 serve React dist 静态资源（Tauri shell 用不到）
pub fn mount<S: Clone + Send + Sync + 'static>() -> axum::Router<S> { axum::Router::new() }
