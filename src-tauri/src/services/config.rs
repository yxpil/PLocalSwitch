//! 配置管理服务（网关 gateway.yaml）
//! NOTE: 网关配置统一走 crate::config（含热更新 ArcSwap + bundled 默认）。
//!       此处仅保留一个轻量封装，供历史 storage 路径使用，避免与 commands/config 重复。

use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use crate::services::storage;
use std::path::Path;

/// 从磁盘加载（不存在则回退 bundled 默认并落盘）
pub fn load() -> AppResult<AppConfig> {
    let path = AppConfig::disk_path();
    if !path.exists() {
        return crate::config::reset_to_default();
    }
    let txt = std::fs::read_to_string(&path).map_err(AppError::Io)?;
    serde_yaml::from_str(&txt)
        .map_err(|e| AppError::Config(format!("gateway.yaml parse fail: {e}")))
}

/// 保存配置到磁盘
pub fn save(cfg: &AppConfig) -> AppResult<()> {
    crate::config::save_to_disk(cfg)?;
    Ok(())
}

/// 重置为默认
pub fn reset() -> AppResult<AppConfig> {
    let cfg = crate::config::reset_to_default()?;
    Ok(cfg)
}

fn ensure_parent(path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::Io)?;
    }
    Ok(())
}

#[allow(dead_code)]
fn _unused_storage(_: &storage::FileItem) {}
