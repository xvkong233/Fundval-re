# Async Estimate + NAV Sync Queue Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task.

**Goal:** 将“净值同步/估值更新”从请求内批量抓取改为“入队 + 后台节流执行”，避免一次性爬取过多导致外部 API 封锁，并让前端以缓存结果+后台刷新实现“异步实时估值”体验。

**Architecture:** 复用现有 `crawl_job` 队列与后台 worker。`/api/nav-history/sync` 默认只入队 `nav_history_sync`；`/api/funds/batch_estimate` 默认只读缓存并为过期/缺失的基金入队 `estimate_sync`。后台按 `crawl_per_job_delay_ms` 串行执行，控制节奏。估值任务的成功重试间隔独立于净值任务。

**Tech Stack:** Rust (axum, sqlx AnyPool), SQLite/Postgres migrations, tokio background worker。

---

### Task 1: 为 estimate job 写 failing test（scheduler / worker 维度）

**Files:**
- Create: `backend/crates/api/tests/crawl_estimate_jobs_test.rs`

**Step 1: Write the failing test**
- 测试 1：调用 `api::crawl::scheduler::enqueue_estimate_tick(&pool, 10, "tiantian")` 能为自选/持仓基金插入 `job_type='estimate_sync'` 的 `crawl_job`，且优先级自选 > 持仓。
- 测试 2：验证同 key 再次入队不会无意义更新 `updated_at`（保持 scheduler 的“仅提升优先级才更新”规则）。

**Step 2: Run test to verify it fails**
- Run: `cargo test -p api crawl_estimate_jobs_test -- --nocapture`
- Expected: FAIL（函数/JobType 尚不存在或行为不符合）。

---

### Task 2: 实现 estimate job 的入队与调度（GREEN）

**Files:**
- Modify: `backend/crates/api/src/crawl/scheduler.rs`
- Modify: `backend/crates/api/src/crawl/worker.rs`

**Step 1: Minimal implementation**
- `scheduler.rs`：
  - 新增 `pub async fn enqueue_estimate_tick(pool, max_jobs, source_name) -> Result<i64,String>`
  - 新增 `pub async fn upsert_estimate_job(pool, fund_code, source_name, priority) -> Result<(),String>`
  - 入队策略先自选、再持仓、再（可选）全市场慢速 round-robin（先不做全市场也可）。
  - 为 `estimate_sync` 定义独立的 success delay（例如自选 2min、持仓 5min、其余 30min）。
- `worker.rs`：
  - 在 `exec_one` 增加 `estimate_sync` 分支：按 source 取估值（tiantian 用 `eastmoney::fetch_estimate`），写回 `fund.estimate_*` 并 best-effort 记录 `estimate_accuracy`。

**Step 2: Run tests**
- Run: `cargo test -p api crawl_estimate_jobs_test -- --nocapture`
- Expected: PASS

---

### Task 3: 将 batch_estimate 改为“读缓存 + 入队刷新”（RED→GREEN）

**Files:**
- Create: `backend/crates/api/tests/batch_estimate_async_test.rs`
- Modify: `backend/crates/api/src/routes/funds.rs`

**Step 1: Write the failing test**
- 准备：插入 fund A（带过期的 `estimate_time`）与 fund B（无估值），调用 `POST /api/funds/batch_estimate`。
- 断言：
  - 返回结构包含 A/B，并优先返回 DB 中已有字段（缺失时允许返回 `null`）。
  - 对过期/缺失的基金，会在 `crawl_job` 表插入/更新 `estimate_sync` 任务（不在请求内直接外部抓取）。

**Step 2: Run test to verify it fails**
- Run: `cargo test -p api batch_estimate_async_test -- --nocapture`
- Expected: FAIL

**Step 3: Minimal implementation**
- 增加配置开关 `estimate_async_enabled`（默认 true）：开启时 `batch_estimate` 不做外部抓取，只入队刷新。
- 响应增加字段 `queued_refresh`（bool）用于前端提示“后台刷新中”。

**Step 4: Run tests**
- Run: `cargo test -p api batch_estimate_async_test -- --nocapture`
- Expected: PASS

---

### Task 4: 将 nav-history sync 默认改为“只入队”（RED→GREEN）

**Files:**
- Create: `backend/crates/api/tests/nav_history_sync_enqueue_test.rs`
- Modify: `backend/crates/api/src/routes/nav_history.rs`
- Modify: `backend/crates/api/src/crawl/scheduler.rs`

**Step 1: Write the failing test**
- 调用 `POST /api/nav-history/sync`（传入 2 个 fund_codes），断言：
  - 返回包含 `queued=true`（或 `mode=enqueue`），且不会因为外部抓取不可用而失败。
  - `crawl_job` 中出现对应 `nav_history_sync` 任务。

**Step 2: Run test to verify it fails**
- Run: `cargo test -p api nav_history_sync_enqueue_test -- --nocapture`
- Expected: FAIL

**Step 3: Minimal implementation**
- `sync()` 默认走 enqueue；保留显式 `mode=inline`（管理员/小批量）以兼容原行为（可选）。

**Step 4: Run tests**
- Run: `cargo test -p api nav_history_sync_enqueue_test -- --nocapture`
- Expected: PASS

---

### Task 5: 基础回归验证（Verification）

**Files:**
- None

**Step 1: Run focused tests**
- Run: `cargo test -p api crawl_scheduler_test -- --nocapture`
- Run: `cargo test -p api crawl_run_due_jobs_test -- --nocapture`

**Step 2: Run broader suite (可选)**
- Run: `cargo test -p api -- --nocapture`

