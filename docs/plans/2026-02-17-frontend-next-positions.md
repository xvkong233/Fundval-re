# Frontend-Next：持仓管理（/positions）Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 `frontend-next` 增加可用的持仓管理页面：子账户选择、持仓列表展示、操作流水展示、建仓/加仓/减仓（创建操作）、回滚（删除操作）、按需刷新估值/净值，并从账户页可跳转到该页面。

**Architecture:** 页面为 client component；数据通过 `frontend-next/src/lib/api.ts` 的 positions/operations 封装调用；账户数据复用 `GET /accounts/`（只显示子账户）；默认选中子账户逻辑提取为纯函数并用 Vitest TDD；加载持仓后可选地调用 `batch_update_nav + batch_estimate` 以刷新 fund 字段。

**Tech Stack:** Next.js（App Router）、Ant Design、Axios、Vitest。

---

### Task 0: Worktree 依赖准备

**Step 1: 安装依赖**

Run: `cd frontend-next; npm ci`

**Step 2: 验证基线单测**

Run: `cd frontend-next; npm test`
Expected: PASS。

---

### Task 1: TDD — 默认子账户选择纯函数

**Files:**
- Create: `frontend-next/src/lib/positions.ts`
- Test: `frontend-next/src/lib/__tests__/positions.test.ts`

**Step 1: 写失败测试**

- 传入 accounts（包含父/子账户），只在子账户集合中选择：
  - 若 `preferredId` 是子账户：返回该 id
  - 否则：返回第一个子账户 id
  - 若无子账户：返回 `null`

Run: `cd frontend-next; npm test`
Expected: FAIL（函数不存在/导出不存在）。

**Step 2: 最小实现让测试通过**

Run: `cd frontend-next; npm test`
Expected: PASS。

---

### Task 2: 扩展 positions API 封装（positions + operations + query_nav）

**Files:**
- Modify: `frontend-next/src/lib/api.ts`

**Add:**
- `listPositions(params?: { account?: string })` → `GET /positions/`
- `listPositionOperations(params?: { account?: string; fund_code?: string })` → `GET /positions/operations/`
- `createPositionOperation(data)` → `POST /positions/operations/`
- `deletePositionOperation(id)` → `DELETE /positions/operations/{id}/`
- `recalculatePositions(accountId?: string)` → `POST /positions/recalculate/`
- `queryFundNav(payload)` → `POST /funds/query_nav/`（用于根据日期/15点前后自动填充 nav）

---

### Task 3: 实现 `/positions` 页面

**Files:**
- Create: `frontend-next/src/app/positions/page.tsx`
- Modify: `frontend-next/src/components/AuthedLayout.tsx`
- Modify: `frontend-next/src/app/accounts/page.tsx`

**Step 1: 菜单入口与跳转**

- `AuthedLayout` 增加菜单项：`持仓` → `/positions`
- `accounts` 子账户行增加“持仓”按钮：跳转 `/positions?account=<id>`

**Step 2: 页面骨架**

- 顶部：子账户 Select（label 显示 `parent_name / child_name` 或仅 child）
- 左侧/上方按钮：刷新、重算持仓（管理员）等
- 持仓表格：fund_code、fund_name、latest_nav、estimate_nav、estimate_growth、holding_share、holding_cost、pnl 等（最小可用）

**Step 3: 操作流水**

- 表格展示 operations（按 operation_date desc + created_at desc）
- 回滚按钮：调用 `deletePositionOperation`，成功后 reload positions + operations（403 显示无权限）

**Step 4: 创建操作（建仓/加仓/减仓）**

- Modal 表单字段：account（当前子账户，隐藏或只读）、fund_code（AutoComplete 搜索 `/funds/?search=`）、operation_type(BUY/SELL)、operation_date、before_15、amount、share、nav
- 当 fund_code + operation_date + before_15 可用时：调用 `queryFundNav` 自动填充 nav（失败显示提示，不阻塞手填）
- 提交成功：关闭 modal，reload positions + operations

**Step 5: 刷新估值/净值**

- 持仓加载后/或手动按钮：对 positions 的 fund_codes 调 `batch_update_nav + batch_estimate`，合并进 `position.fund.*`

---

### Task 4: 验证

Run: `cd frontend-next; npm test && npm run build`
Expected: exit 0。

（可选 Docker 冒烟，务必先清旧容器）：
Run: `.\scripts\compose-dev.ps1 down; .\scripts\compose-dev.ps1 up -Build`

---

### Task 5: 集成

提交 → merge `main` → push → 删除 worktree 与分支。

