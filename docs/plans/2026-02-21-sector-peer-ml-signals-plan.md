# 关联板块同类（sector peer）ML 信号 + 详情页/嗅探页接入 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 基于“关联板块同类”与全量净值序列，输出并展示两套窗口（`5T + 20T`）的基金信号：位置（偏低/中等/偏高）+ 概率（回撤抄底/神奇反转），并在嗅探页产出购买建议（可解释、非投资建议）。

**Architecture:** 后端新增 `ml_sector_model` 与 `fund_signal_snapshot` 两张缓存表；后台任务负责（1）按板块训练轻量逻辑回归模型（L2 正则，特征标准化）并缓存（2）按基金计算当日信号并写入快照；前端基金详情页读取快照并同时展示 `5T/20T`，嗅探页购买建议改为读取信号聚合；位置分桶阈值固定为 `20/60/20`。

**Tech Stack:** Rust（axum, sqlx AnyPool, tokio）、SQLite/Postgres、Next.js（React 19, AntD 6）。

---

## Milestone 0：在现有 worktree 上执行

目标分支：`feat/sector-peer-ml`（worktree：`.worktrees/feat-sector-peer-ml`）。

基线验证（必须先绿）：
- `cd backend; cargo fmt --check`
- `cd backend; cargo clippy -p api -- -D warnings`
- `cd backend; cargo test -p api`
- `cd frontend; npm test`
- `cd frontend; npm run lint`
- `cd frontend; npm run build`

---

## Milestone 1：DB Schema（模型 + 信号快照）

### Task 1.1：新增迁移（sqlite/postgres 同步）

**Files:**
- Create: `backend/migrations/sqlite/20260221000002_create_ml_sector_model_and_signal_snapshot.sql`
- Create: `backend/migrations/postgres/20260221000002_create_ml_sector_model_and_signal_snapshot.sql`

建议 Schema（按现有风格微调）：
- `ml_sector_model`：
  - `peer_code TEXT`（关联板块代码，示例：`SEC_CODE`）
  - `task TEXT`（`dip_buy` / `magic_rebound`）
  - `horizon_days INTEGER`（5/20）
  - `feature_names_json TEXT`
  - `model_json TEXT`（权重/均值/方差/截距）
  - `metrics_json TEXT`（AUC/Brier/样本数等）
  - `trained_at TIMESTAMP`
  - 唯一键：`(peer_code, task, horizon_days)`
- `fund_signal_snapshot`：
  - `fund_code TEXT`
  - `peer_code TEXT`
  - `as_of_date DATE`
  - `position_percentile_0_100 REAL`
  - `position_bucket TEXT`（`low|medium|high`）
  - `dip_buy_proba_5t REAL` / `dip_buy_proba_20t REAL`
  - `magic_rebound_proba_5t REAL` / `magic_rebound_proba_20t REAL`
  - `computed_at TIMESTAMP`
  - 唯一键：`(fund_code, peer_code, as_of_date)`

**Step 1: 写迁移 smoke test（先失败）**

Create: `backend/crates/api/tests/ml_migrations_test.rs`

断言：
- 迁移后存在 `ml_sector_model` 与 `fund_signal_snapshot`

**Step 2: 运行测试确认 FAIL**

Run：`cd backend; cargo test -p api --test ml_migrations_test`

Expected：FAIL（表不存在）。

**Step 3: 写迁移并让测试 PASS**

Run：`cd backend; cargo test -p api --test ml_migrations_test`

Commit：
- `git add backend/migrations backend/crates/api/tests/ml_migrations_test.rs`
- `git commit -m "feat(db): add ml_sector_model and fund_signal_snapshot tables"`

---

## Milestone 2：后端 ML 核心（特征、标签、逻辑回归）

### Task 2.1：实现轻量逻辑回归（不引入重依赖）

**Files:**
- Create: `backend/crates/api/src/ml/mod.rs`
- Create: `backend/crates/api/src/ml/logreg.rs`
- Test: `backend/crates/api/tests/logreg_test.rs`

**Step 1: 写失败测试（可学习 + 可预测）**

测试用一个可分的数据集（例如 OR / 线性可分）：
- 训练后 `predict_proba` 对正样本 > 负样本
- 序列化/反序列化（json）后结果一致

Run：`cd backend; cargo test -p api --test logreg_test`
Expected：FAIL。

**Step 2: 最小实现**

要求：
- L2 正则
- 特征标准化（mean/std），推理同样标准化
- 固定随机性（不依赖随机源）

Run：`cd backend; cargo test -p api --test logreg_test`
Expected：PASS。

Commit：`git commit -m "feat(ml): add lightweight logistic regression (train+infer)"`

### Task 2.2：定义信号口径（位置分桶 20/60/20 + 标签阈值）

**Files:**
- Create: `backend/crates/api/src/ml/signals.rs`
- Test: `backend/crates/api/tests/signal_defs_test.rs`

**Step 1: 写失败测试（分桶边界）**

断言：
- percentile=20 => `low`
- percentile=21..80 => `medium`
- percentile=81 => `high`

Run：`cd backend; cargo test -p api --test signal_defs_test`
Expected：FAIL。

**Step 2: 实现**

Run：`cd backend; cargo test -p api --test signal_defs_test`
Expected：PASS。

Commit：`git commit -m "feat(ml): add signal definitions and bucket rules"`

---

## Milestone 3：训练集构建 + 模型训练（按关联板块）

### Task 3.1：训练样本构建（同类=关联板块；触发=最深20%）

**Files:**
- Create: `backend/crates/api/src/ml/dataset.rs`
- Test: `backend/crates/api/tests/ml_dataset_test.rs`

**Step 1: 写失败测试（用小型内存 sqlite seed）**

Seed：
- 2 个板块、每个板块 3 只基金、每只基金 40 个交易日 NAV

断言：
- 能产生样本（`n_samples > 0`）
- 只产出“触发集”样本（回撤深度分位在最深 20%）
- 标签计算符合：
  - `dip_buy_success`：未来 `h` 日收益 `> 0`
  - `magic_rebound`：未来 `h` 日最大反弹阈值（`5T>=3%` / `20T>=8%`）

Run：`cd backend; cargo test -p api --test ml_dataset_test`
Expected：FAIL。

**Step 2: 最小实现**

建议实现（先能跑通，再优化）：
- 只对 `as_of_date` 做**周频采样**（例如每 5 个交易日取一次）避免样本爆炸
- 位置/回撤深度：
  - 回撤深度使用 `lookback=252T` 的 rolling max 计算 `dd_mag`
  - 触发集按板块当日 `dd_mag` 排序取 top 20%

Run：`cd backend; cargo test -p api --test ml_dataset_test`
Expected：PASS。

Commit：`git commit -m "feat(ml): build sector-peer dataset with trigger+labels"`

### Task 3.2：训练与入库（ml_sector_model）

**Files:**
- Create: `backend/crates/api/src/ml/train.rs`
- Test: `backend/crates/api/tests/ml_train_test.rs`

**Step 1: 写失败测试（训练后入库并可取回推理）**

断言：
- 调用 `train_sector_models(peer_code, ...)` 后 `ml_sector_model` 有记录
- 从表取回模型 json 反序列化后可推理

Run：`cd backend; cargo test -p api --test ml_train_test`
Expected：FAIL。

**Step 2: 实现 + PASS**

Commit：`git commit -m "feat(ml): train sector models and persist to db"`

---

## Milestone 4：信号快照计算 + API + 嗅探页购买建议接入

### Task 4.1：计算 fund_signal_snapshot（读取模型 + 写快照）

**Files:**
- Create: `backend/crates/api/src/ml/compute.rs`
- Test: `backend/crates/api/tests/ml_compute_snapshot_test.rs`

断言：
- 给定 fund_code + peer_code + 最新 NAV，能写入 `fund_signal_snapshot`
- 输出同时包含 `5T/20T` 两套概率

Commit：`git commit -m "feat(ml): compute and store fund signal snapshots"`

### Task 4.2：新增 API（基金详情页消费）

**Files:**
- Create: `backend/crates/api/src/routes/fund_signals.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`
- Modify: `backend/crates/api/src/main.rs`
- Test: `backend/crates/api/tests/fund_signals_route_test.rs`

Endpoint（建议）：
- `GET /api/funds/{fund_code}/signals?source=tiantian`

返回：
- `as_of_date`
- `peer_code/peer_name`
- `position_percentile/bucket`
- `dip_buy`：`{ p_5t, p_20t }`
- `magic_rebound`：`{ p_5t, p_20t }`

Commit：`git commit -m "feat(api): add fund signals endpoint"`

---

## Milestone 5：调度集成（分批优先）+ 前端改版（详情页/嗅探页/左侧导航）

### Task 5.1：crawl job 新增训练/快照任务（自选/持仓/嗅探优先）

**Files:**
- Modify: `backend/crates/api/src/crawl/scheduler.rs`
- Modify: `backend/crates/api/src/crawl/worker.rs`
- Tests: `backend/crates/api/tests/crawl_scheduler_test.rs`

新增 job_type：
- `ml_sector_train`：按 peer_code 训练（优先覆盖自选/持仓/嗅探涉及的板块）
- `ml_signal_compute`：按 fund_code 计算信号快照

Commit：`git commit -m "feat(crawl): add ml training and signal compute jobs"`

### Task 5.2：基金详情页展示信号（两套都显示，默认 20T）

**Files:**
- Modify: `frontend/src/lib/api.ts`
- Modify: `frontend/src/app/funds/[fundCode]/page.tsx`

UI 规则：
- 同时展示 `5T + 20T`，默认高亮 `20T`
- 同时展示“位置桶（偏低/中等/偏高）”与“预测概率（%）”
- 标注 `as_of_date` 与免责声明

Commit：`git commit -m "feat(ui): show ML signals (5T+20T) on fund detail"`

### Task 5.3：嗅探页深度重构 + 购买建议接入 ML 信号

**Files:**
- Modify: `frontend/src/app/sniffer/page.tsx`
- Modify: `frontend/src/lib/snifferAdvice.ts`

要求：
- 购买建议从“规则占位”升级为：结合 `value_score`（板块同类）+ `signals`（抄底/反转概率）输出 `买入/观望/回避`
- UI：12 栏高密度仪表盘布局（参考 `design-system/fundval/pages/sniffer.md`）

Commit：`git commit -m "feat(ui): refactor sniffer page and ML-based buying advice"`

### Task 5.4：左侧导航移除“基金”入口

**Files:**
- Modify: `frontend/src/app/components/AuthedLayout.tsx`（或实际侧栏所在组件）

Commit：`git commit -m "feat(ui): remove funds entry from left nav"`

---

## Milestone 6：验证 + 合并发布

Run：
- `cd backend; cargo fmt --check`
- `cd backend; cargo clippy -p api -- -D warnings`
- `cd backend; cargo test -p api`
- `cd frontend; npm test`
- `cd frontend; npm run lint`
- `cd frontend; npm run build`

合并前：
- PR 合并 `feat/sector-peer-ml` → `main`
- 打 tag 触发 CI（仅 tag 触发已在 `main` 生效）

