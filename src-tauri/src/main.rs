//! PLocalSwitch - 主二进制入口
//! 职责：初始化日志 → 初始化 Tauri 应用 → 注册命令 → 启动运行时

#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use plocalswitch_lib::run_app;
use std::process::ExitCode;

fn main() -> ExitCode {
    // 初始化日志系统
    let _guard = plocalswitch_lib::logging::init_tracing();

    tracing::info!("PLocalSwitch 启动中...");
    tracing::info!("版本: {}", env!("CARGO_PKG_VERSION"));
    tracing::debug!("运行模式: {}", if cfg!(debug_assertions) { "Debug" } else { "Release" });

    // 运行 Tauri 应用
    match run_app() {
        Ok(()) => {
            tracing::info!("PLocalSwitch 正常退出");
            ExitCode::SUCCESS
        }
        Err(err) => {
            tracing::error!(error = %err, "PLocalSwitch 异常退出");
            eprintln!("应用启动失败: {err:#}");
            ExitCode::FAILURE
        }
    }
}
