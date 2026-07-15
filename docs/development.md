# 本地开发指南

## 环境要求

- **Rust** 1.85+ (edition 2024)
- **Node.js** 22+
- **pnpm** 9+

## 快速开始

### 开发模式（推荐：一个命令前端 + 一个命令后端）

前端代码变动频繁，用 Vite dev server 获得热更新（HMR）：

```bash
# 终端 1: 启动后端
cargo run -p hnu-cg-helper-server
# 监听 localhost:20365

# 终端 2: 启动前端 dev server
cd frontend && pnpm dev
# 监听 localhost:5173，/api 代理到 20365
```

访问 `http://localhost:5173`。修改前端代码即时生效，无需刷新。

### 生产模式（单二进制）

```bash
cd frontend && pnpm build    # 构建前端到 dist/
cargo run -p hnu-cg-helper-server
```

访问 `http://localhost:20365`。前端 `dist/` 由 memory-serve 嵌入并 serve。

## 常用命令

| 命令 | 说明 |
|------|------|
| `cargo build` | 构建整个 workspace |
| `cargo run -p hnu-cg-helper-server` | 运行后端服务（含前端） |
| `cargo test --workspace` | 运行所有测试 |
| `cargo clippy --all-targets -- -D warnings` | 代码检查 |
| `cargo fmt --check` | 格式检查 |
| `cd frontend && pnpm dev` | 启动前端开发服务（HMR） |
| `cd frontend && pnpm build` | 构建前端生产版本（输出 dist/） |
| `cd frontend && pnpm lint` | 前端代码检查 |

## 环境变量

| 变量 | 说明 |
|------|------|
| `RUST_LOG` | 日志级别，默认 `hnu_cg_helper_server=debug,info` |

## 构建与发布

### 构建流程

**前端必须先于 Rust 编译**，因为 memory-serve 在编译时把 `frontend/dist/` 嵌入二进制。

```bash
# 1. 构建前端（产出 dist/）
cd frontend && pnpm build

# 2. 构建 Rust 二进制（嵌入 dist/）
cargo build -p hnu-cg-helper-server --release
```

结构示意：

```
hnu-cg-helper-server          ← 单一二进制
├── 嵌入的 frontend/dist/      ← memory-serve 编译时嵌入
│   ├── index.html
│   ├── assets/
│   └── ...
├── axum HTTP 服务 (端口 20365)
│   ├── /      → 静态文件 + SPA fallback
│   └── /api/* → REST API
└── core 业务逻辑
```

### 前端未构建时的行为

如果编译时 `frontend/dist/` 不存在，`cargo build` 会失败。memory-serve 的 `MemoryServe::load_dir!()` 宏要求编译期路径存在，否则编译报错。