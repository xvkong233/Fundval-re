# Main 分支仅保留移植后代码 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 `main` 分支移除 golden 源码，仅保留 `backend/` + `frontend/` 可运行版本；golden 代码保留在对照分支。

**Architecture:** `docker-compose.yml` 仅包含 candidate 栈（PostgreSQL + backend + frontend）。启动后通过日志输出 `BOOTSTRAP KEY` 完成初始化。

**Tech Stack:** Docker Compose、Rust (axum/sqlx)、Next.js

### Task 1: 保留对照参考分支

**Files:**
- Git: create branch `reference/golden`

**Step 1: 创建分支快照**
- Run: `git branch reference/golden HEAD`
- Expected: 分支指向移除前的 commit

### Task 2: 移除 golden 源码与对照工具

**Files:**
- Delete (git): `backend/`, `frontend/`, `tools/contract-tests/`
- Delete (git): `docker-compose.contract.yml`, `scripts/compose-contract.ps1`

**Step 1: 执行删除**
- Run: `git rm -r backend frontend tools docker-compose.contract.yml scripts/compose-contract.ps1`
- Expected: `git status` 显示大量 `D` 变更

### Task 3: 精简 compose / 文档 / 启动脚本

**Files:**
- Modify: `docker-compose.yml`
- Modify: `.env.example`
- Modify: `docker-start.sh`
- Modify: `README.md`
- Modify: `scripts/README.txt`

**Step 1: compose 仅保留 candidate**
- Run: `docker compose config`
- Expected: exit 0

**Step 2: 更新 README 与端口变量说明**
- Expected: README 中不再引用 `backend/`、`frontend/`、contract-tests

### Task 4: bootstrap key 开箱即用

**Files:**
- Modify: `backend/crates/api/src/main.rs`

**Step 1: 未初始化时输出 BOOTSTRAP KEY**
- Expected: `docker compose logs backend` 中可 grep 到 `BOOTSTRAP KEY`

### Task 5: 静态验证

**Step 1: bash 语法检查**
- Run: `bash -n docker-start.sh`
- Expected: exit 0

**Step 2: compose 配置检查**
- Run: `docker compose config`
- Expected: exit 0
