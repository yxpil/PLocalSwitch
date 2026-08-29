# PLocalSwitch · 本地模型代理中转站

> **PLocalSwitch** —— 本地部署的 LLM API 代理网关，对外 **100% 兼容 OpenAI v1 协议**。内置 13 类厂商协议适配、双账本计费、节点质量评估、流式安全链路，并附赠一个 Tauri 桌面管理界面。

<p>
  <a href="#核心特性">✨ 核心特性</a> ·
  <a href="#快速开始">🚀 快速开始</a> ·
  <a href="./DOCS/README.md">📚 完整文档</a>
</p>

---

## ✨ 核心特性

- **对外只输出 OpenAI v1**：`/v1/chat/completions`、`/v1/models`、`/v1/embeddings`，客户端零改动接入。
- **13 类上游适配器**：OpenAI / Anthropic / Gemini / Responses / AWS Bedrock / Cohere / 百度千帆 / 阿里 DashScope / 讯飞星火 / 腾讯混元 / Ollama / vLLM / TGI，以及任意 OpenAI 兼容中转。
- **Tools 双向完整转发**：Anthropic `tool_use`、Gemini `functionCall`、Responses `function_call`、Ollama `tool_calls` 全链路还原，模型工具调用不卡住。
- **双账本计费**：上游真实采购成本 与 客户端计费 隔离，支持 tiktoken 分词对账。
- **节点质量评估**：0-100 打分、样本阈值保护、低质量自动降权/摘除。
- **柔性适配层**：协议嗅探 + 宽容解析 + 重试控制 + 故障研判。
- **流式安全**：流式吐过首字节即锁死节点/协议，禁止重试/回退；错误统一脱敏不外泄。
- **黑白药丸设计 UI**：Tauri 2 桌面管理壳 + React 18 + Tailwind，纯黑白灰单色系。
- **可观测**：Trace / SubAttempt 链路、Prometheus 指标、敏感字段自动打码。

---

## 🧩 模块一览

| 模块 | 说明 |
|---|---|
| [Gateway API](DOCS/gateway-api.md) | 对外 OpenAI v1 接口 + 管理接口 + /metrics |
| [Router](DOCS/router.md) | 模型别名 → 节点组路由（权重/主备/质量） |
| [Flex Adapter](DOCS/flex-adapter.md) | 柔性适配核心（嗅探/重试/解析/研判） |
| [Backend Adapters](DOCS/backend-adapters.md) | 13 类厂商协议适配器 |
| [Cache Pool](DOCS/cache-pool.md) | LRU-TTL 缓存池（内存双上限） |
| [Billing](DOCS/billing.md) | 双账本计费 + 分词器对账 |
| [Node Quality](DOCS/node-quality.md) | 节点质量打分与自动降权 |
| [Observability](DOCS/observability.md) | Trace / 脱敏 / Prometheus |
| [Safety Runtime](DOCS/safety-runtime.md) | 并发/超时/连接池/背压/优雅关闭 |
| [Frontend](DOCS/frontend.md) | 桌面 UI（黑白药丸设计 + 插件化） |

完整文档目录见 **[DOCS/README.md](DOCS/README.md)**。

---

## 🚀 快速开始

```bash
# 1. 依赖
npm install
cd src-tauri && cargo build

# 2. 填上游 Key
#    编辑 src-tauri/config/gateway.yaml → node_groups[].nodes[].api_keys（<YOUR_...> 占位符）

# 3. 启动（桌面版，带管理 UI）
cargo run
```

网关监听 `127.0.0.1:8787`（OpenAI v1），管理端口 `127.0.0.1:9631`。

```bash
curl http://127.0.0.1:8787/v1/models -H "Authorization: Bearer <client_key>"
```

> 🔐 **安全提示**：公开版配置中的 Key 均为 `<YOUR_...>` 占位符。请勿将含真实密钥的配置提交到公开仓库；真实配置建议放本地 `gateway.local.yaml`（已被 `.gitignore` 忽略）或经 `PLS_GATEWAY_CONFIG` 指向外部路径。

详细步骤见 **[Getting Started](DOCS/getting-started.md)**。

---

## 📚 文档

- [文档中心 / 索引](DOCS/README.md)
- [架构总览](DOCS/architecture.md)
- [配置详解](DOCS/configuration.md)
- [Gateway API](DOCS/gateway-api.md)
- [Frontend 设计](DOCS/frontend.md)

---

## 🐛 反馈

反馈请到 [yxpil.com/feedback](https://yxpil.com/feedback) 。
