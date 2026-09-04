//! =============================================================
//!  commands::tray —— 托盘自绘网页菜单动作
//!  网页菜单点击「显示主窗口 / 反馈 / 退出」时经 IPC 回调到这里的命令；
//!  「暂停/继续/重启网关」复用 commands::gateway 的 gateway_* 命令。
//! =============================================================
#![cfg(feature = "desktop-shell")]
use crate::state::AppState;
use std::sync::Arc;

/// 托盘菜单动作命令（带副作用：显示主窗口 / 打开反馈 / 退出应用）
#[tauri::command]
pub fn tray_action(app: tauri::AppHandle, action: String) -> Result<(), String> {
    use tauri::Manager;

    match action.as_str() {
        "show" => {
            // 恢复主窗口并聚焦
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }
        "feedback" => crate::open_url("https://yxpil.com/feedback"),
        "quit" => {
            // 先优雅停止网关再退出
            let st = (*app.state::<Arc<AppState>>()).clone();
            let _ = st.gateway_ctrl.request_stop();
            app.exit(0);
        }
        _ => {}
    }

    // 除「退出」外，点击后关闭托盘菜单小窗
    if action != "quit" {
        if let Some(win) = app.get_webview_window("tray-popup") {
            let _ = win.hide();
        }
    }
    Ok(())
}
