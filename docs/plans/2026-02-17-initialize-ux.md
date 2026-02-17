# Initialize 页面体验优化 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 优化 `frontend-next` 的 `/initialize`：对 `410 Gone（系统已初始化）`、网络失败等常见错误给出清晰反馈，并提供返回/跳转，初始化成功后引导至登录。

**Architecture:** 抽取一个纯函数层 `src/lib/bootstrapInit.ts` 来解析 axios-like 错误（含 status、后端 error 字段、网络错误），UI 仅消费解析结果来展示 `Result/Alert` 和控制步骤流转。关键分支用 Vitest 覆盖。

**Tech Stack:** Next.js App Router、Ant Design、TypeScript、Vitest

---

### Task 1: 错误解析工具（TDD）

**Files:**
- Create: `frontend-next/src/lib/bootstrapInit.ts`
- Test: `frontend-next/src/lib/__tests__/bootstrapInit.test.ts`

**Step 1: Write the failing test**

```ts
import { describe, expect, test } from "vitest";
import { getBootstrapInitError } from "../bootstrapInit";

describe("getBootstrapInitError", () => {
  test("maps 410 to already-initialized", () => {
    const err = { response: { status: 410, data: { error: "系统已初始化，接口失效" } } };
    expect(getBootstrapInitError(err)).toMatchObject({
      kind: "already_initialized",
      status: 410,
    });
  });
});
```

**Step 2: Run test to verify it fails**

Run: `cd frontend-next && npm test -- src/lib/__tests__/bootstrapInit.test.ts`
Expected: FAIL（模块不存在或导出不存在）

**Step 3: Write minimal implementation**

```ts
export type BootstrapInitErrorKind = "already_initialized" | "invalid_key" | "network" | "unknown";
export function getBootstrapInitError(error: unknown) { /* minimal */ }
```

**Step 4: Expand tests for key branches**

- `400` + `data.error` → `invalid_key`
- `response` 缺失（网络断开）→ `network`
- 其他 status / 结构 → `unknown`

**Step 5: Run tests**

Run: `cd frontend-next && npm test`
Expected: PASS

---

### Task 2: 初始化页交互与提示

**Files:**
- Modify: `frontend-next/src/app/initialize/page.tsx`

**Step 1: 写一个最小的 UI 变更清单（不写生产代码）**

- 捕获 `already_initialized`：在卡片内展示 `Result`（warning）+ “前往登录”
- 捕获 `network`：提示检查 `API_PROXY_TARGET` / 后端服务
- Step 1 增加 “返回修改密钥” 按钮，清空 key 并回到 Step 0
- Step 1 显示 “已验证密钥（脱敏）”
- Step 2 初始化成功后 `router.replace("/login")`（延迟 1-2s）并保留按钮

**Step 2: 实现 UI（仅消费 Task 1 的纯函数）**

**Step 3: 本地构建验证**

Run: `cd frontend-next && npm run build`
Expected: SUCCESS

---

### Task 3: 集成与推送

**Files:**
- Verify: `frontend-next` 相关改动

**Step 1: 运行完整验证**

- `cd frontend-next && npm test`
- `cd frontend-next && npm run build`

**Step 2: 提交与推送**

```bash
git add docs/plans/2026-02-17-initialize-ux.md frontend-next/src/lib/bootstrapInit.ts frontend-next/src/lib/__tests__/bootstrapInit.test.ts frontend-next/src/app/initialize/page.tsx
git commit -m "feat(frontend-next): improve initialize UX and error handling"
git push -u origin feat/initialize-ux
```

