# 预测曲线叠加历史净值图 + 全市场预测模型训练任务 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在基金详情页把「未来 60T 预测净值曲线（含区间/高低点/拐点）」叠加到同一张历史净值图，并提供一个显式的「训练全市场预测模型」任务入口（任务队列可查看训练日志）。

**Architecture:**  
- 前端 `buildNavChartOption()` 接收 `forecast`，将未来交易日日期拼接到历史 xAxis，预测曲线与区间以独立 series 叠加；预测高/低点与 swing points 通过 `markPoint` 标注。  
- 后端新增 `forecast_model_train` 任务类型与路由：一次点击入队 1 个任务；任务内部输出样本数/特征维度/训练耗时等日志，并 upsert `forecast_model`。

**Tech Stack:** Next.js + Ant Design + ECharts；Rust (axum + sqlx AnyPool)；Postgres/SQLite migrations。

---

### Task 1: 前端图表叠加（历史 + 未来）

**Files:**
- Modify: `frontend/src/lib/__tests__/navChart.test.ts`
- Modify: `frontend/src/lib/navChart.ts`

**Step 1: 写一个失败用例（RED）**
- 断言：传入 `forecast.points` 后，`option.xAxis.data` 会追加未来交易日（跳过周末），并存在 `series.name === "预测净值"`。

**Step 2: 运行用例确认失败**
- Run: `cd frontend; npm test -- navChart`
- Expected: FAIL（预测曲线未叠加/日期未追加）

**Step 3: 最小实现（GREEN）**
- 在 `buildNavChartOption()` 中：
  - 解析 `forecast.points`（step 从 1 开始）
  - `nextTradingDates()` 生成未来日期（跳过周末）
  - 追加预测 series（均值/上下界）
  - 用 `markPoint` 标注 `forecast.low/high/swing_points`

**Step 4: 运行测试确认通过**
- Run: `cd frontend; npm test -- navChart`
- Expected: PASS

---

### Task 2: 基金详情页合并展示

**Files:**
- Modify: `frontend/src/app/funds/[fundCode]/page.tsx`

**Step 1: 写/调整 UI 行为**
- `buildNavChartOption(navHistory, { ..., forecast })` 传入 `analysisV2.result.windows[0].forecast`
- 删除或收纳单独「预测走势」卡片，避免页面过长

**Step 2: 手动验证**
- Run: `cd frontend; npm run dev`
- Expected: 历史净值图上出现未来预测曲线与预测高低点标注

---

### Task 3: 后端「训练预测模型」任务 + 路由

**Files:**
- Modify: `backend/crates/api/src/routes/mod.rs`
- Create/Modify: `backend/crates/api/src/routes/forecast.rs`（或放在现有模块中）
- Modify: `backend/crates/api/src/tasks.rs`
- Test: `backend/crates/api/tests/forecast_model_train_task_test.rs`

**Step 1: 写失败测试（RED）**
- 调用 `POST /api/forecast/model/train`，期望 `202 {task_id}`。

**Step 2: 运行测试确认失败**
- Run: `cd backend; cargo test -p api --tests forecast_model_train_task_test`
- Expected: FAIL（路由不存在/任务类型不支持）

**Step 3: 最小实现（GREEN）**
- 新路由：校验登录 → `enqueue_task_job("forecast_model_train", payload)` → `tokio::spawn(run_due_task_jobs(...))`
- 新 executor：读取 payload（source/horizon/lag_k/model_name）→ 训练 → upsert `forecast_model` → 输出日志

**Step 4: 运行测试确认通过**
- Run: `cd backend; cargo test -p api --tests forecast_model_train_task_test`
- Expected: PASS

---

### Task 4: 任务队列页面添加训练入口

**Files:**
- Modify: `frontend/src/lib/api.ts`
- Modify: `frontend/src/app/tasks/page.tsx`

**Step 1: 写最小 API 方法**
- `trainForecastModel(payload?)` → `POST /forecast/model/train/`

**Step 2: Tasks 页新增按钮**
- 文案：`训练预测模型（全市场）`
- 点击：调用 API → `router.push(/tasks/{taskId})`

**Step 3: 手动验证**
- Run: `cd frontend; npm run dev`
- Expected: 点击后入队 1 个任务，进入详情可看到训练日志

