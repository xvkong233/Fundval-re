# FundVal-Live（Rust + Next.js）移植 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 `FundVal-Live` 以 Rust（axum + sqlx + Postgres）+ Next.js 的形式在本仓库实现“可对照验证的 100% 移植”（以接口契约与行为一致性为准）。

**Architecture:** 保留原 `backend/`（Django/DRF）作为 **golden**；新增 `backend-rs/` 作为 **candidate**；用 `tools/contract-tests/` 同时请求两套后端并做差分，逐接口补齐，直到对照测试全绿。

**Tech Stack:** Rust（axum, sqlx, tokio, reqwest, rust_decimal）、Next.js（frontend-next）、Postgres、Docker Compose、Node/TS 合同测试。

---

### Task 1: 修复并锁定 Rust 编译

**Files:**
- Modify: `backend-rs/crates/api/src/routes/nav_history.rs`
- Modify: `backend-rs/crates/api/Cargo.toml`

**Step 1: 写一个最小“编译即通过”的检查**

Run: `cd backend-rs; cargo check`
Expected: FAIL（如果目前有编译错误）

**Step 2: 最小修复让它通过**

- 修掉 `nav_history.rs` 的编译错误（类型、导入、feature、Decimal 解析等）

**Step 3: 再次验证**

Run: `cd backend-rs; cargo check`
Expected: PASS

---

### Task 2: 对齐 `/api/sources/{source}/accuracy/`（Rust）

**Files:**
- Modify: `backend-rs/crates/api/src/routes/sources.rs`
- (If needed) Modify: `backend-rs/crates/api/src/routes/mod.rs`
- Test: `tools/contract-tests/src/cases/sources_accuracy.ts`

**Step 1: 先跑对照用例确保目前失败**

Run: `cd tools/contract-tests; pnpm test --filter sources_accuracy`（如无 pnpm 则用 `npm test`）
Expected: FAIL（candidate 缺该端点或返回不一致）

**Step 2: 实现 accuracy 端点（最小行为）**

- 路由：`GET /api/sources/{source}/accuracy/`
- 行为：空表返回 `{avg_error_rate: 0, record_count: 0}`；有记录返回平均误差与条数（与 golden 一致）

**Step 3: 再跑对照**

Run: `cd tools/contract-tests; pnpm test --filter sources_accuracy`
Expected: PASS

---

### Task 3: 对齐 `/api/nav-history/` list/retrieve（Rust）

**Files:**
- Modify: `backend-rs/crates/api/src/routes/nav_history.rs`
- Test: `tools/contract-tests/src/cases/nav_history.ts`

**Step 1: 先跑 nav_history 对照用例**

Run: `cd tools/contract-tests; pnpm test --filter nav_history`
Expected: FAIL（如返回结构/状态码不同）

**Step 2: 实现 list/retrieve（DB 版本）**

- `GET /api/nav-history/`：按 golden 的查询参数/排序/分页行为对齐
- `GET /api/nav-history/{id}/`：404/字段结构对齐

**Step 3: 再跑对照**

Run: `cd tools/contract-tests; pnpm test --filter nav_history`
Expected: PASS（至少覆盖空库/404/缺参等分支）

---

### Task 4: 对齐 `/api/nav-history/batch-query/`（Rust）

**Files:**
- Modify: `backend-rs/crates/api/src/routes/nav_history.rs`
- Test: `tools/contract-tests/src/cases/nav_history.ts`

**Step 1: 扩展对照用例（先写失败）**

- 补齐缺参/空数组/非法 fund_code 的状态码与返回结构断言

**Step 2: 实现最小 batch-query 行为**

- 从 DB 按 `fund_code + date` 范围批量查历史净值并返回与 golden 一致的结构

**Step 3: 运行对照**

Run: `cd tools/contract-tests; pnpm test --filter nav_history`
Expected: PASS

---

### Task 5: 对齐 `/api/nav-history/sync/`（Rust）

**Files:**
- Modify: `backend-rs/crates/api/src/routes/nav_history.rs`
- (If needed) Modify: `backend-rs/crates/api/src/services/*.rs`
- Test: `tools/contract-tests/src/cases/nav_history.ts`

**Step 1: 先写/补齐对照用例（缺参、权限）**

- 未登录/非 admin 的 403 行为对齐
- `fund_codes` 长度上限（>15）错误行为对齐

**Step 2: 实现 sync（可重复执行、幂等）**

- 对每个 fund_code 抓取 EastMoney 历史净值并 upsert 到 `fund_nav_history`
- 返回 `{success: [...], error: [...]}` 或与 golden 一致的结构（以 docs 与对照为准）

**Step 3: 运行对照**

Run: `cd tools/contract-tests; pnpm test --filter nav_history`
Expected: PASS

---

### Task 6: 补齐 Funds action 成功路径（Rust）

**Files:**
- Modify: `backend-rs/crates/api/src/routes/funds.rs`
- Test: `tools/contract-tests/src/cases/*.ts`（缺啥补啥）

**Step 1: 逐个 action 写失败用例**

- `POST /api/funds/query-nav/`
- `POST /api/funds/batch-update-nav/`
- `POST /api/funds/sync/`
- `POST /api/funds/estimate/`、`POST /api/funds/batch-estimate/`

**Step 2: 最小实现让用例通过**

- 先对齐状态码/返回 schema，再补齐真实逻辑（eastmoney + DB）

**Step 3: 运行对照**

Run: `cd tools/contract-tests; pnpm test --filter funds`
Expected: PASS

---

### Task 7: 全量合同测试 + Docker 验证

**Files:**
- Modify (if needed): `docker-compose.yml`
- Modify (if needed): `backend-rs/Dockerfile`
- Modify (if needed): `README.md`

**Step 1: Compose 端到端对照**

Run:
- `docker compose down -v`
- `docker compose --profile contract up --build --abort-on-container-exit contract-tests`

Expected: contract-tests 容器退出码 0

**Step 2: 文档补齐**

- 在 `README.md` 写清楚启动方式、环境变量、对照测试命令与常见问题（Windows Docker Desktop）

---

## 执行方式（选择其一）

1) **本会话顺序执行（推荐）**：我在当前会话按上面 Task 逐条执行，边修边跑测试。
2) **子代理驱动执行**：需要启用协作子代理工具（如果环境支持），按 Task 派发并做 code review。

你希望用哪一种？如果你不回复，我将按 **1）本会话顺序执行** 开始。

