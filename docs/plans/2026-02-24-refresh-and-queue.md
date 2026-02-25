# 刷新（净值/估值）与任务队列执行 Implementation Plan

> **Goal:** 修复 `/funds`、`/watchlists` 的“最新净值/估值”显示与刷新体验，并让任务入队后立即执行（不再等待 tick）。

**Architecture:**
- 前端“刷新估值/净值”改为一次点击提交 1 个任务（内部对多个 `fund_code` 输出子步骤日志），任务完成后自动刷新列表数据。
- 后端任务调度增加 `Notify` 唤醒机制：入队后立即触发 `run_due_task_jobs`，避免依赖 30s tick。
- “最新净值/估值”统一口径：净值展示“最近确认净值”（上一交易日）+ 估值展示“当日实时估算净值”，并保证 DB 回写与列表接口返回一致。

**Tech Stack:** Rust (axum + sqlx), Postgres, Next.js/React, quant-service (Python)

## Tasks

### Task 1: 复现与证据收集
**Files:**
- Read: `frontend/src/app/funds/page.tsx`
- Read: `frontend/src/app/watchlists/page.tsx`
- Read: `backend/crates/api/src/routes/funds.rs`
- Read: `backend/crates/api/src/crawl/worker.rs`

**Steps:**
1. 通过 docker logs 与 DB `task_run` 复现 queued 延迟/刷新不全。
2. 记录刷新按钮调用的 API 序列与返回字段（净值/估值时间戳）。

### Task 2: 任务入队后立即执行
**Files:**
- Modify: `backend/crates/api/src/state.rs`（新增 `Notify`）
- Modify: `backend/crates/api/src/crawl/worker.rs`（tick + notify select）
- Modify: enqueue 任务的 routes（入队后 `notify_one()`）

**Test:**
- Add/Modify: `backend/crates/api/tests/*`（入队后立即进入 running 的可验证用例，若现有测试框架支持）

### Task 3: 刷新改为“单点击=单任务”
**Files:**
- Modify: `backend/crates/api/src/tasks/*`（新增 refresh 任务类型/handler）
- Modify: `backend/crates/api/src/routes/watchlists.rs`（新增 refresh endpoint -> 创建任务）
- Modify: `frontend/src/app/watchlists/page.tsx`（点击刷新创建任务并跳转/轮询）
- Modify: `frontend/src/app/funds/page.tsx`（同上，按需）

### Task 4: 统一“最新”口径与回写
**Files:**
- Modify: `backend/crates/api/src/routes/funds.rs`（确保 nav/estimate 更新后列表读取到最新字段）
- Optional: `backend/crates/api/src/routes/nav_history.rs`（若历史同步未回写 fund.latest_nav）
- Modify: `frontend/src/lib/funds.ts`（合并逻辑不重复、不叠加旧值）

### Task 5: 验证与清理
**Steps:**
1. 运行后端/quant-service 单测（若存在）并验证 docker-compose 非 sqlite 启动流程。
2. 用真实页面路径 `/funds`、`/watchlists` 验证：
   - 一次刷新触发 1 个任务并自动完成
   - 净值/估值时间戳更新符合预期
   - 任务入队后立即执行（无 30s 等待）

