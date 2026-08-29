# Flex Adapter（柔性适配层 · 核心）

> [← 返回文档中心](README.md) ｜ 上一站：[Router](router.md) ｜ 下一站：[Backend Adapters](backend-adapters.md)

模块位置：`src-tauri/src/flex_adapter/`

这是整个网关的**核心模块**，负责把 OpenAI 请求灵活适配到任意上游，并处理容错、重试、协议嗅探、解析。

---

## 1. 子模块划分

| 子模块 | 职责 |
|---|---|
| `capability_cache` | 后台节点能力探测（toolcall / 多模态 / 窗口 / stream） |
| `param_adjust` | 参数改写（删不支持字段 / 截断 max_tokens / response_format 降级） |
| `protocol_sniffer` | 多候选协议嗅探（仅非流式）+ 成功记忆缓存 |
| `flexible_parser` | 宽容 Schema 解析（strict / flex 双模式，字段别名数组） |
| `retry_controller` | 多级试探回退控制（非流式 `MAX_ATTEMPTS` 硬上限） |
| `retry_analysis` | 故障研判 → `human_readable_reason` + 错误标签 |
| `upstream_fingerprint` | 响应头/错误栈指纹 → 上游语言/网关版本标签（仅日志统计） |

---

## 2. 总原则（⚠️ 安全红线）

> **总原则 4**：流式只要向客户端吐出过任意 chunk，立即锁死当前协议/节点，禁止重试/试探/回退。

- 流式：一次选定节点/协议，中途任何解析失败只能结束流，**不得切节点**。
- 非流式：允许在候选节点间进行完整的试探链路（受 `global_max_sub_attempts` 硬上限）。

---

## 3. 非流式执行 `execute_non_stream`

```
1. apply_capability_constraints(state, candidates, &mut req)  // 能力约束改写
2. attempt_chain(state, trace, candidates, req, stream=false)  // 依次尝试
   - 每个候选节点按 candidate_protocols 顺序尝试
   - 每次尝试产生一个 SubAttempt
   - 失败 → retry_analysis 写 human_readable_reason
3. 返回结果：Ok(resp) 或 AllFailed(...)
```

硬上限：`flex_adapter.global_max_sub_attempts`、每个节点 `sniff_attempts_per_node`。

---

## 4. 流式执行 `execute_stream`

- 用 `async_stream::try_stream!` 把上游 SSE 逐步翻译为 `SseChunk` 后 yield。
- `data: [DONE]` 结束本次流（不 yield）。
- **流式错误脱敏**：建连/发送/读流失败都只给“标签级”安全消息（完整错误仅写 tracing 日志），绝不含上游 URL。
- **流式 usage 回填**：从各 chunk 的 usage 累计 `prompt/completion`，结束时写回 trace（解决 trace 里 output=0 的问题）。

---

## 5. 协议嗅探与宽容解析

- **嗅探**：某个节点协议不确定时，非流式按 `candidate_protocols` 顺序尝试，成功后**记忆缓存**（`sniff_remember_ttl_seconds`），后续直接走缓存协议。
- **宽容解析**：上游返回与标准 Schema 不一致时，走 `flex` 模板匹配（字段别名数组、缺失容忍）。

---

## 6. 相关文档

- [Backend Adapters](backend-adapters.md) — 实际翻译请求的适配器
- [Observability](observability.md) — SubAttempt / trace 记录
- [Safety Runtime](safety-runtime.md) — 并发/超时对流式的影响
- [Configuration](configuration.md) — `flex_adapter` 配置段
