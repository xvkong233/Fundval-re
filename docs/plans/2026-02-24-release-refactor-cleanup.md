# 发布标准重构与清理 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 对 FundVal-re 的目录结构/依赖/文档做发布级整理，并清理明显无用文件与构建产物规则，保证三服务（backend/frontend/quant-service）可运行、可测试、可部署。

**Architecture:** 维持现有三服务拆分（Rust API + Next.js UI + FastAPI quant-service），通过补齐服务级 README、收敛忽略规则、增加 docs 索引与自检脚本来提升发布可维护性；不做“破坏性 API 重命名/大规模代码搬迁”。

**Tech Stack:** Rust(axum/sqlx) + Next.js(React) + Python(FastAPI/uvicorn) + Docker Compose + GitHub Actions

---

## Task 1: 仓库目录与发布范围审计

**Files:**
- Modify: `.gitignore`
- Modify: `.dockerignore`
- Create: `docs/README.md`

**Step 1: 列出未追踪目录/文件并分类（保留/忽略/删除）**
- Run: `git status --porcelain=v1`
- 记录：`backend/crates/api/src/{forecast,sim,tasks...}`、`frontend/src/app/{tasks,strategies,sim}`、`quant-service/`、`design-system/`、`docs/reviews/` 等是否属于发布内容。

**Step 2: 收敛 .dockerignore（减少 build context）**
- 增加 Python 缓存与 venv、更多前端/后端产物目录的忽略项（不影响源码）。

**Step 3: 增加 docs 索引**
- 新增 `docs/README.md`：说明 `docs/API文档/`、`docs/plans/`、`docs/reviews/` 的用途与入口。

---

## Task 2: 服务级 README 与开发指引

**Files:**
- Create: `backend/README.md`
- Create: `frontend/README.md`
- Modify: `README.md`（如需补充链接）

**Step 1: backend README**
- 包含：本地开发（`cargo run -p api`/env）、迁移（sqlite/postgres）、运行最小命令、常见故障（端口/DB 连接）。

**Step 2: frontend README**
- 包含：本地开发（`pnpm dev`/`npm run dev`）、环境变量、与 backend/quant-service 的交互说明。

---

## Task 3: 清理明显无用文件/规则（不破坏功能）

**Files:**
- Modify: `.gitignore`（必要时）
- Modify: `README.md`（必要时）

**Step 1: 删除/忽略临时产物**
- 确认：`_sim.patch`、`sim_engine_test_output.txt` 等临时文件已删除并被忽略。

**Step 2: 保留但隔离 upstream**
- 确认 `.codex/_upstream/` 始终被 `.gitignore` 与 `.dockerignore` 排除。

---

## Task 4: 关键自检与回归

**Files:**
- Modify/Create: 视需要（仅修复阻塞性问题）

**Step 1: 后端测试**
- Run: `cargo test -p api`
- 期望：全绿；若失败，仅修复与本次整理直接相关的问题。

**Step 2: quant-service 测试**
- Run: `cd quant-service; pytest -q`

**Step 3: 前端最小构建检查（可选）**
- Run: `cd frontend; npm test`（若有）或 `npm run build`（若可承受时间）

