# PLocalSwitch LLM Gateway 风险点清单（交付物 6）

> 所有风险对应 10 大模块 + gateway_api 层，记录风险现象 / 触发条件 / 防御代码位置 / 监控指标 / 人工处置。
> 如未特殊说明，代码默认已实现"硬上限 + 只告警不覆盖计费"策略，仅需关注红色告警。

| 编号 | 模块 | 风险现象 | 触发条件 | 防御实现 | Prometheus 指标 | 人工处置 |
|------|------|----------|----------|----------|-----------------|----------|
| R01  | safety_runtime/concurrency | 全局/PK 并发信号量耗尽，客户端持续 429 | 上游大模型集体拥塞 + 客户端 QPS 超配置 2 倍 | ConcurrencyLimitLayer + state.semaphores.per_key 双道；超上限直接 429 无排队 | pls_requests_total{status_code="429"} | 临时扩容节点组 / 调大 per_client_key 并发 / 联系网关管理员 |
| R02  | cache_pool | 缓存雪崩：大量热点 key 同时过期触发上游 stampede | 同一 TTL + 同一批热门 prompt 批量到 TTL | TTL 1 ± 10% jitter + min_moka weighted evict；写入时 jitter 化 | pls_cache_miss_total{model} + 节点 P99 TTFT 飙升 | 手动重放 cache rebuild / 对热点模型单独延长 TTL |
| R03  | billing/audit | A vs B token 偏差率 > 阈值，但 policy.override_billing_when_discrepancy=true 导致错扣 | 上游改协议 / 外部分词器 vocab 更新滞后 | 默认 override_billing_when_discrepancy=false，**只告警不覆盖计费**；alarm=true 写 audit_records | pls_audit_discrepancy_alarm_total | 人工核对 1-2 条 trace 后更新 tokenizer binding |
| R04  | billing/ledger | 流式中途断开但客户端被按完整 prompt+max_tokens 扣费 | 上游 SSE 中断 + delta_usage 累加没落地 | 原则 ②：流式断开只按已输出 delta token 计费，绝不按 max_tokens 估算 | pls_sub_attempts_total{outcome="stream_aborted"} + ledger 明细核对 | 审计后人工退款 |
| R05  | node_quality | 抖动误判：新节点样本 < min_samples 却被当故障摘除 | autotrim 开且样本量少时 | sample_sufficient=false 时 **不参与打分**，路由权重强制回退配置 weight | pls_quality_score_samples_total | 关闭 autotrim 或手动发起 probe 热身 |
| R06  | flex_adapter/retry | 多次重试内部 SubAttempt 消耗成本被计入客户端账本 | 账本写串（A→②） | 账本隔离：commit_upstream 与 commit_client 独立通道；客户端永不因重试叠加 | pls_ledger_charge_cny_total + pls_ledger_upstream_cost_cny_total 双账本差额监控 | |
| R07  | flex_adapter/stream_lock | 流式首 chunk 输出后，上游 SSE 断流但客户端没有收到 [DONE]（原则 4 违反） | 开发者误在 stream 的 consumer 内 catch 后切节点重试 | execute_stream 返回 stream 后，**写入 state.stream_locked 标志**；一旦 attempt 中 detect_first_byte 后任何 error 只 close stream，禁止 route_next() | pls_sub_attempts_total{error_label="sse_mid_drop",outcome="failed_terminal"} 计数 > 0 → 立即告警 | 检查具体 protocol adapter 实现是否在 stream consumer 中做了 retry |
| R08  | observability/masking | 明文 token/endpoint 意外落盘（日志/DB/WebUI 回传） | 开发新增字段时忘调 masking::*；IPC 序列化未脱敏 | 原则：明文仅存内存；AppError::serialize 自定义走 to_openai_error；审计 SubAttempt.masked_endpoint/masked_token 非空校验；DB migration 加 CHECK | 代码审计 + 日志文件 grep 真实 API Key 前 8 位抽查 | 立即回滚提交 + 通知所有受影响节点轮换密钥 |
| R09  | safety_runtime/connection_pool | 某节点组 reqwest 连接池耗尽导致整体阻塞 | 单节点 pool 配置过小 + 长连接超时断不开 | per_node 独立 build；max_idle_per_host + pool_idle_timeout_ms 双配置；DashMap 隔离 | reqwest 内部 metric + pls_request_duration_seconds 分组看 per node 分位 | 调大 connect_pool.max_idle_per_host / 节点横向扩容 |
| R10 | gateway_api/manage_routes | admin 接口未鉴权导致账单/trace 全量泄露 | 纯服务器部署直接暴露 /manage | 鉴权骨架：Authorization 匹配 app.admin_token；未配置时 bind 到 127.0.0.1 仅本地 | 手动审计 + 运维 sidecar WAF | admin token 强随机；生产建议 front with oauth2-proxy |
| R11 | safety_runtime/db | sqlite/postgres 慢查询阻塞请求路径 | billing 聚合大时间范围查询无分页 + cache | DB 操作全部在 manage_routes 路径执行，**不在/v1 请求同步路径做任何写入**（写入走 tokio::spawn 异步通道） | sqlx query duration metric + 慢查询日志 | 增加 DB 索引 / migrate / 切 Postgres 分区表 |
| R12 | safety_runtime/shutdown | 优雅关闭超时：in-flight 请求没写完账本即进程退出 | shutdown_timeout < 长流式请求 P99 | shutdown 触发后先停止 accept，再等待超时后强制 kill；等待期间 drain SubAttempt 写 DB 账本 | pls_shutdown_duration_seconds | 调整 shutdown_timeout_ms；极端情况保留进程内 ring buffer 残留 trace 下次启动 flush |
| R13 | backend_adapters (Custom) | 魔改 OpenAI 兼容网关返回特殊字段，flexible_parser 宽容模式仍失败 | 厂商返回 schema 完全变形 / sse 无 data 前缀 | protocol_sniffer 嗅探失败后记录 fingerprint_labels=["custom","malformed_sse"] 并告警，不 panic 不影响其他请求 | pls_sub_attempts_total{error_label="schema_mismatch"} → 分 node_id 维度，>10% 立即告警 | 新增 backend_adapter 的针对性字段别名 |
| R14 | cache_pool/backend | 缓存内存超过 max_total_memory_mb 硬上限导致 OOM | response 超大（多模态长文）+ weigher 计算不准 | mini-moka max_weight 双半分（non-stream / stream 各半）；size_bytes 字段再 × 1.2 安全系数 | 后台 reclaim 打印 estimated_mem_bytes；启动时 env MALLOC_TRIM_THRESHOLD 开启 | 紧急清空缓存 POST /manage/cache/purge |
| R15 | node_quality/autotrim | 故障节点被 temp_ban 后，当所有节点同时故障 → 无候选可用 | autotrim + 全局上游故障 | 临时 ban 时保留"至少 1 个最不坏"节点不 ban：if candidates.len() ≤ 1 跳过 apply；失败再按 policy 顺序 | pls_nodes_available 指标（可用节点数）= 0 → P0 告警 | 关闭 autotrim 或立刻降级回 round_robin 不管质量 |
| R16 | optional_components/webhook_notify | 告警飞书/钉钉接口本身失败导致主路径阻塞 | 账单超阈值时同步调用 webhook | webhook 调用必须在 tokio::spawn 独立任务，**不可阻塞 /v1 请求** | pls_webhook_fail_total | 检查 webhook url 可用性 |
| R17 | safety_runtime/rate_limit | RPM / TPM 限流未生效导致超上游采购配额 | billing.client_keys.rpm 配置未生效 / state.rate_limits map 初始化缺 key | route_client_request 前 verify + per_key rate_limit.checked_add；超限直接 429 + 明细 Reason（**不泄露上游信息**） | pls_requests_total{status_code="429",reason="rpm" | tpm"} | 调大 RPM / TPM；按天硬配额超限先下线高用量 client key |
| R18 | billing/client_key | client_key 明文 gateway.yaml 落盘（违反严禁明文密钥落盘？） | gateway.yaml 存储 ClientKey.key 本身 **本是网关发行 key，不是上游密钥** | 上游密钥（UpstreamNode.api_keys）需通过 PLS_UPSTREAM_NODE_X_KEY 环境变量注入（保留字段，本实现当前仍走 yaml；下一版本迁 env 注入） | 代码审计：UpstreamNode::api_keys 字段入库前一律 mask_token | 生产环境上游密钥从 vault / env 注入；ClientKey 本身发行 key 视为网关配置敏感项但非"上游明文密钥"类 |

## 风险缓解总则（与 11 条核心原则对应）

1. **永不泄露**：客户端 /manage / audit / trace / metrics 所有输出必须走 masking 层（R08）。
2. **流式锁死**：只要 write 过 1 个 SSE 字节，**不允许再重试、切节点、换协议**（R07）。
3. **内存硬上限**：缓存、样本环、日志 ring、连接池、队列全部可配置条目数上限（R02/R14）。
4. **双账本**：客户端计费账本绝不因内部重试累加（R06）。
5. **样本不足保护**：质量分 sample_count < min_samples → sample_sufficient=false，不参与打分不自动摘除（R05）。
6. **对账只告警不覆盖**：默认 override_billing_when_discrepancy=false（R03）。
7. **写入不在请求路径**：所有 DB 写入 / Prometheus 大聚合 / webhook，必须走 tokio::spawn 或 channel，/v1 同步路径只做纯 CPU + reqwest（R11/R16）。
