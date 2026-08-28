//! 优雅关闭 token：所有 in-flight SubAttempt 完成后再 axum shutdown
pub use super::wait_for_termination as signal_wait;  // re-export
