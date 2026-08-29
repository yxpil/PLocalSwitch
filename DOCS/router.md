# Router（路由层）

> [← 返回文档中心](README.md) ｜ 上一站：[Gateway API](gateway-api.md) ｜ 下一站：[Flex Adapter](flex-adapter.md)

模块位置：`src-tauri/src/router/`

输入：客户端请求的模型**别名**（如 `gpt-4o-mini`）。
输出：按路由策略排序的**候选节点列表** `[CandidateNode]`。

---

## 1. 参与要素

| 要素 | 来源 | 作用 |
|---|---|---|
| `cfg.model_aliases` | 配置 | 别名 → 真实模型 & 目标分组名 |
| `cfg.node_groups` | 配置 | 节点组，含若干 upstream node（权重/主备） |
| `node_quality` 得分 | [node_quality](node-quality.md) | 低分降权、低于阈值自动摘除 |
| `cache key` 命中 | [cache_pool](cache-pool.md) | 命中时可跳过下游请求 |

---

## 2. 子模块

| 子模块 | 职责 |
|---|---|
| `model_alias` | 别名映射（`gpt-4o-mini` → 真实模型 + 分组） |
| `group_selector` | 在节点组内选择节点（轮询/权重/主备/按质量分） |
| `fallback_policy` | 降级策略（主节点失败后的备选顺序） |

---

## 3. CandidateNode（候选节点）

路由解析后的上游候选，核心结构：

```rust
pub struct CandidateNode {
    pub node_id:      String,
    pub group_id:     String,
    pub real_model:   String,
    pub endpoint:     String,          // 内部使用，落盘前必须脱敏
    pub protocol:     ProtocolKind,    // 硬编码适配器走哪条
    pub candidate_protocols: Vec<ProtocolKind>, // 非流式嗅探时的顺序
    pub weight:       f64,
    pub quality:      u8,              // 0..=100（来自 node_quality）
    pub api_key_name: String,          // 脱敏 key 前缀
    #[doc(hidden)] pub _api_key: String, // 明文，仅请求过程内存中存在，严禁落盘
}
```

**安全要点**：`endpoint`、`_api_key` 等敏感字段在落盘/trace 前必须经 [observability/masking](observability.md) 脱敏。

---

## 4. 路由策略

- **权重路由**：`weight` 越高选中概率越大。
- **主备**：`primary` 节点优先，失败走 `fallback_policy` 备选。
- **质量路由**：结合 `node_quality` 实时得分——低分降权，低于阈值临时摘除（可配置关闭）。
- **负载均衡**：分组可配 `round_robin`。

节点组内选择逻辑在 `group_selector`，降级顺序在 `fallback_policy`。

---

## 5. 相关文档

- [Architecture](architecture.md) — 请求生命周期
- [Backend Adapters](backend-adapters.md) — protocol 决定的适配器
- [Node Quality](node-quality.md) — 路由用到的质量分
- [Configuration](configuration.md) — `model_aliases` / `node_groups` 配置
