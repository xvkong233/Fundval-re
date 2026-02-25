# Fund Analysis v2（Qbot/xalpha 口径）Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 彻底替换旧的基金分析（`/funds/{code}/analytics` + 前端“专业指标/预测走势”）为 v2：统一走任务队列 + quant-service（Qbot/xalpha 风格输出），并将结果落库成快照供前端直接展示。

**Architecture:** `frontend → backend enqueue → task_job → background worker → quant-service`；计算产出写入 `fund_analysis_snapshot`（按 fund_code+source+profile upsert）。

**Tech Stack:** Rust/axum + sqlx(any) + task_job/task_run/task_run_log；Python/FastAPI quant-service。

---

## Task 1：新增分析快照表

**Files:**
- Create: `backend/migrations/sqlite/20260223000003_create_fund_analysis_snapshot.sql`
- Create: `backend/migrations/postgres/20260223000003_create_fund_analysis_snapshot.sql`

**Schema:**
- `fund_analysis_snapshot(id, fund_code, source, profile, as_of_date, result_json, last_task_id, created_at, updated_at)`
- Unique: `(fund_code, source, profile)`

---

## Task 2：新增 v2 路由（入队 + 读取快照）

**Files:**
- Create: `backend/crates/api/src/routes/fund_analysis_v2.rs`
- Modify: `backend/crates/api/src/routes/mod.rs:1`
- Modify: `frontend/src/lib/api.ts:1`

**Endpoints:**
- `GET /api/funds/{fund_code}/analysis_v2?source=...` → 返回最新快照（无则 404）
- `POST /api/funds/{fund_code}/analysis_v2/compute` → 入队 `task_type=fund_analysis_v2_compute`，返回 `202 { task_id }`

---

## Task 3：任务执行器 fund_analysis_v2_compute

**Files:**
- Modify: `backend/crates/api/src/tasks.rs:1`
- Test: `backend/crates/api/tests/fund_analysis_v2_test.rs`

**Behavior:**
- payload:
  - fund_code, source, profile, windows=[60,120,252], params(grid_step_pct/every_n/amount/sell_position/buy_position/risk_free_annual), quant_service_url
- 对每个 window：
  - 从 `fund_nav_history` 取 series（升序）
  - 调 quant-service：
    - `/api/quant/xalpha/metrics`
    - `/api/quant/macd`
    - `/api/quant/xalpha/grid`
    - `/api/quant/xalpha/scheduled`
  - 写 `[fund_code] window=... step=...` 日志
- upsert `fund_analysis_snapshot`

---

## Task 4：前端切换到 v2

**Files:**
- Modify: `frontend/src/app/funds/[fundCode]/page.tsx:1`

**Behavior:**
- 不再调用旧 `getFundAnalytics`
- 展示 v2 快照（分 60T/120T/252T）
- 提供“重新计算”按钮（入队 + 跳任务队列查看日志）

---

## Verification

- `cd backend; cargo test -p api --tests`
- `cd quant-service; python -m pytest -q`

