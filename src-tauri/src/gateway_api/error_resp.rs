//! axum AppError → OpenAI 标准错误响应（绝不泄露上游）
use crate::error::{AppError, ErrorLabel};
use axum::body::Body;
use axum::response::{IntoResponse, Response};
pub struct AppErrorResponse(pub AppError);
impl IntoResponse for AppErrorResponse {
    fn into_response(self) -> Response<Body> {
        let status = match self.0.label() {
            ErrorLabel::BadParam4xx       => 400,
            ErrorLabel::Auth401403        => 401,
            ErrorLabel::Http429           => 429,
            ErrorLabel::Http413           => 413,
            ErrorLabel::Upstream5xx       => 502,
            ErrorLabel::NetworkConnectRefused | ErrorLabel::DnsFail | ErrorLabel::TlsError
                                          => 503,
            ErrorLabel::ConnectTimeout | ErrorLabel::ReadTimeout
                                          => 504,
            _ => 500,
        };
        let body = serde_json::to_string(&self.0.to_openai_error()).unwrap_or_else(|_| r#"{"error":{"message":"internal"}}"#.into());
        Response::builder().status(status)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(body)).unwrap()
    }
}
impl From<AppError> for AppErrorResponse {
    fn from(e: AppError) -> Self { Self(e) }
}
