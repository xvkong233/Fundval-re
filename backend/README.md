# Backend（Rust / axum / sqlx）

本目录为 FundVal-re 的后端 API 服务（默认端口 `8001`）。

## 本地开发（推荐）

### 1) 选择数据库

后端支持 SQLite 与 Postgres：

- **SQLite（默认）**：不设置 `DATABASE_URL` 时自动使用 `sqlite:data/fundval.sqlite`
- **Postgres**：设置 `DATABASE_URL=postgres://...`

可选：通过 `FUNDVAL_DATA_DIR` 改变本地 `data/` 目录位置。

### 2) 运行

在仓库根目录执行：

```bash
cargo run -p api
```

健康检查：

- `http://localhost:8001/api/health/`

## Docker 运行

优先使用仓库根目录的 `docker compose`：

- Postgres：`docker compose up --build`
- SQLite：`docker compose -f docker-compose.sqlite.yml up --build`

## 常见问题

### 迁移/表结构不一致

如果你切换了数据库（SQLite ↔ Postgres）或升级了迁移文件，建议清理本地数据卷/数据文件后重启，再检查 `backend` 日志中的迁移输出。

