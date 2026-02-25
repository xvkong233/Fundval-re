# Hotfix Implementation Plan: Postgres 类型修复 + 模拟盘删除 + 自选跳转

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复 Docker(Postgres) 下 `uuid/date = text` 相关 500 报错，并补齐模拟盘运行删除能力、自选页跳转到基金详情，顺带修复 `000001` 被“Test Fund”占用导致基础信息不可信的问题。

**Architecture:** 后端继续使用 `sqlx::AnyPool`，通过 `db_kind()`/`database_kind_from_pool()` 在 SQL 层对 Postgres 参数做显式类型转换（`::uuid` / `::date`），避免 driver 以 `TEXT` 绑定导致的比较/插入失败。前端仅做小改动：补 API 调用与按钮/链接。

**Tech Stack:** Rust(Axum, sqlx AnyPool), Postgres/SQLite migrations, Next.js + Ant Design, Jest.

---

### Task 1: 修复 `sim_train_round.run_id uuid but expression is text`

**Files:**
- Modify: `backend/crates/api/src/sim/engine.rs`
- Test: `backend/crates/api/tests/sim_train_round_sql_cast_test.rs`

**Step 1: 写失败测试（SQL 断言）**

创建测试断言 Postgres SQL 包含 `($1)::uuid`（或等价 `::uuid` cast），当前实现应失败（未 cast）。

Run: `cargo test -p api --tests sim_train_round_sql_cast_test -q`
Expected: FAIL（断言找不到 `::uuid`）

**Step 2: 最小实现**

将训练结果 upsert SQL 提取为 helper：`sim_train_round_upsert_sql(is_postgres: bool) -> &'static str`，Postgres 分支为 `VALUES (($1)::uuid, ...)`。

**Step 3: 重新运行测试**

Run: `cargo test -p api --tests sim_train_round_sql_cast_test -q`
Expected: PASS

---

### Task 2: 增加“删除模拟盘运行”后端接口

**Files:**
- Modify: `backend/crates/api/src/routes/sim.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`（如果需要补路由）
- Test: `backend/crates/api/tests/sim_routes_test.rs`

**Step 1: 写失败测试**

在 `sim_routes_test.rs`：
1) 创建 run
2) 调用 `DELETE /api/sim/runs/{id}`
3) 再次 `GET /api/sim/runs` 不包含该 id

Run: `cargo test -p api --tests sim_routes_test -q`
Expected: FAIL（404 或 method not allowed）

**Step 2: 最小实现**

新增 handler `delete_sim_run`：
- 需登录（复用现有 auth 校验方式）
- 只允许删除 `user_id` 本人的 run
- 直接 `DELETE FROM sim_run ...`，依赖外键 `ON DELETE CASCADE` 清理关联表

**Step 3: 重新运行测试**

Run: `cargo test -p api --tests sim_routes_test -q`
Expected: PASS

---

### Task 3: 前端补“删除模拟盘运行”按钮

**Files:**
- Modify: `frontend/src/lib/api.ts`
- Modify: `frontend/src/app/sim/page.tsx`

**Step 1: 写（或补）最小调用**

新增 `deleteSimRun(runId)`：`DELETE /sim/runs/{id}`。

**Step 2: UI**

在模拟盘列表“操作”列新增 `Popconfirm + 删除`，成功后重新拉取 runs 列表。

**Step 3: 手工验证**

Run: `docker compose up -d --build backend frontend`
Expected: 模拟盘列表可删除某条 run，列表刷新且详情页访问应返回 404/提示不存在。

---

### Task 4: 自选页基金代码/名称支持跳转到基金详情

**Files:**
- Modify: `frontend/src/app/watchlists/page.tsx`

**Step 1: 最小实现**

将“代码”“基金名称”两列改为 `Link href=/funds/{fund_code}`，并保持单行省略（避免换行导致布局抖动）。

**Step 2: 手工验证**

打开自选页，点击代码/名称应进入对应基金详情页。

---

### Task 5: 修复 `000001` 被“Test Fund”占用（基础信息自动纠偏）

**Files:**
- Modify: `backend/crates/api/src/routes/funds.rs`

**Step 1: 写失败测试（可选）**

若现有测试体系不方便 mock 上游，则跳过自动化测试，改为增加极小的“条件刷新”逻辑并用 docker 手测验证。

**Step 2: 最小实现**

在基金详情/估值等路径读取到 `fund_name` 为明显占位符（例如 `Test Fund`/空字符串）时，触发一次 `ensure_fund_exists` 刷新基础信息并回查。

**Step 3: 手工验证**

在 Postgres 容器里更新：
- 查询 `SELECT fund_code,fund_name FROM fund WHERE fund_code='000001'`
- 访问 `GET /api/funds/000001/` 后应自动纠正为真实基金名（来自上游）

---

### Task 6: Docker(Postgres) 验证：日志不再出现类型错误

**Step 1: 重建并重启**

Run: `docker compose up -d --build backend`

**Step 2: 观察日志**

Run: `docker compose logs -n 200 backend db-candidate`
Expected: 不再出现
- `operator does not exist: uuid = text`
- `operator does not exist: date = text`
- `column "run_id" is of type uuid but expression is of type text`

