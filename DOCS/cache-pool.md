# Cache Pool（缓存池）

> [← 返回文档中心](README.md) ｜ 上一站：[Backend Adapters](backend-adapters.md) ｜ 下一站：[Billing](billing.md)

模块位置：`src-tauri/src/cache_pool/`

独立的 LRU-TTL 缓存池，用于缓存重复请求（相同模型 + 相同输入），减少上游调用、降低成本。

---

## 1. 子模块

| 子模块 | 职责 |
|---|---|
| `cache_key` | Key 构造：`xxhash(model + messages text + temperature + top_p + ...)` |
| `backend` | `CacheBackend` trait + `InMemory` 实现 + Redis stub |
| `cache_entry` | 缓存条目：流式 `Vec<u8>` 字节序列；非流式 `ChatCompletionResponse` |
| `metrics` | 命中/未命中/淘汰/内存占用的 Prometheus 指标 |

---

## 2. 设计要点

1. **双实例**：非流式 / 流式各自独立缓存；**tool_call 默认跳过缓存**（避免缓存了带工具调用的请求）。
2. **内存双上限**：mini-moka 同时限制**最大条目数** + **预估内存占用**，防止无限膨胀。
3. **Key 构造**：对 `model + messages 文本 + temperature + top_p + ...` 做 xxhash。
4. **每模型独立 TTL 开关** + **命中是否对客户端收费开关**（`charge_on_cache_hit`）。
5. **后台 reclaim 协程**：定时 evict，不做请求路径重淘汰。
6. **预留 `CacheBackend` trait**：之后可无缝替换成 Redis 实现（见 [Optional Components](optional-components.md)）。

---

## 3. 命中流程

- 非流式命中 → 直接返回 `(entry, billing_treatment)`，不串下游。
- `billing_treatment` 决定命中时是否对客户端收费（默认 `charge_on_cache_hit=false` 不收费）。

---

## 4. 相关文档

- [Billing](billing.md) — 缓存命中计费策略
- [Optional Components](optional-components.md) — Redis 缓存替换
- [Configuration](configuration.md) — `cache_pool` 配置段
