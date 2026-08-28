//! 日志初始化模块

use anyhow::Result;
use std::sync::OnceLock;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// 全局日志 Guard（防止 guard 被 drop 导致日志停止写入）
static LOG_GUARD: OnceLock<Option<tracing_appender::non_blocking::WorkerGuard>> = OnceLock::new();

/// 初始化 tracing 日志系统
/// - Debug 模式：输出到控制台，级别 DEBUG
/// - Release 模式：输出到控制台 + 文件，级别 INFO
pub fn init_tracing() -> Option<&'static tracing_appender::non_blocking::WorkerGuard> {
    use tracing_appender::rolling::{RollingFileAppender, Rotation};

    // 环境变量过滤器：默认 Debug=debug，Release=info
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            EnvFilter::new("debug,plocalswitch_lib=debug,tauri=warn,winit=warn")
        } else {
            EnvFilter::new("info,plocalswitch_lib=info,tauri=warn")
        }
    });

    // 注册控制台层
    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_line_number(cfg!(debug_assertions))
        .with_thread_ids(cfg!(debug_assertions));

    // 文件层：只在 Release 或显式环境变量下开启
    let (file_layer, guard) = if std::env::var("LOG_TO_FILE").is_ok() || !cfg!(debug_assertions) {
        let log_dir = app_log_dir();
        let _ = std::fs::create_dir_all(&log_dir);
        let file_appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("plocalswitch")
            .filename_suffix("log")
            .build(log_dir)
            .ok();
        if let Some(appender) = file_appender {
            let (non_blocking, guard) = tracing_appender::non_blocking(appender);
            let layer = fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .boxed();
            (Some(layer), Some(guard))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    // 注册全局 guard
    let _ = LOG_GUARD.set(guard);

    // 组装 subscriber
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer.boxed())
        .with(file_layer)
        .init();

    LOG_GUARD.get().and_then(|g| g.as_ref())
}

/// 获取日志目录（跨平台）
fn app_log_dir() -> std::path::PathBuf {
    if let Some(proj) = directories::ProjectDirs::from("com", "plocalswitch", "PLocalSwitch") {
        proj.data_local_dir().join("logs")
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("logs")
    }
}

/// 供 lib.rs run_app 调用的 Result 版本（保持 API 整洁）
#[allow(dead_code)]
pub fn try_init_tracing() -> Result<()> {
    init_tracing();
    Ok(())
}
