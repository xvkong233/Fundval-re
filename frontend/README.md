# Frontend（Next.js）

本目录为 FundVal-re 的前端（默认端口 `3000`）。

前端通过 Next.js `rewrites()` 把 `GET/POST /api/*` 代理到后端（默认 `http://localhost:8001`）。

## 本地开发

### 1) 安装依赖

```bash
npm i
```

### 2) 启动开发服务器

```bash
# 可选：指定后端地址（默认 http://localhost:8001）
set API_PROXY_TARGET=http://localhost:8001
npm run dev
```

访问：

- `http://localhost:3000`

## 生产构建

```bash
npm run build
npm run start
```

## Docker 运行

优先使用仓库根目录的 `docker compose`（同时启动 backend / quant-service / db）：

- Postgres：`docker compose up --build`
- SQLite：`docker compose -f docker-compose.sqlite.yml up --build`

