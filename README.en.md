# Fundval (Extended / Second-Development Edition)

> Primary README (Chinese): `README.md`

Fundval is a fund analytics system that combines **valuation**, **risk/return metrics**, **ML signals**, **anti-ban crawling & caching**, and **operational observability**.

This repository is an **extended second-development edition** (not just a “port”): it builds on the original open-source ideas and keeps evolving with more professional analytics and a data-dense UI for decision support.

## Highlights

- **Professional metrics on fund detail**: max drawdown, annualized volatility, Sharpe ratio, etc.
- **Value & economics-based scoring**:
  - `value_score`: peer-percentile composite score (same-sector/peer aware)
  - CE (Certainty Equivalent): supports risk aversion parameter `gamma`
- **Short-term strategy (trend-first)**: provides shorter-horizon trading hints aligned with holding cycle.
- **ML signals (sector peers)**:
  - 20/60/20 position bucket: low / medium / high
  - dip-buy / magic-rebound probabilities on **5T + 20T** horizons
- **Sniffer dashboard (deep UI refactor)**:
  - data-dense layout
  - ML signals shown in table
  - “Neutral” buckets: **Buy Candidates / Watch / Avoid** with reasons
- **Crawling & caching (anti-ban)**:
  - batched crawling with throttling, jitter, daily limit
  - multi-source fallback
  - prioritizes watchlists/positions before trying to cover all funds
- **Release & deploy**:
  - one-command Docker startup (Postgres / SQLite)
  - GitHub Actions: tag-triggered CI + image publishing + release assets

## Repository structure

- `backend/`: Rust (axum/sqlx) API server
- `frontend/`: Next.js UI (Ant Design + ECharts)
- `quant-service/`: Python (FastAPI) quant/strategy service
- `packaging/`: cross-platform packaging templates
- `docs/`: plans & documentation

## Quick start (Docker recommended)

```bash
cp .env.example .env
docker compose up --build
```

SQLite-only mode (no Postgres service):

```bash
cp .env.example .env
docker compose -f docker-compose.sqlite.yml up --build
```

Endpoints:

- Frontend: `http://localhost:3000`
- Backend health: `http://localhost:8001/api/health/`
- Quant service health: `http://localhost:8002/health`

## Bootstrap (first-time initialization)

If the system is not initialized, the backend will print a **BOOTSTRAP KEY**:

```bash
docker compose logs backend | grep "BOOTSTRAP KEY"
```

Then open `http://localhost:3000/initialize`.

## Data sources

Supported sources:

- `tiantian` (alias: `eastmoney`)
- `danjuan`
- `ths` (aliases: `tonghuashun` / `10jqka`)
- `tushare` (configure token in Settings)

Sector/peer info (e.g., “Defense & Military”) is used for peer-percentile analytics and ML signal computation.

## Key pages

- Fund detail: `http://localhost:3000/funds/[fundCode]`
- Sniffer: `http://localhost:3000/sniffer`
- Settings: `http://localhost:3000/settings`
- Admin crawl config: `http://localhost:3000/server-config`

## CI / Release (tag-triggered)

When pushing a version tag like `v1.2.3`, GitHub Actions will:

- run backend tests (`cargo test -p api`)
- run frontend tests & build (`npm test`, `npm run build`)
- build & push images to **GHCR**
- optionally push to **Docker Hub** if secrets are set
- create GitHub Release notes from `CHANGELOG.md`
- upload cross-platform assets

Docker Hub secrets (multiple naming conventions supported):

- username: `DOCKERHUB_USERNAME` (or `DOCKERHUB_USER` / `DOCKER_USERNAME`)
- token: `DOCKERHUB_TOKEN` (or `DOCKERHUB_ACCESS_TOKEN` / `DOCKER_PASSWORD`)

## License

Licensed under **GNU AGPL-3.0**. See `LICENSE`.

## Disclaimer

For research and educational purposes only. Not investment advice.

## Acknowledgements

Inspired by and evolved from the open-source project **FundVal-Live**. Thanks to the original author **Ye-Yu-Mo** and all contributors.

