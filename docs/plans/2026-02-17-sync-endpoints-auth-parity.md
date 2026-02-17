# Sync Endpoints Auth Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 通过 contract-tests 将 Rust 实现的 sync 相关端点与 Django(golden) 在鉴权与响应行为上对齐，并把修复合并回 `main`。

**Architecture:** 使用“对照迁移（golden Django vs candidate Rust）+ contract-tests”的方式推进：先补充/收紧合同测试用例（RED），再修复 Rust 路由实现（GREEN），最后跑全量合同测试确认无回归后提交并合并。

**Tech Stack:** Docker Compose（`--profile contract`）、TypeScript contract-tests、Rust API（`backend-rs`）。

---

### Task 1: 确认 worktree 与变更范围

**Files:**
- Modify: `backend-rs/crates/api/src/routes/nav_history.rs`
- Modify: `backend-rs/crates/api/src/routes/funds.rs`
- Modify: `tools/contract-tests/src/cases/nav_history.ts`

**Step 1: 进入 worktree 并确认分支**

Run: `cd .worktrees/sync-endpoints; git status -sb`

Expected: `## wip/sync-endpoints` 且仅有上述 3 个文件处于修改状态。

**Step 2: 确认 `.worktrees/` 已被 gitignore**

Run: `cd ..\\..; git check-ignore -v .worktrees`

Expected: 输出显示来自 `.gitignore` 的忽略规则（例如 `.worktrees/`）。

---

### Task 2: 跑单用例合同测试验证 nav_history 的 401/403 行为

**Files:**
- Test: `tools/contract-tests/src/cases/nav_history.ts`

**Step 1: 运行 nav_history 相关用例**

Run (示例端口避免冲突，可按需调整):
`cd .worktrees/sync-endpoints; $env:COMPOSE_PROJECT_NAME="fundval-sync-navhist"; $env:BACKEND_HOST_PORT="18082"; $env:BACKEND_RS_HOST_PORT="18083"; $env:CASE_FILTER="nav_history"; docker compose --profile contract up --build --exit-code-from contract-tests db-golden db-candidate redis backend backend-rs contract-tests`

Expected: `contract-tests` 退出码为 `0`。

**Step 2: 清理容器（可选但推荐）**

Run: `docker compose -p fundval-sync-navhist down -v`

---

### Task 3: 跑全量合同测试，发现并修复回归

**Files:**
- Test: `tools/contract-tests/src/**/*`
- Modify: `backend-rs/crates/api/src/routes/**/*.rs`（仅在发现差异时最小化修改）

**Step 1: 运行全量合同测试**

Run:
`cd .worktrees/sync-endpoints; $env:COMPOSE_PROJECT_NAME="fundval-sync-all"; $env:BACKEND_HOST_PORT="18182"; $env:BACKEND_RS_HOST_PORT="18183"; Remove-Item Env:CASE_FILTER -ErrorAction SilentlyContinue; docker compose --profile contract up --build --exit-code-from contract-tests db-golden db-candidate redis backend backend-rs contract-tests`

Expected: `contract-tests` 退出码为 `0`。

**Step 2: 若失败，按 case 定位**

Run（示例）:
`$env:CASE_FILTER="funds"; docker compose --profile contract up --build --exit-code-from contract-tests db-golden db-candidate redis backend backend-rs contract-tests`

Expected: 能稳定复现单个 case 的差异，便于修复。

**Step 3: 修复后重复 Step 2，直到全量通过**

---

### Task 4: 提交、合并回 main 并推送 origin

**Files:**
- Modify: `backend-rs/crates/api/src/routes/nav_history.rs`
- Modify: `backend-rs/crates/api/src/routes/funds.rs`
- Modify: `tools/contract-tests/src/cases/nav_history.ts`

**Step 1: 确保全量合同测试已通过（Task 3 Step 1 退出码 0）**

**Step 2: 在 worktree 提交变更**

Run:
`cd .worktrees/sync-endpoints; git add backend-rs/crates/api/src/routes/nav_history.rs backend-rs/crates/api/src/routes/funds.rs tools/contract-tests/src/cases/nav_history.ts; git commit -m "fix: align sync auth behavior with golden"`

Expected: 生成 1 个 commit，描述清晰。

**Step 3: 合并到 main 并推送**

Run:
`cd ..\\..; git checkout main; git pull; git merge wip/sync-endpoints`

Then run（再次验证全量合同测试，端口可复用/另选）:
`$env:COMPOSE_PROJECT_NAME="fundval-sync-main-verify"; $env:BACKEND_HOST_PORT="18282"; $env:BACKEND_RS_HOST_PORT="18283"; docker compose --profile contract up --build --exit-code-from contract-tests db-golden db-candidate redis backend backend-rs contract-tests`

Expected: 退出码 `0` 后再执行 `git push origin main`。

**Step 4: 清理 worktree（合并完成后）**

Run:
`git worktree remove .worktrees/sync-endpoints; git branch -d wip/sync-endpoints`

