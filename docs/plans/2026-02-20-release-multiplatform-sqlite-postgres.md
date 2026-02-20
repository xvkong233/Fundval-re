# Fundval 多平台发行 + Postgres/SQLite 双数据库 支持实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 打 tag 发布时同时产出 Windows/Linux/macOS 发行包（Windows 含 MSI/EXE/绿色压缩包），并在发行版默认使用 SQLite；Docker 方案保留且默认 Postgres，同时镜像推送到 GHCR + Docker Hub；后端运行时通过 `DATABASE_URL` 在 Postgres/SQLite 间切换。

**Architecture:** 后端采用单一二进制，通过解析 `DATABASE_URL`（或缺省落到本地 SQLite 文件）选择数据库，并在启动时按数据库类型执行对应 migrations。Docker Compose 显式提供 Postgres `DATABASE_URL`，发行包不提供 `DATABASE_URL` 从而默认 SQLite。

**Tech Stack:** Rust + axum + sqlx（postgres + sqlite + any + migrate），Next.js standalone，GitHub Actions，Docker Buildx，(Windows) WiX Toolset + Inno Setup。

---

### Task 1: 为后端引入 `sqlx` SQLite/Any 能力（RED/GREEN）

**Files:**
- Create: `backend/crates/api/tests/sqlite_any_smoke_test.rs`
- Modify: `backend/Cargo.toml`

**Step 1: Write the failing test**

在 `backend/crates/api/tests/sqlite_any_smoke_test.rs` 写一个最小测试：使用 `sqlx::any::AnyPoolOptions` 连接 `sqlite::memory:` 并执行 `SELECT 1`。

**Step 2: Run test to verify it fails**

Run: `cd backend && cargo test -p api sqlite_anypool_can_connect_and_query -q`
Expected: 失败（缺少 sqlite driver / feature 未启用）。

**Step 3: Write minimal implementation**

在 `backend/Cargo.toml` 的 `sqlx` features 增加 `sqlite` 与 `any`（保留 `postgres`）。

**Step 4: Run test to verify it passes**

Run: `cd backend && cargo test -p api sqlite_anypool_can_connect_and_query -q`
Expected: PASS

---

### Task 2: 后端启动时支持两类 migrations（Postgres/SQLite）

**Files:**
- Move: `backend/migrations/*.sql` -> `backend/migrations/postgres/*.sql`
- Add: `backend/migrations/sqlite/*.sql`
- Modify: `backend/crates/api/src/main.rs`

**Steps:**
1. 将现有 Postgres migrations 移入 `backend/migrations/postgres/`。
2. 新增/完善 `backend/migrations/sqlite/`（UUID/NUMERIC/DATE/TIMESTAMPTZ 等在 SQLite 采用 TEXT/NUMERIC/DATE/DATETIME）。
3. 在 `main.rs` 中按数据库类型选择对应 `sqlx::migrate!(...)` 并执行。

**Verification:**
- SQLite: `DATABASE_URL=sqlite::memory:` 运行 migrations 不报错（单测中创建 pool 后执行 migrator）。
- Postgres: Docker Compose 启动能正常跑 migrations（手工或 CI service 验证）。

---

### Task 3: 运行时选择数据库（发行版默认 SQLite / Docker 默认 Postgres）

**Files:**
- Create: `backend/crates/api/src/db.rs`
- Modify: `backend/crates/api/src/main.rs`
- Modify: `backend/crates/api/src/state.rs`
- Modify: `backend/crates/api/src/config.rs`

**Steps:**
1. 解析 `DATABASE_URL`：
   - 未设置时：默认 `sqlite:<data-dir>/fundval.sqlite`（`FUNDVAL_DATA_DIR` 优先，否则当前目录 `./data`）。
   - 设置为 `postgres://`/`postgresql://`：Postgres。
   - 设置为 `sqlite:`：SQLite。
2. Postgres：保留“数据库不存在则创建”的逻辑（仅对 Postgres 执行）。
3. AppState 存 `AnyPool`（或 Option）并提供 `pool()` 访问器。
4. 配置文件路径也跟随 `FUNDVAL_DATA_DIR`（发行包不写到根目录）。

**Verification:**
- `DATABASE_URL` 未设置：后端启动后创建 `./data/fundval.sqlite` 并可访问 `/api/health/`。
- Docker Compose：保持使用 Postgres（无需额外改用户行为）。

---

### Task 4: 路由查询改造为跨库 SQL（以 SQLite 为基准回归）

**Files:**
- Modify: `backend/crates/api/src/routes/*.rs`（重点：`funds.rs` / `accounts.rs` / `positions.rs` / `watchlists.rs` / `nav_history.rs`）

**Strategy:**
- 使用 `sqlx::AnyPool`/`sqlx::Transaction<'_, sqlx::Any>`。
- 彻底移除 Postgres-only 语法：
  - `NOW()` -> `CURRENT_TIMESTAMP`
  - `ILIKE` -> `LOWER(x) LIKE LOWER($n)`
  - `::text` -> `CAST(x AS TEXT)`
  - `COUNT(*)::bigint` -> `COUNT(*)`
  - `RETURNING (xmax = 0)` -> 通过 `SELECT` 预查是否存在或用 `changes()`/`excluded` 逻辑替代（保证跨库）
- 对 UUID/Decimal/DateTime 等：DB 层以 `TEXT` 读取/写入（必要时 `CAST($n AS uuid/numeric/date/timestamptz)` 仅为让 Postgres 接受；SQLite 会忽略类型名）。
- 保持 API 输出中 `created_at` 等字段可被 JS `new Date()` 解析（优先 RFC3339；必要时 Rust 端统一格式化）。

**Verification:**
- 新增 SQLite 集成测试：`sqlite::memory:` 跑 migrations 后调用少量核心查询（health + initialize 相关的最小链路）。
- 现有 `cargo test -p api` 全绿。

---

### Task 5: Docker 同时支持 SQLite/Postgres（默认 Postgres）

**Files:**
- Add: `docker-compose.sqlite.yml`
- Modify: `README.md`

**Steps:**
1. 保持 `docker-compose.yml` 默认 Postgres。
2. 新增 `docker-compose.sqlite.yml`：不启 Postgres；backend 使用 SQLite（挂载 data/config 卷）。
3. README 增加两种 Docker 启动方式说明。

---

### Task 6: CI：推送镜像到 GHCR + Docker Hub；发布三平台发行包（含 Windows MSI/EXE/Zip）

**Files:**
- Modify: `.github/workflows/release-tag.yml`
- Add: `packaging/windows/wix/*.wxs`
- Add: `packaging/windows/inno/setup.iss`
- Add: `packaging/dist/*`（启动脚本/README 模板）

**Steps:**
1. Docker job：
   - 增加 Docker Hub 登录（secrets：`DOCKERHUB_USERNAME`、`DOCKERHUB_TOKEN`）。
   - metadata-action 同时生成 GHCR 与 Docker Hub tags。
2. Release artifacts job（matrix: `ubuntu-latest`, `macos-latest`, `windows-latest`）：
   - backend: `cargo build --release -p api`
   - frontend: `npm ci && npm run build`，拷贝 `.next/standalone` + `.next/static` + `public`
   - 打包：Linux/macOS -> `.tar.gz`；Windows -> `.zip`（绿色包）
   - Windows 额外：用 WiX 产出 `.msi`，用 Inno Setup 产出 `setup.exe`
3. 用 `softprops/action-gh-release` 将上述产物上传到对应 tag release。

**Verification:**
- workflow 可在 fork/tag 上跑通（至少构建与上传阶段无脚本错误）。
- Docker 镜像在 GHCR 与 Docker Hub 都能看到同 tag 与 latest。

