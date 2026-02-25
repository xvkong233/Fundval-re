# 任务队列 + 信号异步/分页 设计

**目标：**在 Fundval-re 中提供统一的“任务队列”入口，能查看队列中/运行中任务与最近 20 条已完成任务，并能查看每个任务 run 的日志；同时把基金“批量信号”从同步接口改为“后端异步任务 + 分页取回结果”，以避免一次性计算/查询过大导致超时或上游封锁。

## 背景与问题

- 当前系统存在两类“重任务”：
  - **爬虫/同步类**：`crawl_job`（净值历史、关联板块、估值等）
  - **计算/批量类**：例如嗅探页批量 ML 信号、可能的全市场计算任务
- 现有 UI 缺少统一的“队列/任务”可观测入口，排障依赖 Docker 日志。
- 批量信号同步接口会在高基金数量时拖慢响应甚至失败；需要改为异步并可分页拉取结果。

## 方案概览

### 数据层（统一可观测）

- 保留 `crawl_job`（队列本身不改）
- 新增通用任务表 `task_job`（计算/批量类任务）
- 新增统一 run 与日志表：
  - `task_run`：一次执行记录（可对应 `crawl_job` 或 `task_job`）
  - `task_run_log`：run 内的日志行
- 为异步批量信号新增结果表：
  - `fund_signals_batch_item(task_id, fund_code, source, as_of_date, best_peer_json, computed_at)`

### 执行层（worker）

- 在后端后台循环中：
  - 继续执行 `crawl_job`（已有）
  - 增加执行 `task_job`：`run_due_task_jobs(max_run)`（目前先支持 `task_type="signals_batch"`）
- 每个任务执行都会：
  - 创建 `task_run(queue_type, job_id, job_type, ...)`
  - 持续写 `task_run_log`（关键步骤、进度、错误）
  - 完成后写回 `task_run` 与 `task_job` 状态

### API（面向前端）

- 任务队列
  - `GET /api/tasks/overview`：队列中（crawl/task）、运行中 run、最近完成 run（默认 20）
  - `GET /api/tasks/runs/{run_id}/logs`：某次 run 的日志（默认 500 行）
- 异步信号
  - `POST /api/funds/signals/batch_async`：入队 `signals_batch`，返回 `{ task_id }`
  - `GET /api/funds/signals/batch_async/{task_id}`：分页返回 `{ status, error, total, done, page, page_size, items }`
  - `POST /api/funds/signals/batch`：兼容旧入口，改为等价的 async 入队（返回 `{ task_id }`）

### 前端（发布级可用的最小闭环）

- 左侧导航栏底部新增：`任务队列`（`/tasks`）
- 任务队列页：
  - 分区展示：爬虫队列、任务队列、运行中、最近完成
  - 点击 run 打开抽屉/弹窗查看日志（自动滚动到底、支持刷新）
- 嗅探页批量信号改为：
  - 先入队任务
  - 轮询任务状态 + 分页拉取结果
  - UI 提示“去任务队列查看日志/进度”

