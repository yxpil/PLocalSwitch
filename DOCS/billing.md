# Billing（计费 + 对账）

> [← 返回文档中心](README.md) ｜ 上一站：[Cache Pool](cache-pool.md) ｜ 下一站：[Node Quality](node-quality.md)

模块位置：`src-tauri/src/billing/`

**双账本隔离**：上游真实采购成本 与 客户端计费 分开记账，互不污染，可对账。

---

## 1. 双账本

| 账本 | 写入时机 | 用途 |
|---|---|---|
| ① 上游真实成本账本 `upstream_ledger` | 每次 `SubAttempt` 都写（无论失败/成功/重试全部累加） | 内部采购成本核算 |
| ② 客户端计费账本 `client_ledger` | 仅按客户端请求的**最终结果**写 | 客户端扣费 |

客户端计费规则：
- 完全失败无输出 → `0`
- 非流式成功：用 A（上游 usage 优先）+ B（本地分词器）对账偏差后写
- 流式中途断开：按已输出 delta token 累加扣费
- 多次重试：**客户端只计 1 次**，绝不把内部重试开销叠加给用户

---

## 2. 子模块

| 子模块 | 职责 |
|---|---|
| `tokenizer_pool` | 外部分词器文件（bpe/vocab/merges）/ tiktoken 绑定，每模型独立实例 |
| `counter` | 三档优先级 token 计数：上游 usage → 本地分词 → 流式 delta 累加 |
| `audit` | 对账：A vs B 偏差率，超阈值告警，写 `audit_records` |
| `ledger` | 双账本写入 DB + 聚合 |
| `pricing` | 每模型采购价/售价、费率分组、client_key 余额/配额 |
| `client_key_mgr` | 网关自有 API Key（RPM/TPM/最大并发/余额/硬配额/透支开关） |

---

## 3. 对账（Audit）

`commit_audit_record(request_id, model, upstream_usage, local_usage, audit_cfg)` 计算 A（上游）与 B（本地分词）偏差率：

- 偏差率 > `audit.discrepancy_alarm_percent` → 告警（metrics + 写 audit_records）。
- `override_billing_when_discrepancy` / `override_prefer` 决定以谁为准（`upstream|local`）。

---

## 4. 相关文档

- [Observability](observability.md) — metrics 上报
- [Configuration](configuration.md) — `billing` 配置段
- [Frontend](frontend.md) — 账本/对账报表页面（Storage 页）
