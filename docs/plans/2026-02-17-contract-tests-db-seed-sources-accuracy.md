# Contract Tests DB Seed（sources_accuracy）Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 `tools/contract-tests` 增加可选的 Postgres DB seed 机制，使 `/api/sources/{source}/accuracy/` 能做稳定的“值级对照”（不仅仅是 schema 对照）。

**Architecture:** 在合同测试启动时（`tsx src/run.ts`）读取 `ENABLE_DB_SEED/GOLDEN_DB_URL/CANDIDATE_DB_URL`，分别连接 golden/candidate 两个 Postgres，写入一组固定数据（1 个 Fund + 2 条 EstimateAccuracy），然后在 `sources_accuracy` 用例中断言 record_count 与 avg_error_rate。

**Tech Stack:** Node.js 22 + TypeScript（tsx）、PostgreSQL、`pg` 驱动。

---

### Task 1: 添加 `pg` 依赖并锁定 lockfile

**Files:**
- Modify: `tools/contract-tests/package.json`
- Modify: `tools/contract-tests/package-lock.json`

**Step 1: 安装依赖（先本地生成 lockfile）**

Run: `cd tools/contract-tests; npm install -D pg @types/pg`

Expected:
- `package.json` 出现 `pg` 与 `@types/pg`
- `package-lock.json` 同步更新

---

### Task 2: 实现 seed 逻辑（golden + candidate）

**Files:**
- Create: `tools/contract-tests/src/seed.ts`
- Modify: `tools/contract-tests/src/run.ts`

**Step 1: 写 seed 模块（最小可用）**

- 当 `ENABLE_DB_SEED !== "true"` 时直接返回
- 当缺少 `GOLDEN_DB_URL` 或 `CANDIDATE_DB_URL` 时抛出清晰错误
- 对两个数据库分别执行：
  - upsert `fund`（fund_code 固定 `SEED0001`）
  - 清理 `estimate_accuracy` 中 `source_name="eastmoney"` 的记录（保证对照稳定）
  - 插入两条 `estimate_accuracy`（error_rate=0.018066 / 0.018067，期望平均值 0.0180665）

**Step 2: 在 `src/run.ts` 运行用例前调用**

- `await seedDatabases()` 放在 main() 的最前面（执行用例前）

---

### Task 3: sources_accuracy 增加值级对照断言（seed 模式）

**Files:**
- Modify: `tools/contract-tests/src/cases/sources_accuracy.ts`

**Step 1: 在 `ENABLE_DB_SEED === \"true\"` 时断言**

- `record_count === 2`
- `avg_error_rate` 为可解析数字
- `avg_error_rate` 接近 `0.0180665`（允许微小浮点误差）

---

### Task 4: Docker 合同测试验证（contract profile）

**Files:**
- Modify: `docker-compose.yml`（若缺 env 则补齐）

**Step 1: 启动并跑合同测试**

Run: `docker compose --profile contract up --build --exit-code-from contract-tests`

Expected:
- `contract-tests` 输出 `PASS sources_accuracy`
- 其它用例不因 seed 引入新的失败

**Step 2: 若失败，先最小定位**

- 只跑单用例：`docker compose --profile contract run --rm -e CASE_FILTER=sources_accuracy contract-tests`

---

### Task 5: 提交与推送准备

**Files:**
- Add/Modify: 上述变更文件

**Step 1: git status 确认仅包含预期文件**

Run: `git status -sb`

**Step 2: 提交**

Run:
- `git add docker-compose.yml tools/contract-tests docs/plans/2026-02-17-contract-tests-db-seed-sources-accuracy.md`
- `git commit -m "test(contract): seed db for deterministic sources accuracy"`

