//! IPC 命令模块集合（按领域拆分）
#![cfg(feature = "desktop-shell")]
// NOTE: 整个 IPC 模块仅在编译桌面壳 feature=desktop-shell 时构建；
//       纯 axum 网关二进制不依赖 tauri crate。
pub mod system;     // 系统信息、心跳
pub mod config;     // 配置读写（保留原 Tauri IPC）
pub mod storage;    // 文件存储
pub mod gateway;    // ✅ 新增：桌面管理 UI 通过 IPC 访问网关运行状态 / 优雅重启
pub mod tray;       // ✅ 新增：托盘自绘网页菜单动作（显示主窗口 / 反馈 / 退出）
