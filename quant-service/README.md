# quant-service

独立的基金量化计算服务（Python/FastAPI），为 FundVal 后端提供可控、可审计的策略与指标计算能力。

设计原则：
- **输入显式**：不在服务端隐式抓取净值/指数数据（由 `backend` 统一供数、缓存、限流）。
- **可审计**：所有策略/指标计算可复现、可测试。
- **按需兼容**：逐步对齐/移植 Qbot 与 xalpha 的策略口径，但不强绑它们的抓取与可视化依赖。

## 开发

```bash
# Linux/macOS
python3 -m venv .venv
source .venv/bin/activate
python -m pip install -U pip
python -m pip install -e ".[dev]"
pytest -q
uvicorn app.main:app --reload --port 8002

# Windows PowerShell
python -m venv .venv
.\.venv\Scripts\activate
python -m pip install -U pip
python -m pip install -e ".[dev]"
pytest -q
uvicorn app.main:app --reload --port 8002
```

## Docker

本仓库 `docker-compose.yml` / `docker-compose.sqlite.yml` 已包含 `quant-service`，一般不需要单独启动。

如需单独构建镜像：

```bash
docker build -t fundval-quant-service ./quant-service
docker run --rm -p 8002:8002 fundval-quant-service
```

## API

- `GET /health`
- `POST /api/quant/macd`
- `POST /api/quant/xalpha/metrics`
- `POST /api/quant/xalpha/grid`
- `POST /api/quant/xalpha/scheduled`
- `POST /api/quant/xalpha/qdiipredict`
- `POST /api/quant/xalpha/backtest`
- `POST /api/quant/fund-strategies/ts`
- `POST /api/quant/fund-strategies/compare`
- `GET /api/quant/pytrader/strategies`
- `POST /api/quant/pytrader/backtest`

## 与 backend 集成

`backend` 通过环境变量 `QUANT_SERVICE_URL` 访问本服务。

在 docker-compose 中已默认设置：
- `QUANT_SERVICE_URL=http://quant-service:8002`
