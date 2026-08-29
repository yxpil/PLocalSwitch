# Safety Runtime（运行时质量保障）

> [← 返回文档中心](README.md) ｜ 上一站：[Observability](observability.md) ｜ 下一站：[Optional Components](optional-components.md)

模块位置：`src-tauri/src/safety_runtime/`

网关自身的质量保障：高并发、内存可控、背压保护、优雅关闭。

---

## 1. 子模块

| 子模块 | 职责 |
|---|---|
| `connection_pool` | `reqwest` 独立连接池组（每 backend 分组独立池） |
| `semaphores` | 全局 + per_client_key 并发令牌桶 |
| `rate_limits` | 全局 / 每节点限流组骨架 |
| `timeouts` | 超时矩阵（连接/读取/流式） |
| `backpressure` | 429 快速失败（队列超上限即拒） |
| `shutdown` | 信号监听 + 优雅关闭 |

---

## 2. 约束实现

1. **全局并发上限 Semaphore**：`gateway_api` 层叠加 1 次，DB/上游请求再叠加（`http.global_concurrency_limit`、`per_client_key_concurrency_limit`）。
2. **每节点独立 reqwest Client**：连接池最大连接数 + 超时隔离（connect / read / stream_read）。
3. **请求体大小限制**：`gateway_api` 层已加，DB 查询分页兜底。
4. **客户端断开 → abort 上游流**：用 abort_handle 及时释放资源。
5. **缓存/样本/日志缓冲全部 max_entries 硬上限**。
6. **背压**：队列超上限直接 429，不无限排队。
7. **优雅关闭**：SIGINT/SIGTERM + 窗口关闭触发。

---

## 3. 网关生命周期控制

`GatewayCtrl`（在 `state` 中）维护运行标志与 shutdown 发送端：

- `is_running()`：当前是否运行。
- `register(tx)`：登记本次运行实例的 shutdown 发送端，并置 `running=true`。
- `request_stop()`：触发优雅停止，置 `running=false`。

桌面面板/托盘通过 IPC（`gateway_start` / `gateway_stop` / `restart_graceful`）控制启停，`spawn_axum_server` 统一负责拉起并登记状态。

---

## 4. 相关文档

- [Gateway API](gateway-api.md) — 挂载的 tower 层
- [Configuration](configuration.md) — `http` 并发/超时配置
- [Architecture](architecture.md) — 启动流程
