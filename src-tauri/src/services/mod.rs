//! 业务服务层：为 IPC 命令提供实现

pub mod storage;      // 本地文件存储
pub mod config;       // 配置管理
pub mod trace_store;  // 链路追踪 + 双账本持久化
pub mod error_logger; // 错误日志记录工具（转发链路失败统一落库）
