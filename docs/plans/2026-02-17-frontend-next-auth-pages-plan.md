# Frontend-Next（Next.js）初始化/登录/注册迁移 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在 `frontend-next/` 中落地最小可用闭环：根据 `/api/health/` 判断是否已初始化 → 未初始化走 `/initialize` → 已初始化走 `/login`/`/register` → 成功登录后进入 `/dashboard`（先占位），并确保请求全部通过 Next.js `/api/*` rewrite 代理到 `backend-rs`。

**Architecture:** 复用现有 `frontend/` 的 UI 与交互（Ant Design 组件 + 既有字段/文案），在 Next.js App Router 下以 client components 实现；用 `axios` + localStorage 保存 token，并通过 interceptor 处理 401 的 refresh 重试（与旧前端一致）；用 `AuthProvider` 在根布局注入用户状态。

**Tech Stack:** Next.js 16（App Router）+ React 19、Ant Design、Axios、TypeScript。

---

### Task 1: 引入依赖与基础 Providers

**Files:**
- Modify: `frontend-next/package.json`
- Modify: `frontend-next/package-lock.json`
- Create: `frontend-next/src/app/providers.tsx`
- Modify: `frontend-next/src/app/layout.tsx`

**Step 1: 添加依赖（对齐旧前端）**

- `antd`
- `@ant-design/icons`
- `axios`

Run: `cd frontend-next; npm ci`
Expected: 安装完成且无致命错误。

**Step 2: 添加全局 Providers**

- `AuthProvider`（client）
- 可选：`ConfigProvider`（先保持默认 theme）

---

### Task 2: Auth & API 封装（localStorage + refresh）

**Files:**
- Create: `frontend-next/src/lib/auth.ts`
- Create: `frontend-next/src/lib/api.ts`
- Create: `frontend-next/src/lib/http.ts`
- (Optional) Create: `frontend-next/src/lib/types.ts`

**Step 1: 迁移 token 存取工具**

- `setToken/getToken/clearToken/isAuthenticated`
- `setUser/getUser/logout`

**Step 2: axios 实例与拦截器**

- baseURL：`/api`
- request：带上 `Authorization: Bearer <access_token>`
- response：401 时用 `refresh_token` 调 `/api/auth/refresh` 刷新并重试；失败则清 token 并跳 `/login`

**Step 3: API 函数**

- `verifyBootstrapKey` → `POST /api/admin/bootstrap/verify`
- `initializeSystem` → `POST /api/admin/bootstrap/initialize`
- `login` → `POST /api/auth/login`
- `register` → `POST /api/users/register/`
- `getCurrentUser` → `GET /api/auth/me`

---

### Task 3: 页面迁移（/ /initialize /login /register /dashboard）

**Files:**
- Modify: `frontend-next/src/app/page.tsx`
- Create: `frontend-next/src/app/initialize/page.tsx`
- Create: `frontend-next/src/app/login/page.tsx`
- Create: `frontend-next/src/app/register/page.tsx`
- Create: `frontend-next/src/app/dashboard/page.tsx`

**Step 1: `/` 路由跳转逻辑**

- 调 `/api/health/` 读取 `system_initialized`
- 未初始化：push 到 `/initialize`
- 已初始化且已登录：push 到 `/dashboard`
- 已初始化但未登录：push 到 `/login`

**Step 2: `/initialize`**

- 迁移旧版 Steps + 表单交互
- `verifyBootstrapKey` 成功 → 进入下一步
- `initializeSystem` 成功 → 显示完成页并引导去 `/login`

**Step 3: `/login`**

- 登录成功保存 token + user，跳 `/dashboard`

**Step 4: `/register`**

- 注册成功保存 token + user，跳 `/dashboard`

**Step 5: `/dashboard`（占位）**

- 简单展示“已登录用户信息（若有）”与后续导航占位

---

### Task 4: 验证

**Step 1: Next.js build**

Run: `cd frontend-next; npm run build`
Expected: `exit 0`，无 TS/ESLint 阻断错误。

**Step 2: Docker 端到端启动（可选但推荐）**

Run: `docker compose up --build -d backend-rs frontend-next`

Manual check:
- 打开 `http://localhost:3000/`
- 可看到自动跳转到 `/initialize`（空库未初始化时）
- 完成初始化后可登录进入 `/dashboard`

---

### Task 5: 集成到 main

**Step 1: commit**

Run: `git add frontend-next docs/plans/2026-02-17-frontend-next-auth-pages-plan.md; git commit -m "feat(frontend-next): add initialize/login/register flow"`

**Step 2: merge + push**

Run: `git checkout main; git pull; git merge wip/frontend-next-auth; git push origin main`

**Step 3: cleanup**

Run: `git worktree remove .worktrees/frontend-next-auth; git branch -d wip/frontend-next-auth`

