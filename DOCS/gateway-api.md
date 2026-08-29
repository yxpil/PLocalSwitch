# Gateway API（对外接口层）

> [← 返回文档中心](README.md) ｜ 上一站：[Architecture](architecture.md) ｜ 下一站：[Router](router.md)

模块位置：`src-tauri/src/gateway_api/`

对外 HTTP 入口，用 axum 组装 Router。职责是**把外部请求接进来**、校验、限流、分发到路由层，并把结果翻译回客户端。

---

## 1. 路由总览

| 路径 | 说明 | handler |
|---|---|---|
| `POST /v1/chat/completions` | OpenAI 聊天补全（流式/非流式） | `openai_routes` |
| `GET  /v1/models` | 列出可用模型 | `openai_routes` |
| `POST /v1/embeddings` | 向量化（预留） | `openai_routes` |
| `/v1/messages`、`/anthropic/...`、`/gemini/...` | 入站协议路由（Anthropic/Gemini 客户端也能接） | `inbound_routes` |
| `/manage/...` | 内部管理接口（账单/trace/节点/对账） | `manage_routes` |
| `/metrics` | Prometheus 拉取 | (tower-http) |
| `/admin/...` | 生命周期（启停/health） | `manage_routes` |

---

## 2. 子模块

| 子模块 | 职责 |
|---|---|
| `auth` | 网关自有 API Key 校验；RPM/TPM、余额、并发的前置检查 |
| `rate_limit` | RPM/TPM 限流（dashmap + 滑动窗口） |
| `openai_routes` | `/v1/chat/completions`、`/v1/models`、`/v1/embeddings` 处理 |
| `inbound_sniffer` | 入站协议嗅探 + 归一化/反归一化（OpenAI/Anthropic/Gemini） |
| `inbound_routes` | 入站协议路由（`/v1/messages`、`/anthropic/...`、`/gemini/...`） |
| `manage_routes` | 管理接口（账单/Trace/节点/对账/单条 sub_attempt、生命周期） |
| `sse_utils` | SSE chunk 归一化（流式序列化 + `data:` 前缀） |
| `error_resp` | 统一错误映射为 OpenAI 标准错误（严格不泄露上游） |

---

## 3. 全局 tower 层

在 `mod.rs` 组装 Router 时挂载：

- **CORS**：按 `cors` 配置；`allow_credentials=true` 时禁止 `*`，改用 origins 白名单。
- **请求体大小限制**：`RequestBodyLimitLayer`（`http.request_body_max_bytes`）。
- **请求超时**：`TimeoutLayer`。
- **并发限流**：`ConcurrencyLimitLayer`（`http.global_concurrency_limit`）。
- **RequestId**：每个请求生成 trace 关联 ID。
- **catch_panic**：handler panic 不拖垮服务。
- **SensitiveHeaders**：`Authorization` / `x-api-key` / `x-api-token` 等自动脱敏。

---

## 4. 流式 SSE 输出

- 后端把上游 SSE 逐块翻译成标准 `SseChunk`，`sse_utils` 序列化为 `data: {...}\n\n`，末尾补 `data: [DONE]`。
- 流式一旦向客户端输出过任意 chunk，`flex_adapter` **禁止切换节点/协议**。
- 流内错误也会脱敏成“标签级”消息输出，绝不含上游 URL/token。

---

## 5. 错误响应

所有错误经 `error_resp.rs` 的 `AppErrorResponse` 统一输出，格式：

```json
{
  "error": {
    "message": "All upstream endpoints unavailable.",
    "type": "gateway_error",
    "code": "network_connect_refused"
  }
}
```

状态码与错误体**严格一致**：
- `400` 参数错误 / `401` 鉴权失败 / `429` 限流
- `502` 上游 5xx / `503` 建连/DNS/TLS 失败 / `504` 连接/读超时
- 其余 `500`

**安全红线**：`message` 只含“标签级”安全文案，绝不透出上游地址、token、内部诊断。

---

## 6. 相关文档

- [Architecture](architecture.md) — 请求生命周期
- [Router](router.md) — 路由层
- [Flex Adapter](flex-adapter.md) — 流式/非流式适配核心
- [Configuration](configuration.md) — 关于 http/cors/masking 的配置项
