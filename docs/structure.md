# 项目结构

## 1. 顶层目录

| 路径 | 说明 |
|------|------|
| `crates/core/` | 共享业务逻辑库，封装 hnu_query |
| `crates/server/` | HTTP 服务（axum），嵌入前端 + API + SPA fallback |
| `crates/adapter/` | 站点适配框架：QuickJS 沙箱 + 脱敏 + 验证 + 热重载 |
| `adapters/` | 站点适配包（配置 + 脚本 + fixtures），见 docs/adapter-framework.md |
| `frontend/` | React SPA 前端（Vite + shadcn/ui） |
| `fixtures/raw/` | 原始抓取数据（含隐私，gitignore，勿入库） |
| `docs/` | 项目文档 |
| `src-tauri/` | Tauri 桌面端（暂未实现） |

## 2. 架构

### WebUI 模式

```
┌──────────────────────────────────────────────────┐
│  hnu-cg-helper-server (localhost:20365)           │
│                                                   │
│  axum HTTP Server                                 │
│  ├─ /            → memory-serve → 嵌入的 dist/   │
│  │                 SPA fallback → index.html      │
│  ├─ /api/auth/*  → 登录/验证码                    │
│  ├─ /api/courses/* → 课程/作业/题目               │
│  └─ /api/ai/*    → AI 聊天 SSE + 配置管理         │
│                   ↓                               │
│              core crate ──→ hnu_query ──→ CG 服务器│
└──────────────────────────────────────────────────┘
```

- 前端 `dist/` 在编译时嵌入二进制（memory-serve）
- API 与前端同源（`localhost:20365`），无需 CORS、无需代理
- 用户启动一个程序，浏览器访问 `localhost:20365` 即可

### Tauri 模式（暂未实现）

```
┌──────────────────────────────────────────────┐
│  Tauri 桌面应用                               │
│                                               │
│  OS 原生 WebView                              │
│  └─ 前端 (React SPA)                          │
│       └─ Tauri IPC ──→ core crate             │
│                         ↓                     │
│                    hnu_query ──→ CG 服务器     │
│                                               │
│  axum 服务器 ❌ 不需要（IPC 替代 HTTP）        │
└──────────────────────────────────────────────┘

两种模式复用同一个 `core` crate，只是桥接层不同（HTTP API vs Tauri IPC）。

## 3. crate 职责

### `crates/core` — 共享业务逻辑

| 模块 | 职责 |
|------|------|
| `auth` | 登录会话管理、Token 序列化/反序列化 |
| `course` | 课程/作业/题目 查询封装 |
| `problem` | 题目页管线（调适配框架）+ 提交计划执行 |
| `repair` | AI 修复循环（LLM 改脚本 → 验证 → 热重载） |
| `ai` | SSE 流式聊天客户端 + 非流式 chat_once |
| `config` | 凭据加密存储：AES-GCM + OS 密钥环，TOML 配置文件读写 |
| `error` | 统一错误类型 |

Core 不区分 HTTP 或 Tauri IPC，只暴露 Rust API。

### `crates/adapter` — 站点适配框架

| 模块 | 职责 |
|------|------|
| `engine` | QuickJS 沙箱：宿主 API 白名单注入、内存/栈/超时限额 |
| `sanitize` | 固定层脱敏：区域提取 + 模式擦除 + cleanHtml 白名单 |
| `adapter` | 适配包加载、题型嗅探、解析管线 |
| `schema` | 输出契约（ProblemPageOutput / SubmissionPlan 等） |
| `validate` | fixture 精确比对验证 |
| `registry` | 注册表热重载（validate-then-swap）、文件监听 |

详见 [adapter-framework.md](adapter-framework.md)。

### `crates/server` — HTTP 服务

| 模块 | 职责 |
|------|------|
| `routes/auth` | 验证码获取、登录 |
| `routes/course` | 课程/作业/题目 API + 提交 + AI 修复端点 |
| `routes/ai` | AI 聊天 SSE 端点、AI 配置管理 |
| `state` | 全局状态（session 存储、ConfigManager、适配器注册表） |
| 静态文件 | memory-serve 嵌入 `frontend/dist/`，SPA fallback 到 `index.html` |
| `build.rs` | 编译时检查 `frontend/dist/` 并加载为静态资源 |

server 负责：
1. 嵌入并 serve 前端静态文件（编译时 `frontend/dist/`）
2. 所有未匹配 API 的路径 fallback 到 `index.html`，由 React Router 接管
3. 提供 REST API 供前端调用

## 4. 前端组件

| 组件 | 职责 |
|------|------|
| `Sidebar` | 左侧导航：课程列表展开 + 作业链接 |
| `AIPanel` | 右侧 AI 助手：流式对话，自动携带当前题目上下文 |
| `stores/problem-context` | 当前题目上下文（题面纯文本），供 AI 面板使用 |
| `pages/login` | 登录页：学号/密码/验证码 |
| `pages/courses` | 课程卡片列表 |
| `pages/assignments` | 作业列表 |
| `pages/problems` | 题目列表（含分值） |
| `pages/problem` | 题目详情：结构化题面渲染（.cg-prose）+ 提交区 + 失败一键修复 |
