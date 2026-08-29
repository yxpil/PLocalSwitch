# Optional Components（可选组件）

> [← 返回文档中心](README.md) ｜ 上一站：[Safety Runtime](safety-runtime.md) ｜ 下一站：[Frontend](frontend.md)

模块位置：`src-tauri/src/optional_components/`

默认不启用，通过 feature 或 trait 显式加载的高级选项，用于企业级/进阶部署。

---

## 1. 组件清单

| 组件 | 用途 |
|---|---|
| `redis_cache` | 替换 `cache_pool::backend::InMemoryBackend`，用 Redis 做分布式缓存 |
| `webui_static` | axum `ServeDir` 直接提供 React `dist/` 静态页面（纯网关服务器场景，无需独立前端） |
| `ldap_auth` | 企业内网：管理端 LDAP/SSO 登录 |
| `webhook_notify` | 账单超阈值 / SSE 断流频发 → 飞书/钉钉/webhook 告警 |

---

## 2. 何时启用

- **多实例部署 / 想共享缓存**：用 `redis_cache` 替换内存缓存。
- **只想部署一个网关二进制、不要单独前端**：用 `webui_static` 让网关自己托管前端页面。
- **企业内部统一账号体系**：用 `ldap_auth` 接入 LDAP/SSO。
- **需要主动告警**：用 `webhook_notify` 把异常推送到 IM 群。

---

## 3. 与核心模块的关系

- `redis_cache` 实现 `cache_pool::backend::CacheBackend` trait（见 [Cache Pool](cache-pool.md)）。
- `webui_static` 复用 [Frontend](frontend.md) 构建出的 `dist/`。
- 通过 Cargo feature 开关加载，默认关闭，不增加核心二进制体积。

---

## 4. 相关文档

- [Cache Pool](cache-pool.md) — redis_cache 替换目标
- [Safety Runtime](safety-runtime.md) — 并发保障
- [Configuration](configuration.md) — 相关配置段
