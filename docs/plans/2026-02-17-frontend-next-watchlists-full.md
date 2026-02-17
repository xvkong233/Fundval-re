# Frontend-Next：完整自选列表（/watchlists）Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 `frontend-next/src/app/watchlists/page.tsx` 从占位升级为可用版本：展示自选列表与 items；支持创建/重命名/删除；支持搜索基金并添加；支持移除基金；支持调整顺序并调用 `/api/watchlists/{id}/reorder/` 保存。

**Architecture:** 页面为 client component；数据通过 `src/lib/api.ts` 封装调用；估值/净值刷新复用 `batch_estimate` 与 `batch_update_nav`；排序逻辑提取为纯函数并用 Vitest 做 TDD 覆盖。

**Tech Stack:** Next.js 16（App Router）、Ant Design、Axios、Vitest、Docker Compose 冒烟。

---

### Task 1: 修复 `.gitignore` 对 `frontend-next/src/lib` 的误伤（可选但推荐）

**Files:**
- Modify: `.gitignore`

**Step 1: 添加 negate 规则**

让 `frontend-next/src/lib/**` 的新文件可正常 `git add`（无需 `-f`）。

---

### Task 2: TDD — 排序纯函数

**Files:**
- Test: `frontend-next/src/lib/__tests__/watchlistsOrder.test.ts`
- Modify: `frontend-next/src/lib/watchlists.ts`

**Step 1: 写失败测试**

- `moveInArray([a,b,c], 2, 0)` → `[c,a,b]`
- `clampIndex(-1,len)` / `clampIndex(len,len)` 行为
- `getFundCodes(rows)` 只返回 `fund_code` 数组

Run: `cd frontend-next; npm test`
Expected: FAIL。

**Step 2: 最小实现让测试通过**

Run: `cd frontend-next; npm test`
Expected: PASS。

---

### Task 3: 扩展 watchlists API 封装

**Files:**
- Modify: `frontend-next/src/lib/api.ts`

**Add:**
- `patchWatchlist(id,name)` → `PATCH /watchlists/{id}/`
- `deleteWatchlist(id)` → `DELETE /watchlists/{id}/`
- `removeWatchlistItem(id,fundCode)` → `DELETE /watchlists/{id}/items/{fund_code}/`
- `reorderWatchlist(id,fundCodes)` → `PUT /watchlists/{id}/reorder/`

---

### Task 4: 实现完整 `/watchlists` 页面

**Files:**
- Modify: `frontend-next/src/app/watchlists/page.tsx`

**Step 1: Tabs 展示 watchlists**

- 左侧/顶部 Tabs：每个 watchlist 一个 tab
- tab label 提供：重命名按钮、删除按钮（Popconfirm）

**Step 2: 内容区**

- 添加基金：AutoComplete（搜索 `/funds/?search=`）+ 添加按钮（调用 `addWatchlistItem`）
- items 表：代码、名称、最新净值、估值、涨跌、操作（移除、上移、下移）
- “保存排序”按钮：调用 `reorderWatchlist(activeId, fundCodes)`，成功后 reload
- “刷新估值/净值”按钮：对当前 watchlist 的 fund_codes 调 batch 接口并合并

---

### Task 5: 验证

**Step 1: 本地**

Run: `cd frontend-next; npm ci && npm test && npm run build`
Expected: exit 0。

**Step 2: Docker 冒烟**

Run:
`$env:COMPOSE_PROJECT_NAME="fundval-frontend-watchlists-full"; $env:FRONTEND_NEXT_HOST_PORT="19700"; $env:BACKEND_RS_HOST_PORT="19701"; docker compose up --build -d db-candidate backend-rs frontend-next`

Check:
- `/watchlists` 返回 200

Cleanup:
- `docker compose -p fundval-frontend-watchlists-full down -v`

---

### Task 6: 集成

提交 → merge `main` → push → 删除 worktree 与分支。

