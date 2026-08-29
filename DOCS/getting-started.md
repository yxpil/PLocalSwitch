# Getting Started（快速开始）

> [← 返回文档中心](README.md) ｜ 上一站：[Configuration](configuration.md)

本页带你从零跑起 PLocalSwitch。

---

## 1. 环境要求

- **Rust**：1.76+
- **Node.js**：18+（仅构建前端需要；纯网关服务器无需前端）
- **数据库**：默认 SQLite（零配置）；可选 PostgreSQL
- **Tauri 前置**（桌面版）：Windows 需 WebView2；macOS/Linux 按 Tauri 文档配置

---

## 2. 克隆与安装依赖

```bash
git clone https://github.com/yxpil/PLocalSwitch.git
cd PLocalSwitch

# 前端依赖
npm install

# 后端（Rust）依赖 —— 首次会拉取较多 crate
cd src-tauri
cargo build
```

---

## 3. 配置账号（填 Key）

编辑 `src-tauri/config/gateway.yaml`，在 `node_groups[].nodes[].api_keys` 里填你的真实上游 Key：

```yaml
node_groups:
- id: group-deepseek
  nodes:
  - id: deepseek-official
    endpoint: https://api.deepseek.com
    api_keys:
    - sk-你的DeepSeekKey       # ① 填上真实 Key
    protocol_hints: [openai]
```

> 🔐 **安全**：公开版本已将 Key 替换为 `<YOUR_...>` 占位符。填完请**不要把含真实 Key 的配置文件提交到公开仓库**——建议把真实配置放到本地 `gateway.local.yaml`（已在 `.gitignore` 忽略）或用环境变量 `PLS_GATEWAY_CONFIG` 指向外部路径。

---

## 4. 网关独有 API Key（客户端访问用）

网关自身需要一个 Client Key，客户端请求时通过 `Authorization: Bearer <client_key>` 鉴权。

- 在桌面「设置 → 通用/网关配置」里添加 Client Key；
- 或直接在 `billing.client_keys` 段配置。

---

## 5. 启动

### 方式 A：桌面版（推荐，带管理 UI）

```bash
cd src-tauri
cargo run
```

启动后：网关监听 `127.0.0.1:8787`（OpenAI v1），管理端口 `127.0.0.1:9631`，同时弹出桌面管理窗口。

### 方式 B：纯网关服务器（无桌面）

```bash
cd src-tauri
cargo run --no-default-features --features gateway-server
```

---

## 6. 验证

```bash
# 列出模型
curl http://127.0.0.1:8787/v1/models \
  -H "Authorization: Bearer <你的client_key>"

# 发起对话
curl -X POST http://127.0.0.1:8787/v1/chat/completions \
  -H "Authorization: Bearer <你的client_key>" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "deepseek-chat",
    "messages": [{"role": "user", "content": "你好"}]
  }'
```

期望返回 OpenAI 标准格式的 `choices[].message.content`。

---

## 7. 用前端管理

启动桌面版后，在「网关总览」可看到状态、启停网关；「模型与路由」配置别名/节点/Key；「账本与对账」看计费；「链路追踪」查 trace。

---

## 8. 常见问题

| 问题 | 原因 & 解决 |
|---|---|
| 服务请求全部 503 | 检查 `proxy_enabled`；若为 true 但代理(7890)未运行会全线失败，先关掉或启动代理 |
| 模型返回 tool_calls 但客户端卡住 | 确认走的是支持 tools 转发的适配器（Anthropic/Gemini/Responses/Ollama 均已支持） |
| trace 里 output_tokens=0 | 新版已累计回填流式 usage，请确认用的是新构建 |

---

## 9. 相关文档

- [Architecture](architecture.md) — 整体设计
- [Configuration](configuration.md) — 配置详解
- [Gateway API](gateway-api.md) — 对外接口
