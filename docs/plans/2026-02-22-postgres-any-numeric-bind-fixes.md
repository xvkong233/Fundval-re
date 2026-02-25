# Postgres + sqlx::Any 数值绑定修复 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复 Docker(Postgres) 下 `sqlx::AnyPool` 在数值/日期参数绑定时引发的 500 与 Postgres 类型错误，确保“同步后”可正常计算并展示专业指标、预测信号与性价比。

**Architecture:** 保持 sqlite/postgres 双后端兼容：对 Postgres 侧使用“参数按 TEXT 绑定 + 显式类型转换”的 SQL 形式，避免二进制绑定格式不兼容；对 sqlite 侧保持现有 CAST/DATE 逻辑。

**Tech Stack:** Rust (axum/sqlx AnyPool), PostgreSQL 16, SQLite

### Task 1: 修复 `fund_signal_snapshot` 写入与日期比较

**Files:**
- Modify: `backend/crates/api/src/ml/compute.rs:181`

**Steps:**
1. 将 `h.nav_date <= $3` 改为 `h.nav_date <= DATE($3)`，避免 `date <= text`。
2. `INSERT fund_signal_snapshot` 在 Postgres 分支使用 `CAST(CAST($n AS TEXT) AS DOUBLE PRECISION)` 写入数值列。
3. 跑 `cargo test -p api`。

### Task 2: 修复 `fund_relate_theme` upsert 的 Postgres 数值绑定

**Files:**
- Modify: `backend/crates/api/src/tiantian_h5.rs:158`

**Steps:**
1. Postgres SQL 使用 `CAST(CAST($n AS TEXT) AS DOUBLE PRECISION)` 写入 `corr_1y/ol2top`。
2. sqlite SQL 继续使用 `CAST($n AS REAL)`。
3. 跑 `cargo test -p api`。

### Task 3: 前端同步后自动刷新指标/信号

**Files:**
- Modify: `frontend/src/app/funds/[fundCode]/page.tsx:65`

**Steps:**
1. 增加 `navReloadKey`，每次“同步并加载”完成后递增。
2. 让 signals/analytics/value_score 相关 `useEffect` 依赖 `navReloadKey` 触发重算。
3. 确保 title 单行显示（字符串）。

### Task 4: Docker(Postgres) 验证

**Steps:**
1. `docker compose up -d --build backend frontend`
2. 调用 `/api/funds/{code}/signals` 与 `/api/funds/{code}/analytics` 验证返回 200。
3. 检查 `db-candidate` 日志不再出现 `incorrect binary data format` / `date <= text`。

