# 任务队列 + 信号异步/分页 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 新增“任务队列”页面与后端可观测 API，并将批量信号改为异步任务 + 分页取回，补齐 run 日志输出以便调试。

**Architecture:** 以 `crawl_job`（爬虫）+ `task_job`（计算任务）双队列并存，统一沉淀到 `task_run/task_run_log`；前端用 `/tasks` 页面读取 `overview + run_logs` 展示；嗅探页信号改为创建 `signals_batch` 任务并轮询分页结果。

**Tech Stack:** Rust (axum/sqlx AnyPool), Next.js (App Router) + Ant Design.

---

## Task 1: 后端接入 task_job 执行循环

**Files:**
- Modify: `backend/crates/api/src/config.rs`
- Modify: `backend/crates/api/src/crawl/worker.rs`

**Steps:**
1. 写测试：默认配置包含 `task_run_max_jobs`（并可被 env 覆盖）
2. 运行测试：`cargo test -p api config_defaults_test -- --nocapture`（预期先失败）
3. 实现：在默认配置加 `task_run_max_jobs`，在后台 tick 里调用 `tasks::run_due_task_jobs`
4. 运行测试：同上（预期通过）

---

## Task 2: 信号异步入队与分页接口（替换 /batch）

**Files:**
- Modify: `backend/crates/api/src/routes/fund_signals.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`（如需新增路由）
- Test: `backend/crates/api/tests/fund_signals_batch_route_test.rs`（改为 async 流程或新增测试文件）

**Steps:**
1. 写 failing test：`POST /api/funds/signals/batch` 返回 `{task_id}`，执行 `tasks::run_due_task_jobs` 后，分页接口可读到结果
2. 运行测试：`cargo test -p api fund_signals_batch_route_test -- --nocapture`（预期先失败）
3. 实现：新增 `POST /api/funds/signals/batch_async` + `GET /api/funds/signals/batch_async/{task_id}`，并让 `/batch` 变为 async 入队
4. 运行测试：同上（预期通过）

---

## Task 3: 前端任务队列页与导航入口

**Files:**
- Modify: `frontend/src/components/AuthedLayout.tsx`
- Create: `frontend/src/app/tasks/page.tsx`
- Modify: `frontend/src/lib/api.ts`

**Steps:**
1. 写前端测试（如已有 test harness）：至少覆盖 `api` 方法返回 shape（可选）
2. 实现 `/tasks`：拉取 `/tasks/overview`，展示队列/运行/最近完成；点击 run 拉取 `/tasks/runs/{id}/logs`
3. 在左侧导航栏底部加入“任务队列”

---

## Task 4: 嗅探页信号改为异步任务 + 分页轮询

**Files:**
- Modify: `frontend/src/app/sniffer/page.tsx`

**Steps:**
1. 写 failing test（优先逻辑单测）：分页合并结果映射到 `signalsByFund`
2. 实现：点击“加载信号”触发入队；轮询任务状态与分页结果；提供“去任务队列查看日志”的链接
3. 运行前端测试：`npm test`

---

## Task 5: 替换所有 queue 入口并验证

**Files:**
- Modify: 所有包含 “queue/队列/入队” 的入口（全仓搜索）

**Steps:**
1. `rg -n "\\bqueue\\b|队列|入队" frontend/src` 找到入口
2. 统一跳转 `/tasks` 或在操作成功后提示 “去任务队列查看进度/日志”
3. 验证：`cargo test -p api` 与 `npm test`

