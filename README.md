# Fundval

> 中文 `README.md`/ English version: `README.en.md`

Fundval 是一个面向基金投资者与研究者的「估值 + 指标 + 信号 + 缓存爬取 + 运维可观测」系统。

本仓库为 **二次开发版本**：在原始思路与开源基础上持续演进，重点加入了更专业的风控/收益指标、基于经济学的性价比评分、批量缓存与机器学习信号，以及更数据密集的嗅探与购买建议 UI。

## 你能做什么（Highlights）

- **基金详情页专业指标**：最大回撤、年化波动率、夏普比率等，并提供 **同类分位综合分（`value_score`）** 与 **经济学确定性等价（CE，支持 `gamma`）**。
- **短线策略（趋势优先）**：输出更短期的交易策略提示（适合与持有周期配合使用）。
- **ML 预测信号（板块同类）**：基于「关联板块（同类）」做 20/60/20 位置分桶，并输出 **抄底/反转概率（5T + 20T）**。
- **嗅探页（深度重构）**：数据密集型仪表盘布局，表格同屏展示 ML 信号，并给出 **中性** 的“买入候选 / 观望 / 回避”分桶与原因。
- **缓存爬取（防封锁）**：支持分批次、节流、抖动、每日上限与多数据源 fallback，优先覆盖自选/持仓，再尽可能遍历全量基金。
- **发布与部署**：Docker 一键启动（Postgres/SQLite），GitHub Actions 打 Tag 自动发布镜像与跨平台附件。

## 目录结构

- `backend/`：Rust（axum/sqlx）后端 API
- `frontend/`：Next.js 前端（Ant Design + ECharts）
- `quant-service/`：Python（FastAPI）量化计算服务（策略/指标计算）
- `packaging/`：跨平台打包模板（portable、Windows 安装器等）
- `docs/`：计划与文档（包含接口/实现规划）

服务级开发说明：
- `backend/README.md`
- `frontend/README.md`
- `quant-service/README.md`

## 快速开始（Docker，推荐）

```bash
cp .env.example .env
docker compose up --build
```

如果你希望在 Docker 中使用 **SQLite**（不启动 Postgres），可以改用：

```bash
cp .env.example .env
docker compose -f docker-compose.sqlite.yml up --build
```

访问：

- 前端：`http://localhost:3000`
- 后端（health）：`http://localhost:8001/api/health/`
- 量化服务（health）：`http://localhost:8002/health`

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

## 数据源（估值/净值/关联板块）

内置支持以下数据源（用于基金估值、净值与历史净值同步）：

- `tiantian`：天天基金（兼容别名：`eastmoney`）
- `danjuan`：蛋卷
- `ths`：同花顺（兼容别名：`tonghuashun` / `10jqka`）
- `tushare`：Tushare（需在“设置”页面配置 Token）

前端页面支持选择数据源（基金详情 / 基金列表 / 自选），选择后会把 `source` 透传给后端 API。

关联板块（同类）信号优先基于数据源页面的“关联板块信息”（例如国防军工等），用于同类分位与 ML 信号计算。

## 运维（健康度/准确率）

访问 `http://localhost:3000/sources`：展示所有数据源的健康度（含名称）与估值准确率统计。

## 数据库（开箱即用）

默认（`docker-compose.yml`）使用 Docker Postgres。

发行包/本地直接运行（不使用 Docker）时，若未设置 `DATABASE_URL`，后端会默认使用本地 **SQLite**（`./data/fundval.sqlite`）。

若后端检测到 `DATABASE_URL` 指向的数据库不存在，会尝试自动创建并执行 migrations。

## 核心页面

- `http://localhost:3000/funds/[fundCode]`：基金详情（指标 + ML 信号 + 同类分位/性价比）
- `http://localhost:3000/sniffer`：嗅探（筛选 + 信号 + 购买建议）
- `http://localhost:3000/settings`：设置（含 Tushare Token）
- `http://localhost:3000/server-config`：管理员爬虫配置（批次/节流/抖动/每日上限等）

## 技术栈

- **Frontend**：React 19 + Next.js + Ant Design + ECharts
- **Backend**：Rust + axum + sqlx
- **Database**：PostgreSQL 16

## 架构

```
frontend (Next.js)
    ↓ /api
backend (axum)
    ↓ QUANT_SERVICE_URL
quant-service (FastAPI)
    ↓
PostgreSQL
```

## CI（打 Tag 自动发布镜像）

本仓库已配置 GitHub Actions：当你推送版本 tag（例如 `v1.2.3`）时，会自动完成：

- 后端 `cargo test -p api`
- 前端 `npm test` + `npm run build`
- 量化服务 `pytest -q`
- 构建并推送 Docker 镜像到 GHCR（`ghcr.io`）
- （可选）推送镜像到 Docker Hub（需配置 secrets）
- 从 `CHANGELOG.md` 抽取对应版本段落生成 Release Notes，并上传跨平台附件

镜像命名规则：

- 后端：`ghcr.io/<owner>/<repo>-backend:<tag>` 与 `:latest`
- 前端：`ghcr.io/<owner>/<repo>-frontend:<tag>` 与 `:latest`
- 量化服务：`ghcr.io/<owner>/<repo>-quant-service:<tag>` 与 `:latest`

需要你在 GitHub 仓库里确认：

- Actions 允许工作流使用 `GITHUB_TOKEN` 写入 Packages（GHCR）。
- 若你的仓库/组织策略禁止默认 token 推送镜像，可自行改用 PAT，并写入 Secrets。

如需推送到 Docker Hub，请在 GitHub 仓库 Secrets 中配置（已支持多种常见命名）：

- 用户名：`DOCKERHUB_USERNAME`（或 `DOCKERHUB_USER` / `DOCKER_USERNAME`）
- Token：`DOCKERHUB_TOKEN`（或 `DOCKERHUB_ACCESS_TOKEN` / `DOCKER_PASSWORD`）

## 开源协议

本项目采用 **GNU Affero General Public License v3.0 (AGPL-3.0)** 开源协议。详见 `LICENSE`。

## 免责声明

本项目提供的数据与分析仅供技术研究使用，不构成任何投资建议。市场有风险，交易需谨慎。

## 致谢

本项目基于开源项目 **FundVal-Live** 的思路与早期实现持续二次开发而来。感谢原作者 **Ye-Yu-Mo** 与所有贡献者的工作与分享。
