# Quant 异步任务队列对接 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 quant 计算改为“点击一次=入队一个 task_job”，执行过程中按 `fund_code` 输出子步骤日志，并在“任务队列”页面可追踪。

**Architecture:** `frontend → backend enqueue → task_job → background worker → quant-service`。后端入队时把 `quant_service_url` 写入 payload，避免执行器依赖全局 env（测试也更稳定）。

**Tech Stack:** Rust/axum + sqlx(any) + reqwest；现有 `task_job/task_run/task_run_log`。

---

### Task 1: 增加入队路由（metrics batch）

**Files:**
- Modify: `backend/crates/api/src/routes/quant.rs:1`
- Modify: `backend/crates/api/src/routes/mod.rs:1`
- Test: `backend/crates/api/tests/quant_task_queue_test.rs`

**Step 1: 写失败测试（RED）**
- 访问 `POST /api/quant/xalpha/metrics_batch_async` 应返回 202 + `task_id`

**Step 2: 实现路由（GREEN）**
- 入参：`fund_codes[]`、`source?`、`risk_free_annual?`、`window?`
- 入队：`task_type=quant_xalpha_metrics_batch`
- payload 自动注入 `quant_service_url`
- 202 返回 `task_id`

---

### Task 2: 执行器支持 quant_xalpha_metrics_batch

**Files:**
- Modify: `backend/crates/api/src/tasks.rs:1`
- Test: `backend/crates/api/tests/quant_task_queue_test.rs`

**Step 1: 写失败测试（RED）**
- 起一个本地 stub quant-service（axum）响应 `/api/quant/xalpha/metrics`
- 通过入队 + `run_due_task_jobs` 执行任务
- 断言：`task_job.status == 'done'`，日志包含 `[fund_code]` 子步骤

**Step 2: 最小实现（GREEN）**
- 从 DB 拉取每个 fund_code 的 NAV 序列（按日期升序）
- 调用 quant-service `/api/quant/xalpha/metrics`
- 写日志：开始/成功/失败/汇总

---

### Task 3: 验证

**Step 1: backend tests**
- Run: `cargo test -p api --test quant_task_queue_test -- --nocapture`

