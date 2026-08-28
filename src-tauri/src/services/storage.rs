//! 本地文件存储服务
//! 所有操作限制在 APP_DIRS.data_dir 范围内，禁止访问任意路径

use crate::error::{AppError, AppResult};
use crate::state::APP_DIRS;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 文件项元数据
#[derive(Debug, Serialize, Deserialize)]
pub struct FileItem {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub modified_at: i64,
}

/// 确保数据目录存在（首次启动时调用）
pub fn ensure_data_dir() -> AppResult<()> {
    let dirs = [&APP_DIRS.data_dir, &APP_DIRS.config_dir, &APP_DIRS.logs_dir, &APP_DIRS.cache_dir];
    for dir in dirs {
        std::fs::create_dir_all(dir).map_err(AppError::Io)?;
    }
    Ok(())
}

/// 将相对路径安全地拼接到数据目录，并做目录穿越检查
pub fn safe_resolve(relative: &str) -> AppResult<PathBuf> {
    let relative = relative.trim_start_matches(['/', '\\']);
    if relative.is_empty() {
        return Ok(APP_DIRS.data_dir.clone());
    }
    // 不允许 .. 穿越
    if relative.contains("..") {
        return Err(AppError::InvalidPath(relative.into()));
    }
    let joined = APP_DIRS.data_dir.join(relative);
    // 规范化后必须仍在 data_dir 内
    let canonical = match joined.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // 文件可能还不存在，使用父目录校验
            if let Some(parent) = joined.parent() {
                let _ = std::fs::create_dir_all(parent);
                parent.canonicalize().map_err(AppError::Io)?.join(joined.file_name().unwrap_or_default())
            } else {
                joined.clone()
            }
        }
    };
    if !canonical.starts_with(&*APP_DIRS.data_dir) {
        return Err(AppError::InvalidPath(relative.into()));
    }
    Ok(joined)
}

/// 列出目录内文件
pub fn list_files(relative: &str) -> AppResult<Vec<FileItem>> {
    let dir = safe_resolve(relative)?;
    if !dir.exists() {
        return Err(AppError::FileNotFound(relative.into()));
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(AppError::Io)? {
        let entry = entry.map_err(AppError::Io)?;
        let meta = entry.metadata().map_err(AppError::Io)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let path = entry.path().to_string_lossy().into_owned();
        out.push(FileItem {
            name,
            path,
            size: meta.len(),
            is_dir: meta.is_dir(),
            modified_at,
        });
    }
    Ok(out)
}

/// 读取文本文件
pub fn read_text_file(relative: &str) -> AppResult<String> {
    let path = safe_resolve(relative)?;
    if !path.exists() {
        return Err(AppError::FileNotFound(relative.into()));
    }
    Ok(std::fs::read_to_string(path).map_err(AppError::Io)?)
}

/// 写入文本文件（自动创建父目录）
pub fn write_text_file(relative: &str, content: &str) -> AppResult<()> {
    let path = safe_resolve(relative)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::Io)?;
    }
    std::fs::write(path, content).map_err(AppError::Io)?;
    Ok(())
}

/// 占位：读取配置文件的实际路径（供 config 服务调用）
pub fn config_file_path() -> &'static Path {
    // 泄漏为静态：整个生命周期只用一次
    let buf: &'static mut PathBuf = Box::leak(Box::new(APP_DIRS.config_dir.join("config.json")));
    buf.as_path()
}
