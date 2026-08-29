# PLocalSwitch 文档中心

> **PLocalSwitch** —— 本地部署的 LLM API 代理中转网关，对外 **100% 兼容 OpenAI v1 协议**，内置多厂商协议适配、双账本计费、节点质量评估、流式安全链路与桌面管理界面。

本目录是完整文档索引。所有文档之间通过链接互相跳转，建议从本页开始。

---

## 📚 快速导航

| 文档 | 说明 | 适合读者 |
|---|---|---|
| [Getting Started](getting-started.md) | 安装、编译、首次运行、填 key | 想要立刻跑起来 |
| [Architecture](architecture.md) | 总体架构 + 请求生命周期 | 想看整体设计 |
| [Configuration](configuration.md) | `gateway.yaml` 逐字段详解 | 要配节点/计费/缓存 |
| [Gateway API](gateway-api.md) | 对外 OpenAI v1 接口 & 管理接口 | 对接客户端 |
| [Router](router.md) | 模型别名 → 节点组路由 | 研究分流策略 |
| [Flex Adapter](flex-adapter.md) | 柔性适配核心（嗅探/重试/解析） | 想改协议适配 |
| [Backend Adapters](backend-adapters.md) | 13 类厂商协议适配器 | 接新上游 |
| [Cache Pool](cache-pool.md) | LRU-TTL 缓存池 | 调性能/省成本 |
| [Billing](billing.md) | 双账本计费 + 分词器对账 | 算钱/看报表 |
| [Node Quality](node-quality.md) | 节点质量打分与自动降权 | 多节点容灾 |
| [Observability](observability.md) | Trace / 脱敏 / Prometheus | 排障/监控 |
| [Safety Runtime](safety-runtime.md) | 并发/超时/连接池/背压/优雅关闭 | 高并发调优 |
| [Optional Components](optional-components.md) | Redis / LDAP / Webhook / 静态 Web | 进阶部署 |
| [Frontend](frontend.md) | 桌面 UI（B/W Pill 设计 + 插件化） | 前端同学 |

---

## 🧩 项目结构速览

```
PublicVersion/
├── src/                    # 前端（React + Vite + Tailwind）
│   ├── components/ui/      #   Pill 设计组件（按钮/卡片/输入框/开关...）
│   ├── pages/              #   业务页面（Home/Switch/Chat/Settings/Storage/About）
│   ├── plugins/            #   插件化注册（路由/侧边栏/设置Tab/Widget 注入）
│   ├── icons/              #   单色线性 SVG 图标库（Lucide 风格）
│   └── locales/lang/       #   多语言（zh/en/ja/... 9 种）
│
├── src-tauri/              # 后端（Rust + tokio + axum + reqwest）
│   ├── src/
│   │   ├── gateway_api/          # 1 对外 OpenAI v1 接口 + 管理接口
│   │   ├── router/               # 2 模型别名 → 节点组路由
│   │   ├── flex_adapter/         # 3 柔性适配层（核心）
│   │   ├── backend_adapters/     # 4 13 类厂商适配器
│   │   ├── cache_pool/           # 5 LRU-TTL 缓存池
│   │   ├── billing/              # 6 双账本计费 + 对账
│   │   ├── node_quality/         # 7 节点质量评估
│   │   ├── observability/        # 8 Trace + 脱敏 + Prometheus
│   │   ├── safety_runtime/       # 9 并发/超时/连接池/背压
│   │   ├── optional_components/  # 10 可选组件
│   │   └── config.rs             # AppConfig（读 gateway.yaml）
│   └── config/gateway.yaml      # 主配置
│
├── DOCS/                   # 本目录
└── ...                     # 构建/配置文件
```

---

## 🔒 安全设计原则（贯穿所有模块）

1. **对外永远只输出 OpenAI v1 协议**：`/v1/chat/completions`、`/v1/models`、`/v1/embeddings`。
2. **网关只做转发适配**，不做 LLM 推理、不实现 Agent、不持久化会话。
3. **流式请求一旦向客户端吐出过任意 chunk，立即锁死当前协议/节点**，禁止任何重试/回退/重新试探；仅完整非流式请求允许完整试探链路。
4. **敏感信息三重防护**：上游地址、Token 在日志/trace 中自动脱敏打码；对外 error 报文绝不透出上游地址/token/内部诊断。
5. **内存有硬上限**：队列、缓存、并发都有最大条目/字节限制。

---

## 🐛 反馈与贡献

- 反馈请到 [yxpil.com/feedback](https://yxpil.com/feedback)
- 想深入某个模块，直接用上面导航跳到对应文档。

---

# 下一站：[Getting Started →](getting-started.md)
