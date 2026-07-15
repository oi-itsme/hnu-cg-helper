# 项目结构

## 1. 顶层目录

| 路径 | 说明 |
|------|------|
| `crates/core/` | 共享业务逻辑库，封装 hnu_query |
| `crates/server/` | HTTP 服务（axum），嵌入前端 + API + SPA fallback |
| `frontend/` | React SPA 前端（Vite + shadcn/ui） |
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
| `ai` | SSE 流式聊天客户端 |
| `config` | 凭据加密存储：AES-GCM + OS 密钥环，TOML 配置文件读写 |
| `error` | 统一错误类型 |

Core 不区分 HTTP 或 Tauri IPC，只暴露 Rust API。

### `crates/server` — HTTP 服务

| 模块 | 职责 |
|------|------|
| `routes/auth` | 验证码获取、登录 |
| `routes/course` | 课程/作业/题目 API |
| `routes/ai` | AI 聊天 SSE 端点、AI 配置管理 |
| `state` | 全局状态（session 存储、ConfigManager 实例） |
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
| `AIPanel` | 右侧 AI 助手：DeepSeek 流式对话 |
| `pages/login` | 登录页：学号/密码/验证码 |
| `pages/courses` | 课程卡片列表 |
| `pages/assignments` | 作业列表 |
| `pages/problems` | 题目列表（含分值） |
| `pages/problem` | 题目详情（HTML 渲染） |
