# FundVal Rust + Next.js 移植设计（对照迁移）

**目标**：用 **Rust 后端 + Next.js 前端** 100% 移植 FundVal-Live 的 **Web 端功能**，并保持 `docs/API文档/*` 中定义的 API **路径 / 请求 / 响应字段 / 状态码** 完全一致；同时提供完善的 **Docker** 部署与本地开发体验。

## 范围与非目标

### 范围（必须）
- Web 端所有页面与交互：对齐现有 `frontend/src/pages/*` 的功能集合（初始化、登录/注册、仪表盘、基金列表/详情、账户、持仓、设置、自选等）。
- 后端 REST API：对齐 `docs/API文档/*`（以 `/api` 为 Base URL）。
- Docker：一键启动数据库/缓存/后端/前端；支持在“旧后端（Django）”与“新后端（Rust）”之间切换。

### 非目标（可后续再做）
- Tauri 桌面、Capacitor 安卓的打包与发布流程（允许未来扩展）。
- 追求代码层面一字不差复刻（以行为与契约一致为准）。

## 总体策略：对照迁移（Golden Backend）

短期内保留现有 Django/DRF 后端作为 **真值基准（golden）**，并同时运行 Rust 后端：

1. 为每个 API 模块建立“对照用例”（同样的请求流、同样的测试数据）。
2. 对比 **状态码 + 响应 JSON 结构 + 关键字段值**（允许极少数动态字段白名单，如时间戳/随机 token）。
3. 当某个模块对照全部通过后，将前端指向 Rust 后端进行人工验收。
4. 全量通过后，默认切换到 Rust 后端；Django 后端保留为回归基准与紧急回退。

## 技术架构

### 后端（Rust）
- Web 框架：`axum`（tokio 异步）
- 数据库：PostgreSQL（`sqlx` + migrations）
- 缓存：Redis（可选；优先用于估值缓存 TTL 与热点数据）
- 认证：JWT（`jsonwebtoken`）+ 密码哈希（`argon2`）
- 配置：优先 `.env`，同时兼容读取 `config.json`（与现有文档一致的配置项）
- 日志：结构化日志（`tracing`）

### 前端（Next.js）
- Next.js App Router（React 19）
- UI：Ant Design（与现有前端保持一致）
- 图表：ECharts（与现有前端保持一致）
- API 调用：统一封装，Base URL 默认为 `/api`，通过 Next.js rewrite/proxy 走到后端服务

### Docker 拓扑（开发/生产一致）
- `db`：Postgres
- `redis`：Redis（可选但默认启用）
- `backend_py`：现有 Django（golden，主要用于对照测试与回退）
- `backend_rs`：新 Rust
- `frontend_next`：新 Next.js
- `nginx`（可选）：统一入口，将 `/api/` 反代到选择的后端（通过环境变量/配置切换）

## 数据与一致性

### 数据模型
Rust 后端将按现有 Django 模型与 API 文档对齐核心实体：
- User / Role
- Fund / FundHolding（穿透持仓）
- Account / Position / OperationLog（操作记录）
- Watchlist / WatchlistItem
- DataSource（数据源与准确率）
- NavHistory（历史净值）

### “100% 一致”的判定标准
- 每个接口：路径、方法、鉴权要求、请求字段、状态码、响应字段（包含字段名与类型）一致。
- 错误响应：对齐现有后端行为（包含错误字段名/格式）。
- 前端：主要交互路径可在无 Django 依赖下工作，且关键页面与数据展示一致。

## 测试与验收

### API 对照测试（核心）
在 Docker 环境中同时启动 `backend_py` 与 `backend_rs`，通过脚本执行一组“端到端请求流”：
- 注册/登录 -> 账户 CRUD -> 持仓增删改/重算 -> 自选列表 -> 基金估值/详情 -> 历史净值等

测试输出：
- 每个接口的对照差异（JSON diff）
- 通过/失败统计
- 可选快照（snapshot）用于回归

### 前端验收
- 模块迁移完成后，前端切换到 Rust 后端进行人工验收
- 关键路径录屏/截图作为交付物（可选）

