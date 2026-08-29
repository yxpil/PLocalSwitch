# Frontend（桌面管理界面）

> [← 返回文档中心](README.md) ｜ 上一站：[Optional Components](optional-components.md) ｜ 下一站：[Configuration](configuration.md)

前端目录：`src/`（React + Vite + TypeScript + Tailwind），配合 Tauri 2 桌面壳运行。

---

## 1. 技术栈

- **框架**：React 18 + TypeScript 5.5
- **构建**：Vite 5 + `@vitejs/plugin-react`
- **路由**：react-router-dom（HashRouter，适合桌面）
- **状态**：zustand（主题 / 应用状态）
- **样式**：Tailwind CSS 3（自定义 `theme.extend`，黑白药丸设计）
- **多语言**：i18next + react-i18next（9 种语言）
- **图标**：自绘单色线性 SVG 图标库（~41 个 Lucide 风格）

---

## 2. 目录结构

```
src/
├── components/
│   ├── layout/       # AppShell / Sidebar / Topbar（无边框窗口框架）
│   └── ui/           # Pill 组件（Button/Card/Input/Switch/Tabs/Badge/Select/Modal/Toast）
├── pages/            # Home / Switch / Chat / Settings / Storage / About / NotFound...
│   ├── settings/     #   General/Network/Modules/Gateway/Look/About
│   └── widgets/      #   HeroWidget / StatsWidget（Dashboard 槽位）
├── plugins/          # 插件化注册（路由/侧边栏/设置Tab/Widget 注入）
├── router/           # 路由表（从插件 getRoutes 组装）
├── icons/            # 单色线性 SVG 图标
├── locales/lang/     # 多语言 json
├── stores/           # zustand store（theme / app）
└── styles/input.css  # Tailwind 入口
```

---

## 3. 设计语言：B/W Pill Design（黑白药丸）

- **零彩色**：整站只有黑、白、灰三档。亮色 = 白底黑字；暗色（`.dark`）= 黑底白字。
- **全圆角药丸**：按钮/输入框/开关/角标用 `9999px` 胶囊；卡片用 `1.5rem`。
- **语义只用灰阶表达**：成功=深灰底白字、警告=中灰、失败=纯黑反白、信息=浅灰，不引入红绿蓝。
- **黑白柔光阴影**：`boxShadow.pill/card` 全为黑/白透明度投影。
- **动效**：默认 220ms、药丸 280ms，缓动 `cubic-bezier(0.4,0,0.2,1)`，悬停上浮/阴影加深。
- **字体**：`Inter` / PingFang SC / Microsoft YaHei；数字等宽 `JetBrains Mono`。

设计令牌与工具类集中在 `tailwind.config.js`（含 `.pill-*`、`.pill-variant-*` 插件），可整包挪到别的项目复用。

---

## 4. 插件化架构

核心路由/侧边栏/设置 Tab/Home Widget 全部由 [`plugins`](src/plugins) 注册注入 `registerPlugin(...)`：

```
registerPlugin({
  name, version,
  routes:  [...],   // 业务页面路由
  sidebar: [...],   // 左侧导航项
  widgets: [...],   // Home 槽位组件
  settings: [...],  // 设置页 Tab
});
```

好处：新增/禁用功能只改插件注册，不侵入核心路由配置；每个模块有 `SafeRender` 容错包裹，单个插件异常不影响整体。

---

## 5. 无边框窗口

- 自绘标题栏（`AppShell`）：可拖动 + 最小化/最大化/关闭嵌在网页内（右上角胶囊圆钮）。
- 窗口移动/缩放钳制在当前显示器范围内，防止拖出屏幕。
- 侧边栏导航 + 顶栏面包屑 + 主内容区（`max-w-[1400px]` 居中，卡片流）。

---

## 6. 主要页面

| 页面 | 路径 | 功能 |
|---|---|---|
| Home 网关总览 | `/` | 网关状态、启停、上游/别名/Key 数量、Hero/Stats widget |
| Switch 模型与路由 | `/models` | 模型别名映射、节点组、Client Key 管理 |
| Chat | `/chat` | 聊天框（Markdown，选模型，可填 key） |
| Storage 账本与对账 | `/billing` | 账本 + 对账报表 |
| About/Traces 链路追踪 | `/traces` | trace 查询（真实数据，无 demo） |
| Settings 设置 | `/settings` | 通用/网络/模块/关于（含网关配置、代理配置） |

---

## 7. 相关文档

- [Architecture](architecture.md) — 前后端关系
- [Gateway API](gateway-api.md) — 前端通过 IPC/HTTP 调用的后端
- [Configuration](configuration.md) — 设置页对应的配置
