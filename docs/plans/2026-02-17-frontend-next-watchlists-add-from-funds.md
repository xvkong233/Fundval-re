# Frontend-Next：基金列表页“添加到自选” Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 `frontend-next/src/app/funds/page.tsx` 增加“添加到自选”按钮：点击后弹出 Modal，选择自选列表并把基金加入；当没有自选列表时引导去 `/watchlists`。

**Architecture:** 复用 `src/lib/http.ts` 的 axios（自动带 Bearer）；watchlists 的 API 调用封装到 `src/lib/api.ts`；把“默认选中哪个 watchlist”提取成纯函数并用 Vitest 做 TDD 覆盖。

**Tech Stack:** Next.js 16（client components）、Ant Design、Axios、Vitest。

---

### Task 1: TDD 默认选择逻辑（纯函数）

**Files:**
- Test: `frontend-next/src/lib/__tests__/watchlists.test.ts`
- Create: `frontend-next/src/lib/watchlists.ts`

**Step 1: 写失败测试**

- `pickDefaultWatchlistId([])` → `null`
- `pickDefaultWatchlistId([{id:"a"}])` → `"a"`

Run: `cd frontend-next; npm test`
Expected: FAIL（缺模块/函数）。

**Step 2: 最小实现**

实现 `pickDefaultWatchlistId(watchlists)`。

Run: `cd frontend-next; npm test`
Expected: PASS。

---

### Task 2: watchlists API 封装

**Files:**
- Modify: `frontend-next/src/lib/api.ts`

**Step 1: 增加接口函数**

- `listWatchlists()` → `GET /api/watchlists/`
- `createWatchlist(name)` → `POST /api/watchlists/`
- `addWatchlistItem(id, fundCode)` → `POST /api/watchlists/{id}/items/`

---

### Task 3: 基金列表页添加“添加到自选”弹窗

**Files:**
- Modify: `frontend-next/src/app/funds/page.tsx`

**Step 1: UI 与交互**

- 操作列增加按钮（Star）
- 点击后：
  - 拉取 watchlists
  - 若为空：提示并跳转 `/watchlists`
  - 否则弹窗选择列表（Select）并确认添加

**Step 2: 错误提示对齐**

后端 400/404/401 时显示 `error.response.data.error`（若有）否则通用提示。

---

### Task 4: 新增 `/watchlists` 占位页（避免引导死链）

**Files:**
- Create: `frontend-next/src/app/watchlists/page.tsx`
- Modify: `frontend-next/src/components/AuthedLayout.tsx`（菜单加入口）

---

### Task 5: 验证

**Step 1: 本地**

Run: `cd frontend-next; npm test && npm run build`
Expected: exit 0。

**Step 2: Docker 冒烟**

Run:
`$env:COMPOSE_PROJECT_NAME="fundval-frontend-watchlists"; $env:FRONTEND_NEXT_HOST_PORT="19500"; $env:BACKEND_RS_HOST_PORT="19501"; docker compose up --build -d db-candidate backend-rs frontend-next`

Check:
- `http://localhost:19500/funds` 返回 200

Cleanup:
- `docker compose -p fundval-frontend-watchlists down -v`

