# Frontend-Next：仪表盘（/dashboard）Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 `frontend-next/src/app/dashboard/page.tsx` 从“开发中占位”升级为可用的总览页：展示账户总览（父账户汇总）、持仓/自选数量、最近操作流水，并提供快速入口。

**Architecture:** 页面为 client component，使用 `AuthedLayout` 统一布局；数据通过 `src/lib/api.ts` 调用 `listAccounts/listPositions/listPositionOperations/listWatchlists` 并在前端做简单聚合；不引入新的复杂组件（保持最小可用）。

**Tech Stack:** Next.js（App Router）、Ant Design、Axios、Vitest（已有）。

---

### Task 0: Worktree 依赖准备

**Step 1: 安装依赖**

Run: `cd frontend-next; npm ci`

**Step 2: 验证基线单测**

Run: `cd frontend-next; npm test`
Expected: PASS。

---

### Task 1: 实现仪表盘页面（最小可用）

**Files:**
- Modify: `frontend-next/src/app/dashboard/page.tsx`

**Step 1: 数据加载**

- 并发拉取：accounts、positions、operations、watchlists
- 失败提示：message.error（不阻塞其余模块）

**Step 2: 展示模块**

- 账户总览：父账户汇总（holding_cost/holding_value/pnl/today_pnl 等）
- 计数卡片：持仓数量、自选列表数量、操作流水数量
- 最近操作流水：展示最近 10 条（日期、类型、基金、金额/份额、回滚按钮不提供，避免权限复杂）
- 快捷入口按钮：基金/自选/账户/持仓

---

### Task 2: 验证

Run: `cd frontend-next; npm test && npm run build`
Expected: exit 0。

---

### Task 3: 集成

提交 → merge `main` → push → 删除 worktree 与分支。

