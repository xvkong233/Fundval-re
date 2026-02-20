# 基金性价比评分（value_score+CE）+ 短期交易策略（趋势优先双策略）Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在基金详情页新增“同类分位综合分(value_score) + 经济学确定性等价(CE, γ可配置)”并给出多窗口（60T/120T/252T）短期交易策略（趋势/均值回归双策略，趋势优先），同时为嗅探页购买建议升级提供可复用的后端接口。

**Architecture:** 后端新增纯函数评分/策略模块，路由层组装数据并返回“总分+分项拆解+策略信号”；评分按 `fund_type` 同类对比，使用交易日 NAV 序列；CE 使用 `rf`（3M 国债）并支持 `gamma` 参数；短线策略仅依赖 NAV 序列与波动/回撤特征，输出可解释信号与触发原因。前端在基金详情页新增“性价比”与“短线策略”卡片。

**Tech Stack:** Rust（axum, sqlx AnyPool, chrono, rust_decimal）、Next.js（React 19, AntD 6）、SQLite/Postgres migrations。

---

## Milestone 0：分支与基线验证

### Task 0.1：创建 worktree 与分支

Run（PowerShell）：
- `git worktree add .worktrees/feat-value-score -b feat/value-score`
- `cd .worktrees/feat-value-score`

Expected：`git status -sb` 干净。

### Task 0.2：验证基线（必须先绿）

Run：
- `cd backend; cargo fmt --check`
- `cd backend; cargo clippy -p api -- -D warnings`
- `cd backend; cargo test -p api`
- `cd frontend; npm test`
- `cd frontend; npm run lint`
- `cd frontend; npm run build`

Expected：全部 PASS。

Commit：
- `git commit --allow-empty -m "chore: start value score worktree baseline"`

---

## Milestone 1：后端 value_score（同类分位综合分）与 CE（γ可配置）

### Task 1.1：新增评分纯函数模块（先测试）

**Files:**
- Create: `backend/crates/api/src/analytics/value_score.rs`
- Modify: `backend/crates/api/src/analytics/mod.rs`
- Test: `backend/crates/api/tests/value_score_test.rs`

**Step 1: 写失败测试（同类分位 + 方向一致）**

Create `backend/crates/api/tests/value_score_test.rs`（示例断言）：

```rust
#[test]
fn value_score_ranks_better_sharpe_higher() {
    // 构造 3 个基金同类样本：更高 sharpe/更低 mdd 应得更高分
    // 断言 percentile/score 单调
    assert!(true);
}
```

Run：`cd backend; cargo test -p api --test value_score_test`
Expected：FAIL（模块不存在）。

**Step 2: 最小实现**

实现要点（纯函数，便于测）：
- 输入：一组“同类样本”的指标向量（每个基金：ann_return、ann_vol、max_drawdown、sharpe、calmar 等；以及该基金在其中的索引）
- 输出：
  - `score_0_100`（0–100）
  - `percentile_0_100`（越大越好）
  - `components`：每个指标的分位/加权贡献
- 分位计算：
  - 对“越大越好”的指标（如 sharpe、calmar、ann_return）用 rank percentile
  - 对“越小越好”的指标（如 max_drawdown、ann_vol）用反向 rank percentile
- 权重（先常量，后续再开放配置）：
  - sharpe 0.35 / calmar 0.25 / ann_return 0.20 / max_drawdown 0.10 / ann_vol 0.10（可微调）
- 缺失指标处理：缺失则不计入该项权重并做归一化，避免 NaN。

**Step 3: 跑测试**

Run：`cd backend; cargo test -p api --test value_score_test`
Expected：PASS。

**Step 4: Commit**

Run：
- `git add backend/crates/api/src/analytics backend/crates/api/tests/value_score_test.rs`
- `git commit -m "feat(analytics): add value_score calculator"`

### Task 1.2：实现 CE（确定性等价）与参数 gamma

**Files:**
- Create: `backend/crates/api/src/analytics/ce.rs`
- Modify: `backend/crates/api/src/analytics/mod.rs`
- Test: `backend/crates/api/tests/ce_test.rs`

**Step 1: 写失败测试（gamma 增大 -> CE 下降）**

```rust
#[test]
fn ce_decreases_when_gamma_increases() {
    assert!(true);
}
```

**Step 2: 最小实现**

口径（交易日收益序列）：
- 日收益 `r_t = nav_t/nav_{t-1}-1`
- 年化超额收益：`ann_excess = mean(r_t)*252 - rf_ann`
  - `rf_ann`：`rf_percent/100`（3M 年化近似；先用“年化利率常数”）
- 年化方差：`ann_var = var(r_t)*252`
- `CE = ann_excess - 0.5 * gamma * ann_var`

输出：
- `ce`（数值）
- `ce_percentile`（在同类样本内的分位，作为“经济学性价比”）

**Step 3: 跑测试 + Commit**

Run：`cd backend; cargo test -p api --test ce_test`
Commit：`git commit -m "feat(analytics): add certainty-equivalent (CE) calculator"`

---

## Milestone 2：后端 API：基金详情“性价比 + CE + 短线策略”接口

### Task 2.1：扩展现有 analytics endpoint 返回 value_score + CE（多窗口）

**Files:**
- Modify: `backend/crates/api/src/routes/fund_analytics.rs`
- Modify: `backend/crates/api/src/analytics/metrics.rs`（如需补充 ann_return/calmar 等）
- Test: `backend/crates/api/tests/fund_analytics_value_score_test.rs`

**Step 1: 写失败路由测试（返回字段 shape）**

断言：
- 返回包含 `value_score` 与 `ce` 字段
- 支持 `range=60T|120T|252T`
- 支持 query `gamma=...`

Run：`cd backend; cargo test -p api --test fund_analytics_value_score_test`
Expected：FAIL。

**Step 2: 最小实现（不访问外网）**

实现要点：
- 读取基金 NAV 序列（已有）
- 计算本基金指标（已有 + 必要补充）
- 按 `fund_type` 找“同类基金集合”
  - 同类集合大小做保护：不足 `N_MIN=30` 时返回 `value_score=null` 并给 `reason`
- 为同类集合批量计算指标（可先做“只算 sharpe/mdd/vol/ann_return”的轻量版本）
  - 性能：先限制同类样本上限（比如 800）避免大类爆炸；后续再加缓存表
- 调用 `value_score` + `ce` 计算器
- 返回：
  - `value_score: { score_0_100, percentile_0_100, components, fund_type, sample_size }`
  - `ce: { gamma, ce_value, percentile_0_100 }`

**Step 3: 跑测试 + Commit**

Commit：`git commit -m "feat(api): add value_score and CE to fund analytics"`

---

## Milestone 3：短期交易策略（趋势优先双策略）与“合成建议”

### Task 3.1：新增策略纯函数模块（先测试）

**Files:**
- Create: `backend/crates/api/src/analytics/short_term.rs`
- Modify: `backend/crates/api/src/analytics/mod.rs`
- Test: `backend/crates/api/tests/short_term_strategy_test.rs`

**Step 1: 写失败测试（趋势优先规则）**

测试场景：
- 强趋势上行：趋势策略应为“偏低/可追随”，均值回归可能提示“偏高”，合成建议仍趋势优先
- 震荡：趋势弱，均值回归信号生效（高抛低吸区间）

**Step 2: 最小实现**

输出结构（可解释）：
- `trend`: { signal: low/medium/high, score, reasons[] }
- `mean_reversion`: { signal, score, reasons[] }
- `combined`: { signal, action_hint, conflict, rationale }

建议实现（先可解释再优化）：
- 趋势分：多窗口动量（20T/60T）、均线斜率、回撤是否创新低等组合
- 回归分：价格偏离均线 z-score、回撤深度分桶（如 >8%、>12%）、近端波动放大等
- 合成（趋势优先）：
  - 若趋势分 >= 阈值（例如 0.7）：combined 跟随趋势
  - 否则 combined 主要取回归分

**Step 3: 测试通过 + Commit**

Commit：`git commit -m "feat(analytics): add short-term strategy (trend first)"`

### Task 3.2：把短线策略挂到基金 analytics 接口（多窗口）

**Files:**
- Modify: `backend/crates/api/src/routes/fund_analytics.rs`
- Test: `backend/crates/api/tests/fund_analytics_short_term_test.rs`

要求：
- `range` 决定序列窗口（60T/120T/252T）
- `short_term` 始终返回（若数据不足则 return null + reason）

Commit：`git commit -m "feat(api): include short-term strategy in fund analytics"`

---

## Milestone 4：前端基金详情页展示（多窗口 + γ可调 + 短线建议）

### Task 4.1：API 封装扩展（gamma + 新字段）

**Files:**
- Modify: `frontend/src/lib/api.ts`
- Modify: `frontend/src/lib/fundAnalytics.ts`
- Test: `frontend/src/lib/__tests__/fundAnalytics.test.ts`

**Step 1: 写失败测试**
- 断言请求 URL 包含 `gamma`
- 断言解析 `value_score/ce/short_term`

**Step 2: 实现 + 跑测试 + Commit**

Commit：`git commit -m "feat(frontend): extend fund analytics client for value_score/CE/strategy"`

### Task 4.2：基金详情页新增两张卡片

**Files:**
- Modify: `frontend/src/app/funds/[fundCode]/page.tsx`

UI 要点：
- 多窗口切换：60T/120T/252T（与现有 trading-days 映射一致）
- γ 可调：Slider（例如 1–10，步长 0.5）+ 当前值显示；默认 3
- 展示：
  - value_score：总分 + 同类百分位 + 分项拆解（Tooltip）
  - CE：CE 值（年化）+ CE 分位 + γ 值说明
  - 短线策略：趋势/回归/合成建议（趋势优先），并列显示 reasons

Commit：`git commit -m "feat(ui): show value_score CE and short-term strategy on fund detail"`

---

## Milestone 5：验证、合并与发布

### Task 5.1：全量验证

Run：
- `cd backend; cargo fmt --check`
- `cd backend; cargo clippy -p api -- -D warnings`
- `cd backend; cargo test -p api`
- `cd frontend; npm test`
- `cd frontend; npm run lint`
- `cd frontend; npm run build`

Expected：全部 PASS。

### Task 5.2：合并与清理

Run：
- `git push -u origin feat/value-score`
- 在 `main` 上 merge（或开 PR）
- `git worktree remove .worktrees/feat-value-score`
- 删除本地/远端分支（若已合并）

### Task 5.3：打 tag 触发发布 CI

建议：
- 更新 `CHANGELOG.md` 的 `[Unreleased]`
- `git tag -a v2.0.3 -m "v2.0.3"`
- `git push origin v2.0.3`

