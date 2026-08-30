//! ================================================================
//!  PLocalSwitch —— LLM API 代理中转站（对外 100% 兼容 OpenAI v1）
//! ================================================================
//!
//! 启动流程（同时兼顾桌面壳 & 纯网关服务器两种发布方式）：
//!   1. 打印启动横幅（ASCII + 引用 logo.png 路径）
//!   2. 加载 config/gateway.yaml  →  AppConfig（ArcSwap 热更新预留）
//!   3. 初始化 sqlite / postgres 连接池（双账本、trace、对账）
//!   4. 初始化 safety_runtime：并发上限 Semaphore、reqwest 独立连接池组、限流 RateLimit
//!   5. 启动 cache_pool 后台异步淘汰任务
//!   6. 启动 capability_cache 后台节点探测任务
//!   7. 启动 node_quality 后台打分任务
//!   8. 启动 gateway_api axum 服务（OpenAI v1 + 管理接口 + prometheus /metrics）
//!   9. （仅非 gateway-server feature）启动 Tauri 桌面窗口 + 托盘：管理 UI
//!
//! 所有模块解耦，均以独立 pub mod 暴露；以后每个模块单独往对应文件填代码即可。
// ----------------------------------------------------------------

// ====== 10 大核心模块（对应用户交付物 1：目录结构）======
pub mod gateway_api;         // 1. 对外 OpenAI v1 兼容 axum 接口
pub mod router;              // 2. 模型别名映射 + 主备/权重路由
pub mod flex_adapter;        // 3. 柔性适配层（能力缓存/参数改写/协议嗅探/宽容解析/重试控制器/研判/指纹）
pub mod backend_adapters;    // 4. 厂商硬编码适配器（13 类协议）
pub mod cache_pool;          // 5. LRU-TTL 独立缓存池（mini-moka，内存双上限）
pub mod billing;             // 6. 双账本计费 + 分词器对账
pub mod node_quality;        // 7. 节点质量评估（0-100 分，样本阈值保护）
pub mod observability;       // 8. trace + sub_attempt + metrics registry
pub mod safety_runtime;      // 9. 并发上限 / 超时隔离 / 连接池 / 背压 / 优雅关闭
pub mod optional_components; // 10. 可选：Redis 缓存替换 / WebUI 静态资源

// ====== 已有辅助模块（从 PLocalSwitch 保留并扩展）======
#[cfg(feature = "desktop-shell")]
pub mod commands;
pub mod config;              // 新增：从 gateway.yaml 读取 AppConfig
#[cfg(feature = "desktop-shell")]
pub mod services;            // 桌面壳业务服务（文件存储 + 配置，IPC 命令层复用）
pub mod error;
pub mod logging;
pub mod models;              // 重写：OpenAI v1 协议 / 厂商协议通用数据模型
pub mod state;               // 全局托管：ArcSwap<AppConfig> / 连接池 / DB / 并发令牌桶 / Prometheus

use crate::error::AppResult;
use std::sync::Arc;
#[cfg(feature = "desktop-shell")]
use tauri::Manager;

/// 启动横幅（ASCII B/W，输出 logo.png 绝对路径）
fn print_startup_banner() {
    use std::time::{SystemTime, UNIX_EPOCH};
    const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
    let pkg = env!("CARGO_PKG_NAME");
    let ver = env!("CARGO_PKG_VERSION");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| format!("epoch {}s", d.as_secs()))
        .unwrap_or_else(|_| "unknown".to_string());
    let mut logo = std::path::PathBuf::from(MANIFEST_DIR);
    logo.pop();
    logo.push("logo.png");
    let logo_ok = logo.is_file();

    println!(
        "\n\
        ╔════════════════════════════════════════════════════════════════════════╗\n\
        ║  ██████╗ ██╗     ███████╗ ██████╗ █████╗ ██╗      ██████╗              ║\n\
        ║  ██╔══██╗██║     ██╔════╝██╔════╝██╔══██╗██║     ██╔════╝              ║\n\
        ║  ██████╔╝██║     █████╗  ██║     ███████║██║     ██║                    ║\n\
        ║  ██╔═══╝ ██║     ██╔══╝  ██║     ██╔══██║██║     ██║                    ║\n\
        ║  ██║     ███████╗███████╗╚██████╗██║  ██║███████╗╚██████╗               ║\n\
        ║  ╚═╝     ╚══════╝╚══════╝ ╚═════╝╚═╝  ╚═╝╚══════╝ ╚═════╝   v{:<8}║\n\
        ╠════════════════════════════════════════════════════════════════════════╣\n\
        ║  🧭 LLM Gateway  100% OpenAI v1 compatible, 13 upstream adapters      ║\n\
        ║  🚀 Runtime       : tokio-multi-thread + axum + reqwest(pooled)       ║\n\
        ║  🧾 Billing       : dual-ledger (client / upstream) + tiktoken audit  ║\n\
        ║  🧩 Flex Adapter  : protocol sniffer + retry + SSE normalize          ║\n\
        ╠════════════════════════════════════════════════════════════════════════╣\n\
        ║  startup : {:<57}║\n\
        ║  logo    : {:<57.57}║\n\
        ║  logo-ok : {:<57}║\n\
        ╚════════════════════════════════════════════════════════════════════════╝\n",
        ver, now, logo.display(),
        if logo_ok { "YES (logo.png attached)" } else { "NO - missing" },
    );
    eprintln!("[PLS>logo] app-logo-signature -> {} (exists={logo_ok})", logo.display());
    eprintln!("[PLS>boot] {pkg} v{ver} — LLM API Proxy Gateway starting up…");
}

/// 应用启动入口（同时支持 Tauri 壳 / 纯 axum 服务器）
pub fn run_app() -> AppResult<()> {
    print_startup_banner();

    // 使用单一 tokio 多线程 runtime，Tauri 和 axum 共用
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(4 * 1024 * 1024)
        .worker_threads(std::cmp::max(4, num_cpus().get()))
        .build()?;

    rt.block_on(async move {
        // ----- 1. 加载配置 -----
        let cfg = Arc::new(config::AppConfig::load_or_default().await?);
        tracing::info!("gateway config loaded (http.listen={}, db={})", cfg.http.listen, cfg.db.backend);

        // ----- 2. 初始化状态（DB / 连接池组 / 并发 sem / 缓存 / 分词器 / 指标）-----
        // 上游代理：读自配置（由设置页写入），地区受限网络下走 http://127.0.0.1:7890 等，否则某些模型不可达
        crate::backend_adapters::apply_upstream_proxy(
            cfg.http.proxy_enabled,
            cfg.http.proxy.clone(),
            cfg.http.proxy_socks.clone(),
            cfg.http.proxy_no_proxy.clone(),
        );
        let app_state = Arc::new(state::AppState::bootstrap(cfg.clone()).await?);

        // 追踪/账本建表（幂等，缺表则补充）
        services::trace_store::init_schema(&app_state).await;

        // 预填“模型→上游组”目录（别名真实模型），供真实模型名路由；list_upstream_models 会再补充上游真实模型
        for a in app_state.cfg.model_aliases.iter() {
            if a.enabled {
                app_state.node_runtime.model_catalog.insert(a.real_model.clone(), vec![a.group.clone()]);
            }
        }

        // ----- 3. 启动后台任务（能力探测 / 质量打分 / 缓存淘汰 / 指标上报）-----
        flex_adapter::capability_cache::spawn_probe_loop(app_state.clone()).await;
        node_quality::spawn_quality_scoring_loop(app_state.clone()).await;
        cache_pool::spawn_reclaim_loop(app_state.clone()).await;
        observability::spawn_metrics_flush_loop(app_state.clone()).await;

        // ----- 4. 启动 axum 网关服务（OpenAI v1 + 管理接口 + prometheus）-----
        safety_runtime::spawn_axum_server(app_state.clone(), app_state.gateway_ctrl.clone()).await?;

        // 启动后异步刷新「模型→上游组」目录（拉取各上游 /v1/models），
        // 保证真实模型名（如 aisingapore/sea-lion-7b-instruct）开箱即可路由，无需先打开聊天页
        {
            let st = app_state.clone();
            tokio::spawn(async move {
                let n = crate::router::refresh_model_catalog(&st).await.len();
                tracing::info!("model catalog refreshed at startup ({n} entries)");
            });
        }

        // ----- 5. Tauri 桌面壳启动（纯服务器模式下被 feature=desktop-shell 条件跳过）-----
        #[cfg(feature = "desktop-shell")]
        {
            tracing::info!("Starting Tauri desktop management shell…");
            let tauri_app = tauri::Builder::default()
                // 单实例：防止多开（第二个实例启动时唤醒已存在的窗口后自行退出）
                .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.unminimize();
                        let _ = win.set_focus();
                    }
                }))
                .plugin(tauri_plugin_shell::init())
                .plugin(tauri_plugin_dialog::init())
                .plugin(tauri_plugin_fs::init())
                .plugin(tauri_plugin_notification::init())
                .manage(Arc::clone(&app_state))
                .setup(|app| {
                    tracing::info!("Tauri app context ready");
                    // 把 AppHandle 存入 AppState，供托盘菜单 HTTP 控制“显示主窗口/退出/打开反馈”
                    if let Ok(mut guard) = (*app.state::<Arc<crate::state::AppState>>()).app_handle.lock() {
                        *guard = Some(app.handle().clone());
                    }
                    // 系统托盘：原生右键菜单（稳定可靠，不依赖网页/IPC）。
                    // 优化：顶部动态显示网关状态；启停合并成单个动态项（运行=暂停网关/停止=继续网关）。
                    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
                    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

                    let app_state0 = (*app.state::<Arc<crate::state::AppState>>()).clone();
                    let running0 = app_state0.gateway_ctrl.is_running();

                    let mi_status = MenuItem::with_id(app, "status", if running0 { "网关：运行中" } else { "网关：已停止" }, false, None::<&str>)?;
                    let mi_show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
                    let mi_fb = MenuItem::with_id(app, "feedback", "反馈", true, None::<&str>)?;
                    let mi_toggle = MenuItem::with_id(app, "gateway_toggle", if running0 { "暂停网关" } else { "继续网关" }, true, None::<&str>)?;
                    let mi_quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
                    let s1 = PredefinedMenuItem::separator(app)?;
                    let s2 = PredefinedMenuItem::separator(app)?;
                    let s3 = PredefinedMenuItem::separator(app)?;
                    let menu = Menu::with_items(app, &[&mi_status, &s1, &mi_show, &mi_fb, &s2, &mi_toggle, &s3, &mi_quit])?;

                    let mi_status_c = mi_status.clone();
                    let mi_toggle_c = mi_toggle.clone();

                    TrayIconBuilder::new()
                        .icon(app.default_window_icon().cloned().unwrap())
                        .menu(&menu)
                        .show_menu_on_left_click(false)
                        .on_menu_event(move |app, event| {
                            let st = app.state::<Arc<crate::state::AppState>>();
                            let app_state = Arc::clone(&*st);
                            match event.id.as_ref() {
                                "show" => {
                                    if let Some(win) = app.get_webview_window("main") {
                                        let _ = win.show();
                                        let _ = win.unminimize();
                                        let _ = win.set_focus();
                                    }
                                }
                                "feedback" => {
                                    let _ = std::process::Command::new("cmd")
                                        .args(["/C", "start", "", "https://yxpil.com/feedback"])
                                        .spawn();
                                }
                                "gateway_toggle" => {
                                    if app_state.gateway_ctrl.is_running() {
                                        app_state.gateway_ctrl.request_stop();
                                        let _ = mi_toggle_c.set_text("继续网关");
                                        let _ = mi_status_c.set_text("网关：已停止");
                                    } else {
                                        let ctrl = app_state.gateway_ctrl.clone();
                                        let st2 = app_state.clone();
                                        tokio::spawn(async move {
                                            let _ = crate::safety_runtime::spawn_axum_server(st2.clone(), ctrl).await;
                                        });
                                        let _ = mi_toggle_c.set_text("暂停网关");
                                        let _ = mi_status_c.set_text("网关：运行中");
                                    }
                                }
                                "quit" => {
                                    app_state.gateway_ctrl.request_stop();
                                    app.exit(0);
                                }
                                _ => {}
                            }
                        })
                        .on_tray_icon_event(|tray, event| {
                            if let TrayIconEvent::Click { button, button_state, .. } = event {
                                if button_state == MouseButtonState::Up && button == MouseButton::Left {
                                    let app = tray.app_handle();
                                    if let Some(win) = app.get_webview_window("main") {
                                        let _ = win.show();
                                        let _ = win.unminimize();
                                        let _ = win.set_focus();
                                    }
                                }
                            }
                        })
                        .build(app)?;

                    if let Some(win) = app.get_webview_window("main") {
                        tracing::info!(window = %win.label(), "main window ready");
                    }
                    Ok(())
                })
                .invoke_handler(tauri::generate_handler![
                    commands::system::get_app_info,
                    commands::system::ping,
                    commands::system::get_system_info,
                    commands::config::load_config,
                    commands::config::save_config,
                    commands::config::reset_config,
                    commands::config::get_proxy_settings,
                    commands::config::set_proxy_settings,
                    commands::storage::list_files,
                    commands::storage::read_text_file,
                    commands::storage::write_text_file,
                    commands::gateway::gateway_status,
                    commands::gateway::gateway_start,
                    commands::gateway::gateway_stop,
                    commands::gateway::restart_graceful,
                    commands::gateway::gateway_auto_restart,
                    commands::tray::tray_action,
                    commands::system::list_traces,
                    commands::system::export_traces_excel,
                    commands::system::billing_summary,
                    commands::system::gateway_chat,
                    commands::system::gateway_chat_stream,
                    commands::system::list_upstream_models,
                    commands::system::fetch_upstream_models,
                    commands::system::test_node,
                ])
                .on_window_event(|window, event| {
                    // 点击关闭按钮 -> 隐藏到托盘，不退出进程
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let _ = window.hide();
                        api.prevent_close();
                    }
                })
                .build(tauri::generate_context!())?;
            tauri_app.run(|_h, _e| {
                // 关闭窗口时不再退出；仅托盘"退出"才真正结束
            });
            // 托盘退出时优雅停止网关
            app_state.gateway_ctrl.request_stop();
        }

        // 纯网关服务器（默认）：阻塞等待终止信号
        #[cfg(not(feature = "desktop-shell"))]
        safety_runtime::wait_for_termination().await;

        Ok(())
    })
}

/// 给代码中 `num_cpus::get()` 提供编译通路（在 num_cpus 未被引用时避免未使用警告）
fn num_cpus() -> CpuCount { CpuCount }
struct CpuCount;
impl CpuCount { fn get(&self) -> usize { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) } }
