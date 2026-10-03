# PLocalSwitch 测试说明

Rust 端是 Tauri + axum 的本地 LLM API 代理/中转网关，crate 位于 `src-tauri/`
（lib 名 `plocalswitch_lib`）。测试全部为 lib 内 `#[cfg(test)]`（纯函数，不依赖真实数据库/上游）。

## 运行命令

```powershell
cd src-tauri
cargo test --lib
```

> **前置**：与 COMMIX 相同，`tauri::generate_context!()` 编译期要求 `../dist` 存在。
> 本仓库不提交前端产物（`dist/` 在 .gitignore）。本机跑测试前在仓库根建一个最小
> `dist/index.html` 占位即可，不会被 git 跟踪。sqlx 全部用运行时查询（无 `query!` 宏），
> 不需要 DATABASE_URL。

## 覆盖清单

### 既有测试（6 个，未改动）
- `gateway_api/inbound_sniffer.rs`（4）：入站协议形态嗅探（OpenAI/Anthropic/Gemini）归一化与反归一化。
- `observability/masking.rs`（2）：token 打码、endpoint 主机保留。

### 本次新增（9 个）
**计费（`billing/pricing.rs`，4）**
- `exact_match_takes_priority_over_glob`：精确模型名优先于 `gpt-4*` 通配。
- `glob_prefix_matches`：`gpt-3.5*` 前缀命中 `gpt-3.5-turbo`，不命中 `gpt-4`。
- `calc_per_million_math`：按每百万 token 单价正确计算输入/输出/总额。
- `calc_unknown_rate_is_zero`：未知模型费率为 0（不报错、不计费）。

**脱敏/注入防护（`observability/masking.rs`，5 新增，叠加既有 2 个）**
- `nested_sensitive_body_fields_are_redacted`：嵌套 JSON 中 `api_key`/`token` 被替换为 `****`，
  非敏感字段原样保留（防止密钥写进日志）。
- `sensitive_headers_always_flagged`：`Authorization`/`X-API-Key`/`Cookie` 恒为敏感头，
  `Content-Type` 不是。
- `endpoint_strips_embedded_credentials`：URL 中 `https://admin:pass@host/...` 的 userinfo
  用户名密码在脱敏输出中被剥离，不泄漏。
- `disabled_masking_passes_through`：关闭脱敏时原样透传。

## 注入测试

本仓库是网关，不可信输入面是**上游请求体/URL/Header**。新增测试验证：
- 密钥类字段（无论嵌套多深）在日志/审计链路被打码；
- URL 中注入的 `user:pass@` 凭据不进入脱敏后的可见字符串；
- 敏感 Header 一律标记。
这构成“用户数据→日志”路径的泄露防护断言。

## 钩子 / 事件

网关的“嗅探→归一化→路由”链路由既有 `inbound_sniffer` 测试覆盖：
Anthropic 请求能被归一化为 OpenAI 形态并正确反归一化往返，Gemini 形态优先识别。
这些跨函数往返测试保证协议转换不会丢字段（任一方向丢字段即断言失败）。

## 预期结果

`cargo test --lib` → **14 passed; 0 failed**。
