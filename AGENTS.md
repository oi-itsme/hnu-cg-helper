# CLAUDE.md

## 项目概况

hnu-cg-helper 是一个 CG 做题系统辅助客户端。
基于 hnu_query 库，支持 Web UI 和 Tauri（留到后期再做）。

技术栈：Rust (axum + core lib) + React + TypeScript + shadcn/ui。

## 目录结构

- `crates/core/` — 共享业务逻辑，封装 hnu_query
- `crates/server/` — HTTP API 服务（axum）
- `frontend/` — React SPA（Vite + shadcn/ui）
- `docs/` — 项目文档
- `src-tauri/` — Tauri 桌面端

## 常用命令

```bash
cargo build                        # 构建 workspace
cargo run -p hnu-cg-helper-server  # 启动 API 服务
cargo test --workspace             # 运行测试
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cd frontend && pnpm dev            # 启动前端
cd frontend && pnpm build          # 构建前端
```

## 注意事项

- 开发前应阅读 `docs/` 中相关版块的信息
- 修改代码需同步更新文档
- 前端代码风格遵循 oxlint 规则

## 开发守则

对即将执行的每一项操作都应向我持续追问各方面细节，直到我们达成共识。沿着决策树的每个分支走下去，逐一解决决策之间的依赖关系。每个问题都要给出你的推荐答案。

每次只问一个问题，等我反馈后再继续。一次抛出多个问题会让人困惑。

如果某个事实可以通过探索代码库找到，就去查，不要问我。对于事实性信息，联网查询最新文档，确保给出的信息是对的。注意每个决策必须由我确认，等我答复。

在确认我们已达成共识之前，不要动手。
