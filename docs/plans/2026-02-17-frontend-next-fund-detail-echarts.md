# Frontend-Next（Next.js）基金详情历史净值图表（ECharts）Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 `frontend-next/src/app/funds/[fundCode]/page.tsx` 增加历史净值折线图（ECharts），对齐旧前端的“历史净值可视化 + 时间区间切换 + 同步并加载”体验。

**Architecture:** 保持页面为 client component；ECharts 组件使用 `next/dynamic` 禁用 SSR；把图表 option 的生成提取为纯函数 `buildNavChartOption()` 并用 Vitest 覆盖（TDD）。

**Tech Stack:** Next.js 16、Ant Design、echarts + echarts-for-react、Vitest。

---

### Task 1: TDD 图表 option 生成函数

**Files:**
- Test: `frontend-next/src/lib/__tests__/navChart.test.ts`
- Create: `frontend-next/src/lib/navChart.ts`

**Step 1: 写失败测试**

断言 `buildNavChartOption(rows)` 能生成：
- `xAxis.data` 为日期数组
- `series[0].type` 为 `line`
- `series[0].data` 为净值数值数组

Run: `cd frontend-next; npm test`
Expected: FAIL（找不到模块/函数）。

**Step 2: 写最小实现**

实现 `buildNavChartOption()`：清洗数据、生成 axis/series/grid。

Run: `cd frontend-next; npm test`
Expected: PASS。

---

### Task 2: 集成到基金详情页

**Files:**
- Modify: `frontend-next/src/app/funds/[fundCode]/page.tsx`
- Modify: `frontend-next/package.json`
- Modify: `frontend-next/package-lock.json`

**Step 1: 增加依赖**

- `echarts`
- `echarts-for-react`

**Step 2: 动态导入 ECharts 组件**

在详情页用：
- `dynamic(() => import("echarts-for-react"), { ssr: false })`

**Step 3: 渲染折线图**

当 `navHistory.length > 0` 时：
- 先渲染图表，再渲染表格
- 颜色使用 `theme.useToken().token.colorPrimary`
- 小屏旋转 xAxis 标签（compact 模式）

---

### Task 3: 验证

**Step 1: 本地测试与构建**

Run: `cd frontend-next; npm test && npm run build`
Expected: exit 0。

**Step 2: Docker 冒烟**

Run:
`$env:COMPOSE_PROJECT_NAME="fundval-frontend-chart"; $env:FRONTEND_NEXT_HOST_PORT="19300"; $env:BACKEND_RS_HOST_PORT="19301"; docker compose up --build -d db-candidate backend-rs frontend-next`

Check:
- `http://localhost:19300/funds/000001` 返回 200（页面可加载）

Cleanup:
- `docker compose -p fundval-frontend-chart down -v`

