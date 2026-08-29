# Node Quality（节点质量评估）

> [← 返回文档中心](README.md) ｜ 上一站：[Billing](billing.md) ｜ 下一站：[Observability](observability.md)

模块位置：`src-tauri/src/node_quality/`

给每个上游节点打 **0-100 的质量分**，作为路由的辅助权重（不强制阻断，仅降权/临时摘除）。用于多节点容灾、自动避开劣质上游。

---

## 1. 评估项

- 业务成功率（排除客户端 400 错误）
- P50 / P95 / P99 延迟、TTFT（首 token 时间）
- 429 / 5xx / SSE 断流 / JSON 解析失败 / 重试触发频次
- token 对账平均偏差率（越大越扣分）
- SSE 不合格、SSE 异常断开累计计数

---

## 2. 子模块

| 子模块 | 职责 |
|---|---|
| `scoring` | 综合各评估项计算 0-100 分 |
| `sample_buffer` | 环形定长样本缓冲（`DashMap<String, RingBuf<Sample>>`） |
| `label_classifier` | 按分数输出 优秀/良好/一般/较差/故障 5 档 |
| `autotrim` | 低质量自动降权 / 临时摘除（可在配置关闭） |

---

## 3. 样本阈值保护

`min_samples = 30`：样本不足不参与打分，**防止抖动误判**。只有积累到最小样本后才给分，避免刚上线的节点因偶发错误被误降权。

---

## 4. 后台打分任务

`spawn_quality_scoring_loop`：每 15 秒重新计算一次各节点质量分（当前为框架骨架，可在此补充真实计算逻辑）。

`quality_of(state, node_id)`：查询某节点最新质量分。

---

## 5. 与路由的关系

[Router](router.md) 使用 `quality` 字段：低分降权，低于阈值临时摘除（`autotrim`）。分数只影响路由选择，不作为硬阻断——即使质量一般，只要没有其他候选，仍会尝试。

---

## 6. 相关文档

- [Router](router.md) — 路由如何使用质量分
- [Observability](observability.md) — 采样数据来源
- [Configuration](configuration.md) — `node_quality` 配置段
