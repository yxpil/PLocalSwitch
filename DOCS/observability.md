# Observability（可观测性）

> [← 返回文档中心](README.md) ｜ 上一站：[Node Quality](node-quality.md) ｜ 下一站：[Safety Runtime](safety-runtime.md)

模块位置：`src-tauri/src/observability/`

负责整个网关的可观测性：Trace 全局 ID、SubAttempt 链路、敏感字段脱敏、Prometheus 指标。

---

## 1. 核心概念

- **trace_id**：每个客户端请求生成唯一的全局 ID（uuid v7，时间有序）。
- **sub_attempt_id**：每次上游尝试产生的子 ID（ulid），与 trace_id 关联。
- **脱敏**：所有敏感字段（endpoint、token）入库前必须经 `masking` 模块打码。
- **human_readable_reason**：故障研判文案，仅对内，禁止在对外错误响应中透传。

---

## 2. 子模块

| 子模块 | 职责 |
|---|---|
| `masking` | 脱敏工具：地址/Token 打码（`mask_token` / `mask_endpoint`） |
| `trace` | `GatewayTrace` / `SubAttempt` 核心结构体 |
| `fingerprint` | 上游指纹识别记录 |
| `audit_record` | 对账记录 |
| `metrics_registry` | Prometheus 全部指标（error_label 分桶） |

---

## 3. Trace 生命周期

每个请求流程中：
1. 入口生成 `trace_id`。
2. 每次尝试生成 `sub_attempt_id`，记录 `http_status_code`、usage、耗时。
3. 结束时 `trace.close(status, label)` 并落库（`trace_store`）。
4. 所有敏感字段经 `masking` 后才写入 trace/日志。

**流式 usage 回填**：流式各 chunk 的 usage 会累计写回 trace（修复过「trace 里 output=0」的问题）。

---

## 4. 脱敏规则（安全红线）

- `mask_token`：token 只显示头尾若干字符（`mask_token_head_tail`，默认 4/4）。
- `mask_endpoint`：对地址打码，`mask_url_path_segment_limit` 控制路径段。
- 对外错误报文**绝不透出**上游地址、token、内部诊断，只给“标签级”安全消息。

---

## 5. Prometheus 指标

`/metrics` 暴露，`metrics_registry` 维护所有指标，按 `error_label` 分桶。后台 `spawn_metrics_flush_loop` 按配置 flush。

---

## 6. 相关文档

- [Architecture](architecture.md) — 请求生命周期
- [Flex Adapter](flex-adapter.md) — SubAttempt 产生处
- [Billing](billing.md) — 对账记录
- [Frontend](frontend.md) — trace 查询页面（About/Traces 页）
