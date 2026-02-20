# Sniffer（嗅探）页 + 每日自动同步 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 新增“嗅探”页（排序/筛选），并每天 03:10（Asia/Shanghai）自动采集 DeepQ 星标数据并全量镜像同步到所有用户的 `嗅探（自动）` 自选组。

**Architecture:** 后端定时任务拉取 `https://sq.deepq.tech/star/api/data`（CSV），写入 `sniffer_snapshot/sniffer_item/sniffer_run` 表；随后为每个用户确保存在 `watchlist(name='嗅探（自动）')` 并用事务“删旧插新”镜像更新 `watchlist_item`。前端新增 `/sniffer` 页面从后端读取最新 snapshot 展示，并提供标签 AND 过滤、板块过滤、搜索与排序。

**Tech Stack:** Rust (axum/sqlx/tokio/reqwest), Postgres, Next.js 16 + antd Table

---

### Task 1: 数据库表（sniffer）

**Files:**
- Create: `.worktrees/sniffer/backend/migrations/20260220000007_create_sniffer_tables.sql`

**Step 1: 添加 migration（建表）**

建三张表：
- `sniffer_run`：记录每次同步的开始/结束、成功/失败、错误信息、条目数
- `sniffer_snapshot`：成功同步的快照（来源 URL、抓取时间、条目数）
- `sniffer_item`：快照下的基金条目（板块、标签、星级、涨幅/回撤等）

**Step 2: 运行后端测试（编译 + 宏展开会校验 migrations 路径）**

Run: `cd .worktrees/sniffer/backend; cargo test -p api`
Expected: `0 failed`

---

### Task 2: 后端 sniffer 同步实现（抓取 + 解析 + 落库）

**Files:**
- Create: `.worktrees/sniffer/backend/crates/api/src/sniffer.rs`
- Modify: `.worktrees/sniffer/backend/crates/api/src/lib.rs`
- Modify: `.worktrees/sniffer/backend/crates/api/src/state.rs`

**Step 1: 写解析单测（TDD）**

Create: `.worktrees/sniffer/backend/crates/api/tests/sniffer_csv_parse_test.rs`

用固定 CSV 片段断言：
- 能去除 BOM
- 能通过 header name 取列（不依赖列顺序）
- 能把 `★★★★★` 转成 `5`
- 能把 `强势中、涨得多、内部买` 分割成 tags 数组

Run: `cd .worktrees/sniffer/backend; cargo test -p api sniffer_csv_parse_test`
Expected: FAIL（缺实现）

**Step 2: 最小实现使测试通过**

在 `sniffer.rs` 实现：
- `fetch_deepq_csv(client) -> String`
- `parse_csv(text) -> Vec<SnifferRow>`

**Step 3: 追加“落库”单测（不连 DB 的部分用纯函数测）**

只测排序/规范化逻辑（例如默认排序 key 计算、tags 过滤 AND）。

**Step 4: 全量测试**

Run: `cd .worktrees/sniffer/backend; cargo test -p api`
Expected: PASS

---

### Task 3: 后端路由（items/status/admin sync）

**Files:**
- Create: `.worktrees/sniffer/backend/crates/api/src/routes/sniffer.rs`
- Modify: `.worktrees/sniffer/backend/crates/api/src/routes/mod.rs`
- Modify: `.worktrees/sniffer/backend/crates/api/src/lib.rs`

**Step 1: 写路由单测（shape）**

Create: `.worktrees/sniffer/backend/crates/api/tests/sniffer_routes_test.rs`

覆盖：
- 未登录访问 `/api/sniffer/items` 返回 401
- 登录但无快照时返回 200 + 空列表 + `has_snapshot=false`（或你实现的约定）

**Step 2: 实现路由**

端点建议：
- `GET /api/sniffer/status`：返回最后一次 run/snapshot 信息
- `GET /api/sniffer/items`：返回最新 snapshot + items + 聚合 tags/sectors
- `POST /api/admin/sniffer/sync`：管理员手动触发一次同步（用于排障）

**Step 3: 全量测试**

Run: `cd .worktrees/sniffer/backend; cargo test -p api`
Expected: PASS

---

### Task 4: 定时调度（03:10 Asia/Shanghai）

**Files:**
- Modify: `.worktrees/sniffer/backend/crates/api/src/main.rs`
- Modify: `.worktrees/sniffer/backend/crates/api/src/sniffer.rs`

**Step 1: 添加 scheduler**

在后端启动后 `tokio::spawn`：
- 计算下一次 03:10（+08:00 固定偏移）
- 到点执行 `run_sync_once(state)`（带 mutex 防并发）
- 如果系统首次启动且尚无快照：可选择立即跑一次（只在“无快照”时）

**Step 2: 运行 docker 本地验证**

Run: `cd .worktrees/sniffer; docker compose up -d --build`
Expected:
- `GET http://localhost:8001/api/sniffer/status` 有响应
- `POST http://localhost:8001/api/admin/sniffer/sync`（管理员 token）可触发同步

---

### Task 5: 前端嗅探页（排序/筛选/搜索）

**Files:**
- Create: `.worktrees/sniffer/frontend/src/app/sniffer/page.tsx`
- Modify: `.worktrees/sniffer/frontend/src/components/AuthedLayout.tsx`
- Modify: `.worktrees/sniffer/frontend/src/lib/api.ts`
- Test: `.worktrees/sniffer/frontend/src/lib/__tests__/entryRouting.test.ts`（或新增对应用例）

**Step 1: API 封装**

在 `api.ts` 增加：
- `getSnifferStatus()`
- `getSnifferItems()`

**Step 2: 页面实现**

用 antd：
- 顶部：最近同步时间、来源、条目数、手动刷新按钮（可选：仅 admin 显示）
- 过滤：板块 Select、多标签 Select（AND 语义）、搜索（名称/代码）
- 列表：Table 支持排序（星级、周涨幅、年涨幅等）

**Step 3: 前端测试 + build**

Run: `cd .worktrees/sniffer/frontend; npm test`
Run: `cd .worktrees/sniffer/frontend; npm run build`
Expected: PASS

---

### Task 6: 端到端验证（业务可用性）

**Files:**
- (Optional) Create: `.worktrees/sniffer/scripts/sniffer-smoke.ps1`

**Step 1: 冒烟**

1) 登录拿 token  
2) `POST /api/admin/sniffer/sync` 触发同步  
3) `GET /api/sniffer/items` 应返回 items  
4) `GET /api/watchlists` 应存在 `嗅探（自动）` 且 items 数量与 snapshot 一致（或按去重后一致）

---

**Done Criteria**
- `/sniffer` 页面可见且排序/筛选可用
- 后端每天 03:10 自动跑（并可手动触发验证）
- “嗅探（自动）”自选组对所有用户全量镜像更新
- `cargo test -p api`、`npm test`、`npm run build` 通过

