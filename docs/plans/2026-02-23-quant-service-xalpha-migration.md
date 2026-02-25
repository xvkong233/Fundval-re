# Quant Service（A）+ xalpha 风格策略（2）实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在主仓库引入独立 `quant-service`（Python/FastAPI），并提供 xalpha 风格的“定投/网格/风控指标/（简化）QDII 估值”API，Rust 后端通过 HTTP 代理调用。

**Architecture:** `frontend → backend(api) → quant-service`。量化计算在 `quant-service` 里进行；后端只做鉴权、参数校验、错误归一化与任务队列串联（后续）。

**Tech Stack:** Python 3.11 + FastAPI + Pydantic v2；Rust 后端用 reqwest 代理；Docker Compose 接线。

---

## Task 1：把 quant-service 引入主仓库

**Files:**
- Create: `quant-service/pyproject.toml`
- Create: `quant-service/Dockerfile`
- Create: `quant-service/.dockerignore`
- Create: `quant-service/README.md`
- Create: `quant-service/THIRD_PARTY_NOTICES.md`
- Create: `quant-service/app/main.py`
- Create: `quant-service/app/settings.py`
- Create: `quant-service/app/routes/macd.py`
- Create: `quant-service/app/indicators/macd.py`
- Test: `quant-service/tests/test_health.py`
- Test: `quant-service/tests/test_macd*.py`

**Step 1: 写一个健康检查测试（RED）**
- Run: `python -m pytest -q quant-service/tests/test_health.py`
- Expected: FAIL（尚无服务代码）

**Step 2: 最小实现 `/health`（GREEN）**
- 实现 `FastAPI` app 与 `/health` 路由

**Step 3: 补齐 MACD API 与测试（RED→GREEN）**
- 端点：`POST /api/quant/macd`
- 行为：支持 `series[{date,val}] → calc_macd → txn_by_macd` 以及直接 `points[{index,macd}] → txn_by_macd`

---

## Task 2：实现 xalpha 风格“策略/风控指标”API（不依赖外网）

**Files:**
- Create: `quant-service/app/routes/xalpha_like.py`
- Create: `quant-service/app/xalpha_like/metrics.py`
- Create: `quant-service/app/xalpha_like/grid.py`
- Create: `quant-service/app/xalpha_like/scheduled.py`
- Create: `quant-service/tests/test_xalpha_like_metrics.py`
- Create: `quant-service/tests/test_xalpha_like_grid.py`
- Create: `quant-service/tests/test_xalpha_like_scheduled.py`

**Step 1: 指标 API（RED）**
- 端点：`POST /api/quant/xalpha/metrics`
- 入参：`series[{date,val}]`（按净值或收盘价）
- 出参：`total_return, cagr, vol_annual, sharpe, max_drawdown, drawdown_series`

**Step 2: 最小实现（GREEN）**
- 先实现 max drawdown + total return，测试通过后再加 CAGR/波动率/Sharpe

**Step 3: 网格策略（RED→GREEN）**
- 端点：`POST /api/quant/xalpha/grid`
- 入参：`series` + `grid_step_pct` + `max_position` 等
- 出参：交易动作表 + 汇总（不追求 UI 输出，只输出可审计数据）

**Step 4: 定投策略（RED→GREEN）**
- 端点：`POST /api/quant/xalpha/scheduled`
- 入参：`series` + `every_n_days`/`dates` + `amount`
- 出参：交易动作表 + 汇总

---

## Task 3：后端代理 & Docker Compose 接线

**Files:**
- Modify: `docker-compose.yml`
- Modify: `docker-compose.sqlite.yml`
- Modify: `backend/crates/api/src/config.rs`
- Create: `backend/crates/api/src/routes/quant.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`
- Test: `backend/crates/api/tests/quant_routes_test.rs`

**Step 1: compose 增加 quant-service（RED）**
- `backend` 通过 `QUANT_SERVICE_URL=http://quant-service:8002` 访问

**Step 2: 后端新增代理路由（GREEN）**
- `GET /api/quant/health` → `quant-service /health`
- `POST /api/quant/macd` → `quant-service /api/quant/macd`
- `POST /api/quant/xalpha/*` → `quant-service /api/quant/xalpha/*`
- 统一错误：无法连接时返回 502

**Step 3: 后端测试（RED→GREEN）**
- 未登录：401
- quant-service 不可达：502

---

## Task 4：验证

**Step 1: quant-service**
- Run: `python -m pytest -q`

**Step 2: backend**
- Run: `cargo test -p api --tests`

**Step 3: docker（可选）**
- Run: `docker compose up -d --build`

