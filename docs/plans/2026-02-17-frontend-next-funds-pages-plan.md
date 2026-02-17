# Frontend-Next（Next.js）基金列表/详情迁移 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 `frontend-next/` 实现基金列表页 `/funds` 与基金详情页 `/funds/[fundCode]`，对齐旧前端的核心能力（分页搜索 + 批量估值/净值刷新 + 详情页基础信息与历史净值）。

**Architecture:** 继续复用 `frontend-next/src/lib/http.ts` 的 axios + refresh 重试；页面使用 client components（便于使用 antd、localStorage、交互状态）；把“日期范围计算 / 响应解析”提取为纯函数，用 vitest 做最小 TDD 覆盖。

**Tech Stack:** Next.js 16 App Router、React 19、Ant Design、Axios、Vitest（node 环境）。

---

### Task 1: 增加最小测试框架（Vitest）

**Files:**
- Modify: `frontend-next/package.json`
- Modify: `frontend-next/package-lock.json`
- Create: `frontend-next/vitest.config.ts`
- Create: `frontend-next/src/lib/__tests__/dateRange.test.ts`

**Step 1: 写一个会失败的测试（date range 规则）**

在 `dateRange.test.ts` 里先断言 `getDateRange("1W", now)` 计算正确（此时函数尚不存在，应 FAIL）。

**Step 2: 跑测试验证失败**

Run: `cd frontend-next; npm test`
Expected: FAIL（提示找不到模块/函数）。

**Step 3: 写最小实现让测试通过**

Create: `frontend-next/src/lib/dateRange.ts`，实现 `getDateRange(range, now)` 返回 `{startDate,endDate}`（YYYY-MM-DD）。

**Step 4: 再跑测试**

Run: `cd frontend-next; npm test`
Expected: PASS。

---

### Task 2: funds API 封装与响应解析（纯函数）

**Files:**
- Modify: `frontend-next/src/lib/api.ts`
- Create: `frontend-next/src/lib/funds.ts`
- Create: `frontend-next/src/lib/__tests__/funds.test.ts`

**Step 1: 先写失败测试**

- `normalizeFundList(json)` 支持 `{count,results}` 与 `[]`
- `mergeBatchNav(funds, batchNav)` 将 `latest_nav/latest_nav_date` 合并到列表

Run: `cd frontend-next; npm test`
Expected: FAIL。

**Step 2: 最小实现**

实现 `normalizeFundList / mergeBatchNav / mergeBatchEstimate` 等纯函数。

**Step 3: 再跑测试**

Expected: PASS。

---

### Task 3: 实现 `/funds` 列表页（分页 + 搜索 + 批量估值/净值）

**Files:**
- Create: `frontend-next/src/app/funds/page.tsx`
- (Optional) Create: `frontend-next/src/components/AuthedLayout.tsx`

**Step 1: 先跑 dev 手动验证（页面尚不存在应 404）**

Run: `cd frontend-next; npm run dev`
Expected: 访问 `/funds` 为 404。

**Step 2: 实现页面**

页面行为：
- 调 `GET /api/funds/?page=&page_size=&search=`
- 取本页 `fund_codes` 调：
  - `POST /api/funds/batch_estimate/`
  - `POST /api/funds/batch_update_nav/`
- Table 展示：代码、名称、最新净值、实时估值、估算涨跌、操作（查看详情）
- 顶部 Search + 刷新按钮（重新拉 batch）

**Step 3: 手动验证**

- 能分页/搜索
- 刷新后估值/净值列更新

---

### Task 4: 实现 `/funds/[fundCode]` 详情页（基础信息 + 历史净值）

**Files:**
- Create: `frontend-next/src/app/funds/[fundCode]/page.tsx`

**Step 1: 实现基础信息**

并发请求：
- `GET /api/funds/{fund_code}/`
- `GET /api/funds/{fund_code}/estimate/`（失败不阻断）

渲染：代码、名称、类型、最新净值（若有）、实时估值、估算涨跌。

**Step 2: 历史净值**

- 选择时间范围（1W/1M/3M/6M/1Y/ALL）
- 点击“同步并加载”：
  - `POST /api/nav-history/sync/`（fund_codes=[code]，start_date/end_date）
  - `GET /api/nav-history/?fund_code=...&start_date=...`
- Table 展示：日期、单位净值、累计净值（若有）。

---

### Task 5: 回归验证 + 集成

**Step 1: build**

Run: `cd frontend-next; npm run build`
Expected: exit 0。

**Step 2: Docker 冒烟**

Run（端口避免冲突）:
`cd ..; $env:COMPOSE_PROJECT_NAME="fundval-frontend-funds"; $env:FRONTEND_NEXT_HOST_PORT="19100"; $env:BACKEND_RS_HOST_PORT="19101"; docker compose up --build -d db-candidate backend-rs frontend-next`

Manual:
- `http://localhost:19100/funds` 可打开并能加载列表
- 点进详情页正常渲染

清理：`docker compose -p fundval-frontend-funds down -v`

**Step 3: commit / merge / push / cleanup**

同前序流程：在 worktree commit → merge 到 `main` → push → 删除 worktree & 分支。

