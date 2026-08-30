//! 网关自有 API Key 校验 + 提取 client_key 实体
use crate::config::ClientKey;
use crate::state::AppState;
use axum::http::request::Parts;
use axum::{async_trait, extract::FromRequestParts, extract::State};
use std::sync::Arc;

/// axum extractor：一次鉴权结果（失败直接返回 401，不让进入 handler）
pub struct AuthedClient {
    pub key_hash: String,
    pub key_name: Option<String>,
    pub entity:   ClientKey,
}

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthedClient {
    type Rejection = axum::response::Response<String>;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let auth = parts.headers.get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok()).unwrap_or("");
        let entity = state.client_keys.read().await
            .verify(auth).cloned()
            .ok_or_else(|| axum::response::Response::builder().status(401)
                .body(r#"{"error":{"message":"Invalid gateway API key","type":"authentication_error","code":"invalid_api_key"}}"#.into())
                .unwrap())?;
        let h = blake3::hash(entity.key.as_bytes());
        Ok(Self {
            key_hash: hex::encode(&h.as_bytes()[..16]),
            key_name: Some(entity.name.clone()),
            entity,
        })
    }
}

// 让 extractor 边界与 State 对齐兼容
#[allow(dead_code)]
fn _unused_state(_: State<Arc<AppState>>) {}
