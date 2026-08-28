//! storage 模块命令：文件列表 / 读取 / 写入
#![cfg(feature = "desktop-shell")]

use crate::error::CommandResult;
use crate::models::ApiResponse;
use crate::services;
use crate::services::storage::FileItem;
use crate::state::AppState;
use std::sync::Arc;
use tauri::State;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct WriteRequest {
    pub relative_path: String,
    pub content: String,
}

/// 列出目录文件
#[tauri::command]
pub fn list_files(state: State<'_, Arc<AppState>>, relative_path: Option<String>) -> CommandResult<ApiResponse<Vec<FileItem>>> {
    let _ = state.bump_request();
    let rp = relative_path.unwrap_or_else(|| String::from(""));
    let items = services::storage::list_files(&rp)?;
    tracing::debug!("列出目录 {}，文件数 {}", rp, items.len());
    Ok(ApiResponse::ok(items))
}

/// 读取文本文件
#[tauri::command]
pub fn read_text_file(state: State<'_, Arc<AppState>>, relative_path: String) -> CommandResult<ApiResponse<String>> {
    let _ = state.bump_request();
    let content = services::storage::read_text_file(&relative_path)?;
    tracing::debug!("读取文件: {} ({} 字节)", relative_path, content.len());
    Ok(ApiResponse::ok(content))
}

/// 写入文本文件
#[tauri::command]
pub fn write_text_file(state: State<'_, Arc<AppState>>, req: WriteRequest) -> CommandResult<ApiResponse<bool>> {
    let _ = state.bump_request();
    services::storage::write_text_file(&req.relative_path, &req.content)?;
    tracing::info!("写入文件: {} ({} 字节)", req.relative_path, req.content.len());
    Ok(ApiResponse::ok(true))
}
