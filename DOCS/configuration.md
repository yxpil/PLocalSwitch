# 配置详解（gateway.yaml）

> [← 返回文档中心](README.md) ｜ 上一站：[Frontend](frontend.md) ｜ 下一站：[Getting Started](getting-started.md)

配置文件：`src-tauri/config/gateway.yaml`，用 serde_yaml 解析。

**加载优先级**（`config.rs::load_or_default`）：
1. 环境变量 `PLS_GATEWAY_CONFIG` 指定的路径
2. `./config/gateway.yaml`
3. 内置默认值（编译进二进制，保证能启动）

所有配置在运行时通过 `ArcSwap` 支持热更新，也可在桌面「设置」页读写。

---

## 1. 顶层结构

```yaml
version: '1.0'
app:      # 应用元信息
http:     # 监听/并发/超时/代理
db:       # 数据库
metrics:  # Prometheus
cors:     # 跨域
model_aliases: []   # 模型别名映射
node_groups: []     # 上游节点组
flex_adapter: {}    # 柔性适配配置
cache_pool: {}      # 缓存池
billing: {}         # 计费
node_quality: {}    # 节点质量
policy: {}          # 重试策略
masking: {}         # 脱敏
```

---

## 2. `app` — 应用元信息

```yaml
app:
  name: PLocalSwitch-Gateway
  env: dev                  # dev / prod
  timezone: Asia/Shanghai
  log_level: info           # debug / info / warn
  privacy:
    store_payload_text: false   # 是否存请求正文文本（默认不存）
    masking: true               # 是否启用脱敏
    mask_token_head_tail: [4, 4]  # token 头尾保留字符数
    mask_url_path_segment_limit: 2
```

---

## 3. `http` — 监听/并发/超时/代理

```yaml
http:
  listen: 127.0.0.1:8787        # 对外 OpenAI v1 服务
  admin_listen: 127.0.0.1:9631  # 管理/生命周期接口
  request_body_max_bytes: 4194304   # 请求体上限 4MB
  global_concurrency_limit: 512     # 全局并发上限
  per_client_key_concurrency_limit: 64
  client_disconnect_aborts_upstream: true
  timeouts:
    connect_ms: 3000           # 建连 3s
    read_ms: 60000             # 读 60s
    stream_read_ms: 600000     # 流式 600s
  proxy_enabled: false         # 是否走上游代理
  proxy: http://127.0.0.1:7890 # HTTP(S) 代理
  proxy_socks: null            # SOCKS5 代理（proxy 为空时兜底）
  proxy_no_proxy: []           # 不走代理的主机列表
```

> ⚠️ 代理坑：`proxy_enabled: true` 但代理(如 7890)未运行时，会**全线 503**。务必先确认代理在线再开启。

---

## 4. `db` — 数据库

```yaml
db:
  backend: sqlite              # sqlite / postgres
  sqlite_path: ./data/pls_gateway.db
  postgres_url: ''             # postgres 连接串
  pool_max_open: 32
  pool_max_idle: 8
  migrate_on_start: true
```

---

## 5. `metrics` / `cors`

```yaml
metrics:
  enabled: true
  expose_at: /metrics
  process_collector: true
  per_client_key_labels: true
  per_node_labels: true
  per_error_label: true

cors:
  allow_origins: ['*']
  allow_methods: [GET, POST, OPTIONS, DELETE, PUT]
  allow_headers: ['*']
  allow_credentials: false   # 为 true 时禁止 '*'
```

---

## 6. `model_aliases` — 模型别名映射

把客户端请求的 `alias` 映射到 `real_model` 并指定 `group`：

```yaml
model_aliases:
- alias: deepseek-chat
  real_model: deepseek-chat
  group: group-deepseek
  cache_enable: false
  ttl_seconds: null           # 缓存 TTL（cache_enable 时生效）
  charge_on_cache_hit: false  # 缓存命中是否收费
  enabled: true
```

---

## 7. `node_groups` — 上游节点组

```yaml
node_groups:
- id: group-deepseek
  description: DeepSeek Official
  load_balance: round_robin    # round_robin / weighted_random / primary_standby
  enabled: true
  nodes:
  - id: deepseek-official
    endpoint: https://api.deepseek.com
    api_keys:
    - <YOUR_API_KEY>        # ← 在这里填真实 Key！
    protocol_hints:           # 协议嗅探候选顺序
    - openai
    enabled: true
    weight: 1.0
    hard_disable: false
    connect_pool: null
    timeouts_override: null
    primary: null              # primary_standby 时指定主节点
    aws_region: null           # Bedrock 用
    ak: null                   # 部分国内厂商用 AK/SK
    sk: null
```

`protocol_hints` 取值对应 [Backend Adapters](backend-adapters.md) 的协议：`openai` / `openai_response` / `anthropic` / `gemini` / `ollama` / `bedrock_converse` / `cohere_v2` / ...

> 🔐 **安全**：这里的 `api_keys`、`ak`、`sk` 是真实密钥。公开发布版已替换为 `<YOUR_...>` 占位符，请在当地配置文件填真实值，**切勿提交到公开仓库**。

---

## 8. `flex_adapter` — 柔性适配

```yaml
flex_adapter:
  sniff_attempts_per_node: 2      # 每节点协议嗅探次数
  global_max_sub_attempts: 6      # 非流式全局最大尝试次数（硬上限）
  sniff_remember_ttl_seconds: 86400  # 嗅探结果记忆
  flexible_parse_alert_on_fallback: true
  stream_lock_after_first_byte: true  # 流式吐过首字节即锁协议/节点
  capability:
    probe_interval_seconds: 600
    probe_prompt: Say hi.
    probe_priority_nodes_only: true
```

---

## 9. `cache_pool` — 缓存池

```yaml
cache_pool:
  implementation: in_memory        # in_memory / redis
  in_memory:
    max_entries_non_stream: 8192
    max_entries_stream: 4096
    max_total_memory_mb: 256       # 内存双上限（字节）
    evict_interval_seconds: 30
    hash_key_algo: xxh3_128
  redis:
    url: redis://127.0.0.1:6379
    username: ''
    password: ''
    db: 0
    default_ttl_seconds: 86400
```

---

## 10. `billing` — 计费

```yaml
billing:
  currency: CNY
  rates:                      # 每模型：采购价 / 售价（每百万 token）
  - model: deepseek-chat
    upstream_cost_per_m_input: 0.0
    upstream_cost_per_m_output: 0.0
    client_price_per_m_input: 0.5
    client_price_per_m_output: 1.5
  client_keys: []             # 网关自有 API Key（RPM/TPM/余额/配额），由前端生成
  audit:
    discrepancy_alarm_percent: 15.0
    override_billing_when_discrepancy: false
    override_prefer: local        # upstream | local
  tokenizers:                 # 分词器绑定
  - model_glob: gpt-*
    provider: tiktoken
    tiktoken_encoding: o200k_base
```

---

## 11. `node_quality` — 节点质量

```yaml
node_quality:
  min_samples: 30               # 最小样本数（不达标不评分）
  scoring_weights:              # 各评估项权重
    success_rate: 0.35
    latency_p99: 0.2
    ttft: 0.1
    error_counts: 0.2
    token_discrepancy: 0.05
    sse_abnormal_rate: 0.1
  labels:
    excellent: 90..100
    good: 75..89
    normal: 60..74
    poor: 40..59
    fault: 0..39
  autotrim:
    enabled: true
    temporary_ban_seconds_when_fault: 300
    demote_weight_when_poor: 0.5
```

---

## 12. `policy` — 重试策略

```yaml
policy:
  retry_on:                      # 哪些错误才允许重试
    network_connect_refused: true
    dns_fail: true
    connect_timeout: true
    read_timeout: false
    tls_error: true
    http_429: true
    http_5xx: true
    auth_401_403: false
    bad_param_4xx: false
    sse_premature_close: false   # 流式断连不重试（红线）
    json_parse_fail: true
  analysis_history_window_seconds: 3600
```

---

## 13. `masking` — 脱敏

```yaml
masking:
  enabled: true
  sensitive_headers: [authorization, x-api-key, cookie, ...]
  sensitive_body_fields: [api_key, key, secret, token, ...]
  token_show_head: 4             # token 显示头尾
  token_show_tail: 4
  url_preserve_path_segments: 2
```

---

## 14. 相关文档

- [Getting Started](getting-started.md) — 快速开始
- [Backend Adapters](backend-adapters.md) — protocol_hints 协议说明
- [Architecture](architecture.md) — 配置如何驱动启动
