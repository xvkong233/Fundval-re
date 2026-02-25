# 模拟盘（回测 + RL 环境）与 ML 强化实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 增加“基金（日频）模拟盘”功能（回测式 A + step 环境式 B），用于策略/强化学习训练；同时提升现有 ML 模块的效果与可解释性（更合理的验证、指标、特征、训练策略）。

**Architecture:** 后端新增 `sim` 模块（交易规则、撮合/确认/结算、状态推进、回测 runner、RL `reset/step` API），以数据库持久化模拟盘 run 与状态；前端新增“模拟盘”入口与详情页（净值曲线、订单/成交/持仓、RL step 调试）。ML 侧保持 Rust 纯实现：扩展特征、加时间切分验证与核心指标、早停/类不平衡处理，并把 metrics 写入 `ml_sector_model.metrics_json`。

**Tech Stack:** Rust (axum/sqlx), SQLite/Postgres migrations, Next.js/React + antd + echarts。

---

## 一、模拟盘规则（基金/日频，更贴近真实）

**核心选择（已确认）：**
- **下单在 T 日产生**，按 **T+1（下一条 NAV 日期）** 的 `unit_nav` 成交定价
- **赎回资金到账：T+2 个交易日**（以模拟盘交易日历推进）

**细化规则（一期默认，后续可配置）：**
- 交易日历：以所选标的在 `fund_nav_history` 的日期构建“全局交易日序列”，缺失 NAV 的标的用于估值时使用“最近一次 NAV”前向填充；但**成交定价必须命中该标的下一条 NAV 日期**，否则顺延。
- 申购（BUY）：T 日下单冻结现金；执行日 E 按 `nav(E)` 计算份额，扣申购费 `buy_fee_rate`；份额在 E 日确认后可用。
- 赎回（SELL）：T 日下单冻结份额；执行日 E 按 `nav(E)` 计算赎回金额，扣赎回费 `sell_fee_rate`；现金在 **E+2** 交易日到账（可用现金）。
- 估值：每日收盘（该日）权益 = 可用现金 + 冻结现金 + 待到账现金（应收） + Σ(份额可用/冻结 * 当日估值 NAV)。
- 约束：不可超买/超卖；未确认份额不可赎回；最小申购金额/最小赎回份额先做成可配置字段（默认不限制）。

---

## 二、数据模型与迁移

### Task 1: 新增模拟盘表（Postgres + SQLite）

**Files:**
- Create: `backend/migrations/postgres/20260221000003_create_sim_tables.sql`
- Create: `backend/migrations/sqlite/20260221000003_create_sim_tables.sql`
- Create Test: `backend/crates/api/tests/sim_migrations_test.rs`

**Tables (suggested minimal):**
- `sim_run`：run/env 配置与游标（user_id, mode, start/end, current_date, source, fees, settlement_days, calendar_json, status）
- `sim_position`：run 当前持仓（fund_code, shares_available, shares_frozen, avg_cost）
- `sim_cash_ledger`：run 现金分桶（cash_available, cash_frozen, cash_receivable + settle_date）
- `sim_order`：订单（trade_date, exec_date, side, amount/shares, fee, exec_nav, status）
- `sim_trade`：成交记录（exec_date, side, nav, shares, cash_delta, fee）
- `sim_daily_equity`：每日权益曲线（date, total_equity, cash_available, receivable, positions_value）

**Step 1: 写 migrations test（红）**
- 断言新表存在

**Step 2: 写 migrations（绿）**

---

## 三、后端模拟引擎与 API

### Task 2: 实现模拟盘引擎（撮合/确认/结算/估值）

**Files:**
- Create: `backend/crates/api/src/sim/mod.rs`
- Create: `backend/crates/api/src/sim/engine.rs`
- Create: `backend/crates/api/src/sim/db.rs`
- Test: `backend/crates/api/tests/sim_engine_test.rs`

**Step 1: 写引擎单测（红）**
- `step()`：T 日下单 → 下一交易日执行；SELL 现金应收并在 +2 日到账
- 约束：不可超买/超卖；冻结逻辑正确；权益计算不为负且可重复运行（幂等性）

**Step 2: 实现最小引擎（绿）**

### Task 3: 暴露 API（回测 + RL env）

**Files:**
- Create: `backend/crates/api/src/routes/sim.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`

**API (minimal):**
- `POST /api/sim/runs` 创建 run（mode=backtest），返回 id
- `POST /api/sim/runs/{id}/run` 执行回测（baseline 策略：buy-and-hold 等权），落 `sim_daily_equity`
- `GET  /api/sim/runs` / `GET /api/sim/runs/{id}` / `GET /api/sim/runs/{id}/equity` / `.../orders|trades|positions`
- `POST /api/sim/envs` 创建 env（mode=env），返回初始 observation
- `POST /api/sim/envs/{id}/step` 输入 actions（orders），返回 {observation,reward,done,date}

**Step 1: 写路由测试（红）**
- 需要 sqlite in-memory + migrations
- create → step → done 的 happy path

**Step 2: 实现路由（绿）**

---

## 四、前端页面（模拟盘）

### Task 4: 前端新增“模拟盘”入口、列表、详情与 env 调试

**Files:**
- Modify: `frontend/src/components/AuthedLayout.tsx`
- Modify: `frontend/src/lib/api.ts`
- Create: `frontend/src/app/sim/page.tsx`
- Create: `frontend/src/app/sim/[id]/page.tsx`
- (Optional) Create: `frontend/src/app/sim/[id]/env/page.tsx`

**Step 1: 列表页**
- 创建 run/env 表单（fund_codes、日期、初始资金、费率）
- run 列表与跳转

**Step 2: 详情页**
- 净值曲线（echarts）
- 持仓/订单/成交表格（antd Table）

**Step 3: env 调试页**
- 显示 observation JSON
- 手工下单并 step

**Verification:**
- `cd frontend; npm test && npm run lint && npm run build`

---

## 五、ML 强化（效果 + 可解释性）

### Task 5: 训练/评估升级（时间切分验证 + 指标 + 早停 + 类不平衡）

**Files:**
- Create: `backend/crates/api/src/ml/metrics.rs`
- Modify: `backend/crates/api/src/ml/train.rs`
- Modify: `backend/crates/api/src/ml/logreg.rs`
- Modify: `backend/crates/api/src/ml/dataset.rs`
- Modify: `backend/crates/api/src/ml/compute.rs`
- Tests: `backend/crates/api/tests/ml_metrics_test.rs`、必要时更新现有 `ml_train_test.rs`

**Changes:**
- 样本按 `as_of_date` 时间排序，做 80/20 时间切分（避免泄漏）
- 输出并持久化：AUC、LogLoss、Brier、Precision@K、Recall@K（K=20% 可配）
- 训练：学习率衰减 / 早停（以 val LogLoss 为准）/ 类权重（正例稀少时）
- 特征：在不引入外部库的前提下扩充（多窗口收益/波动/回撤/动量），并在 `compute_features()` 保持一致
- 兼容：旧模型特征维度不匹配时视为缺失并触发重训（或返回 None）

**Verification:**
- `cd backend; cargo fmt --check; cargo clippy -p api --all-targets -- -D warnings; cargo test -p api`

