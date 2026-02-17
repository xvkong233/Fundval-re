# Fund Detail Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 让 `frontend-next` 的基金详情页（`/funds/[fundCode]`）与旧前端对齐：自动加载历史净值（随时间范围切换刷新），并展示“我的持仓”和“操作记录（按 fund_code 过滤）”。

**Architecture:** 抽取纯函数到 `src/lib/fundDetail.ts`（过滤/排序/数值计算），UI 仅负责调用 API + 展示；Vitest 覆盖核心计算与排序逻辑。

**Tech Stack:** Next.js、Ant Design、TypeScript、Vitest

---

### Task 1: fundDetail 纯函数（TDD）

**Files:**
- Create: `frontend-next/src/lib/fundDetail.ts`
- Test: `frontend-next/src/lib/__tests__/fundDetail.test.ts`

**Step 1: Write the failing test**

```ts
import { describe, expect, test } from "vitest";
import { buildFundPositionRows } from "../fundDetail";

test("buildFundPositionRows calculates market value and pnl", () => {
  const rows = buildFundPositionRows(
    [
      { account_name: "A", fund_code: "000001", holding_share: "100", holding_cost: "1000", fund: { latest_nav: "12" } },
    ],
    "000001",
    "12"
  );
  expect(rows[0]).toMatchObject({ account_name: "A" });
});
```

**Step 2: Run test to verify it fails**

Run: `cd frontend-next && npm test -- src/lib/__tests__/fundDetail.test.ts`
Expected: FAIL（模块/导出不存在）

**Step 3: Minimal implementation**

实现 `buildFundPositionRows` / `filterPositionsByFund` / `sortOperationsDesc`。

**Step 4: Expand tests**

- 过滤仅保留 `fund_code` 匹配
- 市值/盈亏/盈亏率计算与排序
- 操作记录按日期/创建时间倒序排序

---

### Task 2: FundDetail 页面增强

**Files:**
- Modify: `frontend-next/src/app/funds/[fundCode]/page.tsx`

**Steps:**
- `timeRange` 改为变化即触发 `syncAndLoadNav`
- 增加 `positions`、`operations` 的加载与展示卡片
- 权限/无数据时不弹错误（与旧前端一致），以 Empty 或隐藏卡片处理

---

### Task 3: 验证与集成

- `cd frontend-next && npm test`
- `cd frontend-next && npm run build`
- 提交、推送、合并 `main`

