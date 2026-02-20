# 多平台发行版 + SQLite/Postgres 双栈 + CI 发布 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 发布 `v*` tag 时同时产出 Windows / Linux / macOS 的“一体发行版”，并保留 Docker 方案；后端同时支持 SQLite 与 Postgres，发行版默认 SQLite，Docker 默认 Postgres；Docker 镜像同步推送 GHCR + Docker Hub。

**Architecture:** 后端统一使用 `sqlx::AnyPool` 以支持多数据库；迁移脚本分为 `migrations/postgres` 与 `migrations/sqlite` 并以 `sqlx::migrate!` 嵌入二进制；发行版将 `backend`（Rust release binary）+ `frontend`（Next.js standalone）+ `node` runtime + 启动脚本打包为单目录安装/压缩包。

**Tech Stack:** Rust 2024 + axum + sqlx(Any + postgres + sqlite)；Next.js 16 standalone；GitHub Actions matrix；Docker Buildx；WiX Toolset（MSI）+ NSIS（EXE）用于 Windows 安装包。

---

## Task 1：建立可写数据目录与默认 DB 策略

**Files:**
- Modify: `backend/crates/api/src/config.rs`
- Modify: `backend/crates/api/src/main.rs`
- Create: `backend/crates/api/src/db.rs`
- Test: `backend/crates/api/tests/sqlite_smoke_test.rs`

**Step 1: 写失败测试（SQLite in-memory 可连接并执行 query）**

```rust
#[tokio::test]
async fn sqlite_anypool_can_connect_and_query() {
  let pool = AnyPoolOptions::new().connect("sqlite::memory:").await.unwrap();
  sqlx::query("SELECT 1").execute(&pool).await.unwrap();
}
```

**Step 2: 运行测试确认失败**

Run: `cd backend; cargo test -p api sqlite_anypool_can_connect_and_query -v`
Expected: FAIL（缺少 any/sqlite features）

**Step 3: 写最小实现**

- `sqlx` 增加 `any` + `sqlite` features
- 新增 `db.rs`：解析 URL、默认 sqlite url、`FUNDVAL_DATA_DIR` 数据目录
- `config.rs`：`FUNDVAL_DATA_DIR/config.json` 优先

**Step 4: 运行测试确认通过**

Run: `cd backend; cargo test -p api sqlite_anypool_can_connect_and_query -v`
Expected: PASS

---

## Task 2：拆分并补齐 SQLite migrations（与 Postgres 并行）

**Files:**
- Move: `backend/migrations/*.sql` → `backend/migrations/postgres/*.sql`
- Create: `backend/migrations/sqlite/*.sql`
- Modify: `backend/crates/api/src/main.rs`
- Modify: `backend/Dockerfile`

**Steps:** 建立 `MIGRATOR_PG` / `MIGRATOR_SQLITE`，sqlite schema 与现有 API 兼容，docker build 仍可编译内嵌 migrations。

---

## Task 3：将后端 SQL 改为跨 SQLite/Postgres 可用

**Files:** `backend/crates/api/src/state.rs` + `backend/crates/api/src/routes/*.rs` + 新增 sqlite 路由 smoke tests

**Steps:** Pool 统一 `AnyPool`；占位符统一 `?`；`NOW()`→`CURRENT_TIMESTAMP`；`ILIKE`→`LOWER(..) LIKE LOWER(?)`；`ANY(...)`→动态 `IN (...)`。

---

## Task 4：Docker 同时支持 Postgres 与 SQLite（默认仍为 Postgres）

**Files:** `docker-compose.yml` + `docker-compose.sqlite.yml` + `README.md`

---

## Task 5：发行版打包脚本 + Windows MSI/EXE

**Files:** `scripts/release/*`、`scripts/installer/windows/*`

---

## Task 6：GitHub Actions（tag 发布：三平台资产 + 双 registry 镜像）

**Files:** `.github/workflows/release-tag.yml`

