# 基金详情专业化 + 缓存分批爬取 + ML 预测信号 + 嗅探页重构 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.  
> **前置：** 在 `.worktrees/` 创建独立 worktree 分支后执行本计划（避免污染 `main`）。

**Goal:** 增强基金详情页专业分析能力，增加可运维的数据缓存调度，并在嗅探页提供可解释的购买建议与预测信号展示。  
**Architecture:** 后端新增“分析指标/无风险利率/缓存调度/信号缓存”的模块化路由；前端新增“专业概览 + 信号区块”并重构嗅探页布局；ML 采用离线训练产物 + 在线推理与缓存。  
**Tech Stack:** Rust（axum, sqlx AnyPool, tokio, reqwest）、Next.js（React 19, AntD 6, ECharts）、Python（离线训练）。  

---

## Milestone 0：分支与基线验证

### Task 0.1：创建 worktree 与分支

Run（PowerShell）：
- `git worktree add .worktrees/feat-fund-analytics -b feat/fund-analytics`
- `cd .worktrees/feat-fund-analytics`

Expected：目录存在，`git status -sb` 干净。

### Task 0.2：验证基线（必须先绿）

Run：
- `cd backend; cargo fmt --check`
- `cd backend; cargo clippy -p api -- -D warnings`
- `cd backend; cargo test -p api`
- `cd frontend; npm test`
- `cd frontend; npm run lint`
- `cd frontend; npm run build`

Expected：全部 PASS。若失败，先修基线再进入后续任务。

Commit：
- `git commit --allow-empty -m "chore: start fund analytics worktree baseline"`

---

## Milestone 1：无风险利率（3M 国债）爬取 + 缓存表

### Task 1.1：新增迁移（sqlite/postgres 同步）

**Files:**
- Create: `backend/migrations/sqlite/20260220000001_create_risk_free_rate.sql`
- Create: `backend/migrations/postgres/20260220000001_create_risk_free_rate.sql`

Schema（示例，按项目风格微调）：
- `risk_free_rate_daily(date, tenor, rate, source, fetched_at, created_at, updated_at)`
- 唯一键：`(date, tenor, source)`

**Step 1: 写迁移 smoke test（先失败）**

Create: `backend/crates/api/tests/migrations_risk_free_rate_test.rs`

```rust
#[test]
fn migrations_include_risk_free_rate() {
  // TODO: 读取 migrator 列表或用 sqlite in-memory 跑 migrate 并检查表存在
  assert!(true);
}
```

**Step 2: 运行测试确认失败/占位**

Run：`cd backend; cargo test -p api`

Expected：测试暂时占位（下一步替换为真实断言）。

**Step 3: 实现真实断言**

建议：使用 sqlite in-memory + `sqlx::migrate!()` 执行迁移后查询 `sqlite_master` / information_schema。

**Step 4: 跑测试**

Run：`cd backend; cargo test -p api`

Expected：PASS。

**Step 5: Commit**

- `git add backend/migrations backend/crates/api/tests`
- `git commit -m "feat(db): add risk_free_rate_daily table"`

### Task 1.2：实现 3M 国债利率抓取器（可替换数据源）

**Files:**
- Create: `backend/crates/api/src/rates/mod.rs`
- Create: `backend/crates/api/src/rates/treasury_3m.rs`
- Modify: `backend/crates/api/src/lib.rs`（导出模块）

**Step 1: 写单测（先失败）**

Create: `backend/crates/api/tests/treasury_3m_parse_test.rs`

```rust
#[test]
fn parse_treasury_3m_payload() {
  let payload = r#"{"date":"2026-02-20","rate":"1.80"}"#;
  // TODO: 调用 parse 函数并断言
  assert!(payload.contains("rate"));
}
```

**Step 2: 实现解析函数（最小实现）**

要求：
- 不在测试里真实联网
- 抓取函数与解析函数分离（解析函数纯函数可测）

**Step 3: 跑测试**

Run：`cd backend; cargo test -p api`

Expected：PASS。

**Step 4: Commit**

- `git add backend/crates/api/src/rates backend/crates/api/tests`
- `git commit -m "feat(rates): add 3m treasury rate fetcher parser"`

### Task 1.3：新增 rate API（读缓存 + 过期策略）

**Files:**
- Create: `backend/crates/api/src/routes/rates.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`
- Modify: `backend/crates/api/src/main.rs`（挂载路由）

Endpoints（建议）：
- `GET /api/rates/risk-free/?tenor=3M&date=YYYY-MM-DD`（返回 rate + fetched_at + source）

**Step 1: 合同形状测试（先失败）**

Create: `backend/crates/api/tests/rates_route_test.rs`

```rust
#[tokio::test]
async fn risk_free_rate_returns_shape() {
  // TODO: 调用 handler 并断言字段存在
  assert!(true);
}
```

**Step 2: 最小实现 + 通过测试**

**Step 3: Commit**

- `git add backend/crates/api/src/routes backend/crates/api/tests`
- `git commit -m "feat(api): add risk-free rate endpoint"`

---

## Milestone 2：基金分析指标计算 + 缓存（Sharpe/分位评分）

### Task 2.1：后端新增“分析计算”模块（纯函数优先）

**Files:**
- Create: `backend/crates/api/src/analytics/mod.rs`
- Create: `backend/crates/api/src/analytics/metrics.rs`
- Create: `backend/crates/api/tests/analytics_metrics_test.rs`

**Step 1: 写 Sharpe/最大回撤/波动 的单测（先失败）**

```rust
#[test]
fn sharpe_uses_rf_and_annualizes() {
  // TODO: 构造收益序列与 rf，断言 sharpe 合理
  assert!(true);
}
```

**Step 2: 最小实现**

约束：
- 输入：按日期排序的单位净值或收益序列（交易日）
- 输出：指标结构体（字符串/decimal 由上层格式化）

**Step 3: 跑测试**

Run：`cd backend; cargo test -p api`

Expected：PASS。

**Step 4: Commit**

- `git add backend/crates/api/src/analytics backend/crates/api/tests`
- `git commit -m "feat(analytics): add core metrics calculators"`

### Task 2.2：新增分析 API（基金详情页消费）

**Files:**
- Create: `backend/crates/api/src/routes/fund_analytics.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`
- Modify: `backend/crates/api/src/main.rs`

Endpoint（建议）：
- `GET /api/funds/{fund_code}/analytics/?range=252T&source=tiantian`

返回（示例）：
- `metrics`：Sharpe/Sortino/Calmar/vol/max_drawdown/ann_return/...
- `value_score`：score、percentile、components
- `rf`：rate、date、source、fetched_at
- `computed_at`

测试：
- 新增 handler shape test（不需要外网，使用 seed 数据或 sqlite in-memory）

Commit：
- `git commit -m "feat(api): add fund analytics endpoint"`

---

## Milestone 3：缓存调度器（分批爬取，自选优先）

### Task 3.1：新增 crawl_job / cache_meta 表 + 状态机

**Files:**
- Create: `backend/migrations/sqlite/20260220000002_create_crawl_jobs.sql`
- Create: `backend/migrations/postgres/20260220000002_create_crawl_jobs.sql`
- Create: `backend/crates/api/src/crawl/mod.rs`
- Create: `backend/crates/api/src/crawl/scheduler.rs`
- Create: `backend/crates/api/tests/crawl_scheduler_test.rs`

要求：
- 并发上限、QPS、失败指数退避
- 任务优先级：自选 > 持仓 > 最近访问 > 全量轮询

Commit：`feat(crawl): add job tables and scheduler skeleton`

### Task 3.2：把现有 sync 能力接入 scheduler（不改契约）

目标：复用现有 `syncNavHistory`/fund fetch 逻辑，以 job 驱动批量刷新。

Commit：`feat(crawl): integrate nav sync into scheduler`

---

## Milestone 4：ML（Python 离线训练 + 线上推理 + 缓存）

### Task 4.1：建立 Python 训练脚手架（最小可复现）

**Files:**
- Create: `ml/README.md`
- Create: `ml/requirements.txt`
- Create: `ml/train.py`
- Create: `ml/features.py`
- Create: `ml/export_model.py`
- Create: `ml/reports/`（gitkeep）

要求：
- walk-forward 切分
- 输出：模型文件 + metrics 报告（AUC/Brier/校准）

### Task 4.2：后端推理接口（先用基线模型/占位）

**Files:**
- Create: `backend/crates/api/src/ml/mod.rs`
- Create: `backend/crates/api/src/routes/fund_signals.rs`
- Tests: `backend/crates/api/tests/fund_signals_route_test.rs`

Commit：`feat(ml): add fund signals endpoint (baseline)`

### Task 4.3：信号写入缓存 + 嗅探购买建议聚合接口

**Files:**
- Create: `backend/crates/api/src/routes/sniffer_advice.rs`
- Create/Modify migrations for signal cache tables

Commit：`feat(api): add sniffer advice endpoint`

---

## Milestone 5：前端（基金详情页 + 嗅探页重构 + 移除基金入口）

### Task 5.1：左侧移除“基金”入口（不影响路由）

**Files:**
- Modify: `frontend/src/components/AuthedLayout.tsx`

测试：
- `npm run build` 通过

Commit：`chore(ui): remove funds entry from sidebar`

### Task 5.2：基金详情页新增“专业概览 + 信号 + 持有周期”

**Files:**
- Modify: `frontend/src/app/funds/[fundCode]/page.tsx`
- Modify: `frontend/src/lib/api.ts`（新增 analytics/signal API 封装）

要求：
- 显示 252T 作为默认窗口，可切换 60T/120T/252T
- 同类分位评分与分项拆解
- rf 3M 国债利率展示 + 更新时间

Commit：`feat(ui): add professional analytics to fund detail`

### Task 5.3：嗅探页深度重构 + 购买建议面板

**Files:**
- Modify: `frontend/src/app/sniffer/page.tsx`

实现要点：
- 12 栏布局：左筛选/表格，右建议面板
- 建议面板消费 `GET /api/sniffer/advice/`
- 表格支持列密度/排序/tooltip

Commit：`feat(ui): redesign sniffer page with advice panel`

---

## Milestone 6：收尾与发布

- 跑全量校验：`cargo fmt/clippy/test` + `npm test/lint/build`
- 更新 `CHANGELOG.md`
- 打 tag：`v1.3.0`（CI 仅在 tag 触发）

