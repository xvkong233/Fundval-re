# Multi-Source 基金数据（天天基金/蛋卷/同花顺）Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 后端与前端全链路支持多数据源（`tiantian`/`danjuan`/`ths`），覆盖基金列表同步、估值、净值、历史净值同步、健康探测、准确率入库与计算；默认仍开箱即用走 `tiantian`。

**Architecture:** 在 `backend` 增加“数据源规范化 + 分发层”，把现有 `eastmoney` 作为 `tiantian` 实现并保留别名兼容；新增 `danjuan`/`ths` 实现。API 接口通过 `source` 参数选择数据源（缺省 `tiantian`），DB 历史净值表增加 `source_name` 维度避免多源覆盖。前端在基金详情页增加数据源选择器，并在运维页展示各源健康度。

**Tech Stack:** Rust (axum/sqlx/reqwest/serde), Postgres, Next.js (App Router) + Ant Design.

---

### Task 1: 约定与规范化（source 名称/别名）

**Files:**
- Create: `backend/crates/api/src/sources/mod.rs`
- Modify: `backend/crates/api/src/routes/sources.rs`
- Test: `backend/crates/api/tests/sources_alias_test.rs`

**Step 1: Write failing test**
- 断言 `eastmoney`→`tiantian`，`tonghuashun`→`ths`，未知返回 `None`。

**Step 2: Run test to verify it fails**
- Run: `cd backend; cargo test -p api sources_alias_test -v`

**Step 3: Minimal implementation**
- 实现 `normalize_source_name()` + `builtin_sources()` 返回 `tiantian/danjuan/ths`。

---

### Task 2: 增加 danjuan/thx(同花顺) 源实现（估值/净值/历史净值）

**Files:**
- Create: `backend/crates/api/src/sources/danjuan.rs`
- Create: `backend/crates/api/src/sources/ths.rs`
- Test: `backend/crates/api/tests/sources_fetch_smoke_test.rs`

**Step 1: Write failing test**
- 对 `danjuan.fetch_realtime_nav("161725")` 与 `ths.fetch_realtime_nav("000001")` 做 smoke（只验证能解析出日期与数值；失败时输出错误字符串）。

**Step 2: Run test to verify it fails**
- Run: `cd backend; cargo test -p api sources_fetch_smoke_test -v`

**Step 3: Minimal implementation**
- `danjuan`：调用 `https://danjuanapp.com/djapi/fund/nav/history/{code}?page=1&size=...` 解析 `date/nav/percentage`。
- `ths`：调用 `https://fund.10jqka.com.cn/{code}/json/jsondwjz.json`（可选再取 `jsonljjz.json`）解析 `var xxx=[...]`。

---

### Task 3: API 分发（funds/nav-history/sources health）

**Files:**
- Modify: `backend/crates/api/src/routes/funds.rs`
- Modify: `backend/crates/api/src/routes/nav_history.rs`
- Modify: `backend/crates/api/src/routes/sources.rs`
- Test: `backend/crates/api/tests/funds_sources_param_test.rs`

**Step 1: Write failing test**
- `/api/funds/{code}/estimate?source=danjuan` 不再返回 “数据源不存在”。
- `/api/sources/health/` 返回 `tiantian/danjuan/ths` 三项。

**Step 2: Run test to verify it fails**
- Run: `cd backend; cargo test -p api funds_sources_param_test -v`

**Step 3: Minimal implementation**
- 请求参数/请求体增加可选 `source` 字段；统一通过 `normalize_source_name()` 分发到对应实现；默认 `tiantian`。

---

### Task 4: DB 迁移：历史净值按 source 分表（同表加维度）

**Files:**
- Create: `backend/migrations/20260218000006_add_source_to_nav_history.sql`
- Modify: `backend/crates/api/src/routes/nav_history.rs`

**Step 1: Write failing test**
- 直接写 SQLx 集成测试（可选）或至少保证编译通过，并在文档说明需要执行 migrations。

**Step 2: Minimal implementation**
- `fund_nav_history` 增加 `source_name`，唯一约束改为 `(source_name, fund_id, nav_date)`；查询与 upsert 加上 `source_name`。

---

### Task 5: 准确率入库与计算

**Files:**
- Modify: `backend/crates/api/src/routes/funds.rs`
- Add: `backend/crates/api/src/routes/admin_accuracy.rs`（或放入现有 routes）
- Modify: `backend/crates/api/src/routes/mod.rs`
- Test: `backend/crates/api/tests/accuracy_calculate_route_test.rs`

**Step 1: Write failing test**
- 估值成功后会 upsert `estimate_accuracy(source_name, fund_id, estimate_date, estimate_nav)`。
- 新增管理员 API：按日期拉取 `actual_nav` 并计算 `error_rate=abs(est-actual)/actual`。

**Step 2: Minimal implementation**
- 估值接口/批量估值：成功后写入 `estimate_accuracy`（ON CONFLICT DO UPDATE）。
- 计算接口：仅 `is_staff` 可调用，返回成功/失败计数。

---

### Task 6: 前端增加 source 选择器（基金详情页/同步）

**Files:**
- Modify: `frontend/src/app/funds/[fundCode]/page.tsx`
- Modify: `frontend/src/lib/api.ts`
- Test: `frontend/src/lib/__tests__/fundDetail.test.ts`

**Step 1: Write failing test**
- 选择 `danjuan` 时 `getFundEstimate(fundCode, "danjuan")` 被调用（或 normalize 后请求参数包含 source）。

**Step 2: Minimal implementation**
- 增加 `<Select>` 下拉（数据来自 `/sources/`）；估值、同步历史净值请求带 `source`；默认 `tiantian`。

---

### Task 7: 文档与验证

**Files:**
- Modify: `README.md`
- Modify: `.env.example`

**Steps**
- 文档写清：source 名称、别名、各源能力差异（估值可能为“以最新净值近似”）、以及迁移脚本必跑。
- Run: `cd backend; cargo test -p api`
- Run: `cd frontend; npm test`（如仓库已有测试脚本）

