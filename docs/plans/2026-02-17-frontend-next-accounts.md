# Frontend-Next：账户管理（/accounts）Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 `frontend-next` 增加可用的账户管理页面：支持账户列表展示、创建/编辑/删除、父账户选择与汇总展示，并为后续 `Positions` 页面迁移打基础。

**Architecture:** 页面为 client component；数据通过 `frontend-next/src/lib/api.ts` 的 accounts 封装调用；账号列表按“父账户 + children”展示，避免把列表里“子账户顶层重复项”渲染两次；默认父账户选择逻辑抽成纯函数并用 Vitest 做 TDD。

**Tech Stack:** Next.js（App Router）、Ant Design、Axios、Vitest。

---

### Task 0: Worktree 依赖准备

**Step 1: 安装依赖**

Run: `cd frontend-next; npm ci`

**Step 2: 验证基线单测**

Run: `cd frontend-next; npm test`
Expected: PASS。

---

### Task 1: TDD — 默认父账户选择纯函数

**Files:**
- Create: `frontend-next/src/lib/accounts.ts`
- Test: `frontend-next/src/lib/__tests__/accounts.test.ts`

**Step 1: 写失败测试**

- 当存在 `is_default=true` 的父账户：返回其 `id`
- 否则：返回第一个父账户的 `id`
- 若无父账户：返回 `null`

Run: `cd frontend-next; npm test`
Expected: FAIL（函数不存在/导出不存在）。

**Step 2: 最小实现让测试通过**

Run: `cd frontend-next; npm test`
Expected: PASS。

---

### Task 2: 扩展 accounts API 封装

**Files:**
- Modify: `frontend-next/src/lib/api.ts`

**Add:**
- `listAccounts()` → `GET /accounts/`
- `createAccount(data)` → `POST /accounts/`
- `patchAccount(id,data)` → `PATCH /accounts/{id}/`
- `deleteAccount(id)` → `DELETE /accounts/{id}/`

---

### Task 3: 实现 `/accounts` 页面

**Files:**
- Create: `frontend-next/src/app/accounts/page.tsx`
- Modify: `frontend-next/src/components/AuthedLayout.tsx`

**Step 1: 页面骨架与加载**

- `AuthedLayout` 标题：`账户`
- `useEffect` 调用 `listAccounts()` 加载
- 仅以父账户作为下拉选项：`accounts.filter(a => !a.parent)`
- 选中父账户：用 Task 1 的纯函数做默认值

**Step 2: 创建/编辑/删除**

- Modal + Form（名称、父账户可选、是否默认）
- 创建成功：刷新列表 + 成功提示
- 编辑：允许修改名称/默认/父账户（最小可用）
- 删除：Popconfirm 二次确认

**Step 3: 展示与汇总**

- 模式切换：`全部账户汇总` / `返回单账户`
- 单账户模式：展示“父账户汇总统计 + 子账户表格（children）”
- 金额/百分比格式化：跟随旧前端的红涨绿跌规则（最小实现）

---

### Task 4: 验证

Run: `cd frontend-next; npm test && npm run build`
Expected: exit 0。

（可选 Docker 冒烟）：
Run: `.\scripts\compose-dev.ps1 down; .\scripts\compose-dev.ps1 up -Build`

---

### Task 5: 集成

提交 → merge `main` → push → 删除 worktree 与分支。

