# Fundval（Rust + Next.js 移植版）

盘中基金实时估值与逻辑审计系统：基于持仓穿透与实时行情加权计算，提供估值、净值、历史净值与运维可观测能力。

本仓库主分支仅保留移植后的实现：

- `backend/`：Rust（axum/sqlx）
- `frontend/`：Next.js

对照参考代码保留在分支 `reference/golden`（用于对比验证/回溯）。

## 快速开始（Docker，推荐）

```bash
cp .env.example .env
docker compose up --build
```

访问：

- 前端：`http://localhost:3000`
- 后端（health）：`http://localhost:8001/api/health/`

如端口冲突，可在 `.env` 中调整 `FRONTEND_HOST_PORT / BACKEND_HOST_PORT`。

## 部署注意（安全）

- 生产环境务必设置高熵随机 `SECRET_KEY`；可将 `.env` 中 `REQUIRE_SECURE_SECRET=true` 以强制校验。
- 如需浏览器跨域直连后端（不走前端 `/api` 反代），请配置 `CORS_ALLOW_ORIGINS`（逗号分隔 origins；`*` 将放开所有来源，不建议生产）。

## 初始化（Bootstrap Key）

首次启动后，如果系统未初始化，`backend` 会在日志里输出 `BOOTSTRAP KEY`：

```bash
docker compose logs backend | grep "BOOTSTRAP KEY"
```

然后访问 `http://localhost:3000/initialize` 完成初始化。

## 数据源（估值/净值）

内置支持以下数据源（用于基金估值、净值与历史净值同步）：

- `tiantian`：天天基金（兼容别名：`eastmoney`）
- `danjuan`：蛋卷
- `ths`：同花顺（兼容别名：`tonghuashun` / `10jqka`）
- `tushare`：Tushare（需在“设置”页面配置 Token）

前端页面支持选择数据源（基金详情 / 基金列表 / 自选），选择后会把 `source` 透传给后端 API。

## 运维（健康度/准确率）

访问 `http://localhost:3000/sources`：展示所有数据源的健康度（含名称）与估值准确率统计。

## 数据库（开箱即用）

默认使用 Docker Postgres。

若后端检测到 `DATABASE_URL` 指向的数据库不存在，会尝试自动创建并执行 migrations。

## 技术栈

- **Frontend**：React 19 + Next.js + Ant Design + ECharts
- **Backend**：Rust + axum + sqlx
- **Database**：PostgreSQL 16

## 架构

```
frontend (Next.js)
    ↓ /api
backend (axum)
    ↓
PostgreSQL
```

## CI（打 Tag 自动发布镜像）

本仓库已配置 GitHub Actions：当你推送版本 tag（例如 `v1.2.3`）时，会自动完成：

- 后端 `cargo test -p api`
- 前端 `npm test` + `npm run build`
- 构建并推送 Docker 镜像到 GHCR（`ghcr.io`）

镜像命名规则：

- 后端：`ghcr.io/<owner>/<repo>-backend:<tag>` 与 `:latest`
- 前端：`ghcr.io/<owner>/<repo>-frontend:<tag>` 与 `:latest`

需要你在 GitHub 仓库里确认：

- Actions 允许工作流使用 `GITHUB_TOKEN` 写入 Packages（GHCR）。
- 若你的仓库/组织策略禁止默认 token 推送镜像，可自行改用 PAT，并写入 Secrets。

如需推送到 Docker Hub/私有 Registry，请在仓库 Secrets 中配置并相应修改工作流：

- `DOCKER_USERNAME`
- `DOCKER_PASSWORD`（建议用 Token）

## 开源协议

本项目采用 **GNU Affero General Public License v3.0 (AGPL-3.0)** 开源协议。详见 `LICENSE`。

## 免责声明

本项目提供的数据与分析仅供技术研究使用，不构成任何投资建议。市场有风险，交易需谨慎。

## 致谢

本项目移植自开源项目 **FundVal-Live**，原作者 **Ye-Yu-Mo**。感谢原作者与所有贡献者的工作与分享。
