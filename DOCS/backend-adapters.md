# Backend Adapters（厂商适配器）

> [← 返回文档中心](README.md) ｜ 上一站：[Flex Adapter](flex-adapter.md) ｜ 下一站：[Cache Pool](cache-pool.md)

模块位置：`src-tauri/src/backend_adapters/`

每个适配器负责：**把 OpenAI 请求翻译成某家上游的格式，再把上游响应/SSE 翻译回 OpenAI v1**。

---

## 1. 支持的协议（13 类）

| 协议类型 | 适配器文件 | 说明 |
|---|---|---|
| OpenAI | `openai_adapter.rs` | 整体透传（多数 OpenAI 兼容上游） |
| OpenAI Responses | `responses_adapter.rs` | grok 等 uses Responses API |
| Anthropic | `anthropic_adapter.rs` | Claude Messages API（含 tool_use） |
| Gemini | `gemini_adapter.rs` | Google generateContent（含 functionCall） |
| Bedrock Converse | `bedrock_adapter.rs` | AWS Bedrock |
| Cohere v2 | `cohere_v2_adapter.rs` | Cohere Chat / Completions |
| Qianfan | `qianfan_adapter.rs` | 百度千帆 |
| DashScope | `dashscope_adapter.rs` | 阿里通义（兼容模式） |
| Spark | `spark_adapter.rs` | 讯飞星火 |
| Hunyuan | `hunyuan_adapter.rs` | 腾讯混元 |
| Ollama | `ollama_adapter.rs` | 本地 Ollama（放宽证书校验） |
| vLLM | `vllm_adapter.rs` | 本地 vLLM |
| TGI | `tgi_adapter.rs` | HuggingFace TGI |

另有 `custom_openai_compat_adapter.rs` 处理任意 OpenAI 兼容中转。

---

## 2. 统一 `BackendAdapter` trait

每个适配器实现：

| 方法 | 职责 |
|---|---|
| `protocol()` | 返回自身协议类型 |
| `translate_request(&oai, &node)` | 构建指向上游的 `reqwest::RequestBuilder`（含鉴权 + JSON body） |
| `parse_response_body(&bytes)` | 解析上游非流式 JSON → 标准 `ChatCompletionResponse` |
| `translate_sse_chunk(&vendor_line)` | 逐行翻译上游 SSE → `Option<SseChunk>` |

`adapter_for(kind)` 按协议派发实例；`http_client()` 返回共享的连接池 client。

---

## 3. 请求侧翻译要点

- **OpenAI**：`translate_request` 整体用 `.json(oai)` 透传（`model` 已在路由层替换为真实名）。
- **Anthropic**：把 OpenAI `messages/tools` 转成 Anthropic `content blocks / tools`；同角色消息合并，避免 400；`tool_use` 块正确还原为 OpenAI `tool_calls`；支持 base64 与 http(s) URL 图片。
- **Gemini**：`tools` → `functionDeclarations` + `toolConfig`；`tool` 消息 → `functionResponse`；图片 → `inlineData`。
- **Responses**：`tools` 扁平 `function` 形；assistant tool_calls → `function_call` 项；`tool` 消息 → `function_call_output`。
- **Ollama**：`tools` 透传；图片 → `images[]`；`tool_calls` 双向（arguments 对象/字符串兼容）。

---

## 4. 响应侧翻译要点

每个适配器都做了 **tools / tool_use 双向还原**，保证「模型要求调用工具 → 客户端能拿到 tool_calls → 客户端回传 tool 结果 → 模型继续」这条链路完整。缺失会导致客户端拿到 `finish_reason=tool_calls` 却无 tool_calls 而**卡住**（本项目重点修复过的问题）。

流式方面各适配器还会：
- 把上游 `thinking` 增量透传为 `reasoning_content`（DeepSeek 风格扩展字段）。
- 处理 Anthropic `message_start`/`message_delta` 的 usage，**累计回填 prompt/completion**。
- 上游 SSE `error` 事件不再静默吞掉，转为网关错误。

---

## 5. HTTP 客户端

- 基础 client：`reqwest` + rustls，`connect_timeout=10s`、`read_timeout=120s`。
- 上游代理：`apply_upstream_proxy(enabled, http, socks, no_proxy)` 支持运行时切换并整体重建连接池（走代理时对地区受限上游更友好）。
- Ollama 单独 client：放宽证书校验（自签名）。

---

## 6. 相关文档

- [Flex Adapter](flex-adapter.md) — 调用这些适配器的上层逻辑
- [Router](router.md) — ProtocolKind 决定用哪个适配器
- [Configuration](configuration.md) — `node_groups[].protocol_hints` 配置
