# FundVal Rust + Next.js 移植 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在保持 `docs/API文档/*` API 契约 100% 不变的前提下，用 Rust + Next.js 完整移植 Web 端，并用“对照迁移”自动化测试证明一致性。

**Architecture:** 并行运行 `backend_py`（Django 真值基准）与 `backend_rs`（Rust 新实现），通过可重复的对照测试用例逐模块对齐；前端先按现有页面结构迁移到 Next.js，并通过 proxy/rewrite 指向可切换的后端。

**Tech Stack:** Rust（axum, sqlx, tokio, tracing, jsonwebtoken, argon2）、Next.js（React 19, Ant Design, ECharts）、PostgreSQL、Redis、Docker Compose。

---

## Phase 0：基础设施与目录结构

### Task 0.1：创建 Rust 后端工程骨架

**Files:**
- Create: `backend-rs/Cargo.toml`
- Create: `backend-rs/crates/api/Cargo.toml`
- Create: `backend-rs/crates/api/src/main.rs`
- Create: `backend-rs/crates/api/src/lib.rs`
- Create: `backend-rs/crates/api/src/routes/mod.rs`
- Create: `backend-rs/crates/api/src/routes/health.rs`

**Step 1: 写一个最小可跑的健康检查测试（先失败）**

Create: `backend-rs/crates/api/tests/health_test.rs`

```rust
#[tokio::test]
async fn health_returns_ok_shape() {
  // TODO: 调用路由函数并断言 JSON 字段
  assert!(true);
}
```

Expected: `cargo test` 能编译但测试先占位（下一步替换为真实断言）。

**Step 2: 实现 `/api/health/` 路由（最小实现）**

`GET /api/health/` 响应对齐文档：

```json
{ "status": "ok", "database": "connected|disconnected", "system_initialized": false }
```

**Step 3: 跑测试**

Run: `cd backend-rs && cargo test`

Expected: PASS（此时数据库探活可先返回 `disconnected`，但字段必须存在）。

**Step 4: 提交**

Run:
- `git add backend-rs`
- `git commit -m "feat(backend-rs): scaffold axum api with health endpoint"`

---

### Task 0.2：创建 Next.js 前端工程骨架

**Files:**
- Create: `frontend-next/package.json`（由 `create-next-app` 生成）
- Create: `frontend-next/next.config.js`（配置 rewrite `/api/:path*`）
- Create: `frontend-next/src/app/page.tsx`

**Step 1: 初始化 Next.js（推荐用 pnpm；也可 npm）**

Run:
- `npx create-next-app@latest frontend-next --ts --eslint --app --src-dir --no-tailwind`

Expected: 生成 Next.js App Router 项目。

**Step 2: 配置 `/api` rewrite**

在 `frontend-next/next.config.js` 添加：
- 将 `/api/:path*` 代理到环境变量 `NEXT_PUBLIC_API_BASE`（默认 `http://localhost:8000/api` 或 Nginx 统一入口）

**Step 3: 本地启动验证**

Run: `cd frontend-next && npm run dev`

Expected: 打开 `http://localhost:3000` 能看到首页。

**Step 4: 提交**

Run:
- `git add frontend-next`
- `git commit -m "feat(frontend-next): scaffold next.js app with api rewrites"`

---

### Task 0.3：Docker Compose 支持双后端并行

**Files:**
- Modify: `docker-compose.yml`
- Create: `backend-rs/Dockerfile`
- Create: `frontend-next/Dockerfile`
- (Optional) Create: `infra/nginx/conf.d/default.conf`

**Step 1: 新增服务**
- `backend_py`：沿用现有 `backend`（golden）
- `backend_rs`：Rust 服务，端口建议 `8001`
- `frontend_next`：Next.js（建议 `3000`）

**Step 2: 统一入口（两种方案二选一）**
- A（推荐）：Nginx 作为入口，`/api/` 反代到 `backend_rs` 或 `backend_py`（通过 env 选择上游）
- B：Next.js rewrite 直接指向后端容器名（开发场景 OK，生产不推荐）

**Step 3: 启动验证**

Run: `docker compose up --build`

Expected:
- `backend_py` 健康检查 OK
- `backend_rs` 可访问 `/api/health/`
- `frontend_next` 可打开首页

**Step 4: 提交**

Run:
- `git add docker-compose.yml backend-rs/Dockerfile frontend-next/Dockerfile infra/nginx`
- `git commit -m "chore(docker): run python and rust backends side-by-side"`

---

## Phase 1：对照测试框架（证明“100%一致”）

### Task 1.1：建立 API 对照测试工具（最小可用）

**Files:**
- Create: `tools/contract-tests/package.json`
- Create: `tools/contract-tests/src/run.ts`
- Create: `tools/contract-tests/src/diff.ts`
- Create: `tools/contract-tests/src/cases/health.ts`

**Step 1: 写第一个用例（health）**
- 分别请求 `backend_py` 与 `backend_rs` 的 `/api/health/`
- 对比：状态码、JSON keys、值类型
- 允许白名单字段差异（例如 `database` 值允许不同，但必须是 string 且在枚举内）

**Step 2: 在 docker-compose 下运行**

Run:
- `cd tools/contract-tests && npm i`
- `npm run test:contract`

Expected: 输出对照结果（health 用例通过）。

**Step 3: 提交**

Run:
- `git add tools/contract-tests`
- `git commit -m "test(contract): add golden-vs-rust comparator harness"`

---

## Phase 2：按模块对齐 API（每个模块一个“完成门槛”）

> 每个模块完成的定义：该模块所有接口在对照测试中 PASS；并在前端切换到 Rust 后端后做一轮人工验收。

### Task 2.1：系统管理（bootstrap/initialize）

**Files:**
- Create: `backend-rs/crates/api/src/routes/bootstrap.rs`
- Modify: `backend-rs/crates/api/src/routes/mod.rs`
- Create: `tools/contract-tests/src/cases/bootstrap.ts`

**Step 1: 增加对照用例（先失败）**
- 未初始化：verify/initialize 返回与 Django 一致
- 已初始化：返回 410

**Step 2: Rust 实现 verify/initialize**
- 生成 bootstrap_key（仅在未初始化时有效）
- 初始化成功后写入 DB 状态（`system_initialized=true`）并创建管理员

**Step 3: 跑对照**
Run: `npm run test:contract`
Expected: bootstrap 模块 PASS。

---

### Task 2.2：用户认证（login/refresh/me/password/register/summary）

**Files:**
- Create: `backend-rs/crates/api/src/routes/auth.rs`
- Create: `backend-rs/crates/api/src/routes/users.rs`
- Create: `backend-rs/migrations/*`（users 表等）
- Create: `tools/contract-tests/src/cases/auth.ts`

**Step 1: 用例先行（先失败）**
- 注册（allow_register=false 返回 403；true 返回 201 + token）
- 登录成功/失败
- 刷新 token
- me
- 修改密码
- summary

**Step 2: Rust 实现最小闭环**
- 数据库 schema + 密码哈希 + JWT 发行与校验
- refresh token 存储与失效策略（对齐行为）

**Step 3: 对照通过**

---

### Task 2.3：基金管理 / 账户管理 / 持仓管理 / 自选列表 / 数据源 / 历史净值

对每个模块重复以下模板：

**Files:**
- Create: `backend-rs/crates/api/src/routes/<module>.rs`
- Create: `tools/contract-tests/src/cases/<module>.ts`
- Modify: `backend-rs/migrations/*`

**Steps:**
1) 写对照用例（先失败）  
2) 写最小实现（先跑通 happy path）  
3) 补齐边界条件与错误码对齐  
4) 对照通过  
5) 前端切换验收  
6) 提交

---

## Phase 3：前端页面迁移到 Next.js

### Task 3.1：页面与路由映射

**Files:**
- Create: `frontend-next/src/app/(auth)/login/page.tsx`
- Create: `frontend-next/src/app/(auth)/register/page.tsx`
- Create: `frontend-next/src/app/initialize/page.tsx`
- Create: `frontend-next/src/app/funds/page.tsx`
- Create: `frontend-next/src/app/funds/[code]/page.tsx`
- Create: `frontend-next/src/app/accounts/page.tsx`
- Create: `frontend-next/src/app/positions/page.tsx`
- Create: `frontend-next/src/app/watchlists/page.tsx`
- Create: `frontend-next/src/app/settings/page.tsx`

**Step 1: 先搬运 UI，不改业务**
以现有 `frontend/src/pages/*.jsx` 为蓝本，优先做到功能一致（允许先不做 SSR）。

**Step 2: 统一 API Client**
- Create: `frontend-next/src/lib/api.ts`
- 处理 token 存取、401 跳转、错误提示

**Step 3: 与后端切换联动**
- 通过环境变量切换后端（默认 `/api`）

---

## Done Definition（最终验收）
- `docker compose up --build` 可一键启动
- `tools/contract-tests` 全部模块 PASS
- `frontend-next` 在仅启用 `backend_rs` 时可完整使用并通过人工验收

