//! 宽容解析：strict 失败时，回退到 flex（字段别名，如 output_text/choices[].text/content 通用）
use crate::error::AppResult;
use crate::models::ChatCompletionResponse;
pub fn parse_strict_or_flex(_bytes: bytes::Bytes) -> AppResult<ChatCompletionResponse> {
    // TODO: strict → anyhow!fail → flex 匹配 schema_aliases
    Err(crate::error::AppError::Labeled { label: crate::error::ErrorLabel::JsonParseFail, message: "flex_parser stub".into() })
}
