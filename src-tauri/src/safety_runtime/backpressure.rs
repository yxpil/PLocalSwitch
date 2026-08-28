//! 队列满时直接 429（不在此处 sleep/排队）
pub fn reject_429() -> axum::response::Response<String> {
    axum::response::Response::builder().status(429).header("retry-after", "2").body("Too many queued requests".into()).unwrap()
}
