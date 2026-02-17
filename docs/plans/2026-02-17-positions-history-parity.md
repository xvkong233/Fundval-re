# Positions History Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 补齐旧版前端依赖的 `/api/positions/history/`，并在 Next.js 持仓页提供最小可用的“账户历史市值/成本”趋势图。

**Architecture:** Rust 端新增纯函数计算模块 `api::position_history`（单元测试覆盖核心回放逻辑），路由 `GET /api/positions/history/` 负责鉴权/校验/查询 DB 并调用纯函数；前端新增 ECharts option builder 与持仓页卡片调用。

**Tech Stack:** Axum + SQLx + rust_decimal + chrono；Next.js 16 + Ant Design + echarts-for-react + Vitest。

---

### Task 1: 后端纯函数 + 单测（TDD）

**Files:**
- Create: `backend-rs/crates/api/src/position_history.rs`
- Modify: `backend-rs/crates/api/src/lib.rs`

**Verify:**
- Run: `cd backend-rs && cargo test -p api position_history::tests`

---

### Task 2: 后端路由 `/api/positions/history/`

**Files:**
- Modify: `backend-rs/crates/api/src/routes/mod.rs`
- Modify: `backend-rs/crates/api/src/routes/positions.rs`

**Behavior parity:**
- 缺少 `account_id` → `400` + `{ "error": "缺少 account_id 参数" }`
- 非子账户（父账户）→ `400` + `{ "error": "暂不支持父账户历史查询" }`
- 账户不存在/不属于当前用户 → `404`
- 无操作流水 → `200` + `[]`
- 默认 `days=30`，返回 `days + 1` 条（含今天）

**Verify:**
- Run: `cd backend-rs && cargo test -p api`

---

### Task 3: 前端图表 option builder + 单测（Vitest）

**Files:**
- Create: `frontend-next/src/lib/positionHistoryChart.ts`
- Test: `frontend-next/src/lib/__tests__/positionHistoryChart.test.ts`

**Verify:**
- Run: `cd frontend-next && npm test`

---

### Task 4: 持仓页展示趋势图 + 调用 API

**Files:**
- Modify: `frontend-next/src/lib/api.ts`
- Modify: `frontend-next/src/app/positions/page.tsx`

**Verify:**
- Run: `cd frontend-next && npm run lint`
- Run: `cd frontend-next && npm run build`

---

### Task 5: 更新 API 文档

**Files:**
- Modify: `docs/API文档/05-持仓管理.md`

---

### Task 6: （可选）Docker 合同测试验证

**Before running:** 创建任何新容器前，先删除旧容器（仅删容器/网络，不删 volume）。

**Verify (example):**
- List: `docker ps -a`
- Clean old: `docker rm -f <container>`
- Run contract tests: `docker compose up -d --build backend-rs frontend-next contract-tests`

