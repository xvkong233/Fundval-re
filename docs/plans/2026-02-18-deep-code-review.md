# FundVal-re 深度审查报告（2026-02-18）

## 范围

- Rust 候选后端：`backend-rs/crates/api`
- Python golden 后端：`backend/api`
- Next.js 前端：`frontend-next`
- 合同测试与 Docker 编排：`tools/contract-tests`、`docker-compose*.yml`、`scripts/compose-contract.ps1`

本报告目标：在不引入“UUID 与基金代码混用”的前提下，评估**移植完整性**、**行为一致性**、**安全/稳定性风险**与**可维护性**，并给出下一步可执行整改项。

---

## 已执行的验证（证据型）

- Rust 单测：`cd backend-rs; cargo test -p api` ✅
- Rust 静态检查：`cd backend-rs; cargo clippy -p api -- -D warnings` ✅
- Next 单测：`cd frontend-next; npm test` ✅
- Next lint：`cd frontend-next; npm run lint` ✅
- 合同测试（含容器清理）：`.\scripts\compose-contract.ps1 run -Build` ✅  
  - 该脚本在 `up` 前会 `down --remove-orphans`，结束后也会 `down`，避免堆积容器/网络。

---

## API 端点对照（Python → Rust）

### 1) 明确对齐且已被合同测试覆盖（高置信）

- Health / Bootstrap / Auth
  - `GET /api/health/`
  - `POST /api/admin/bootstrap/verify`
  - `POST /api/admin/bootstrap/initialize`
  - `POST /api/auth/login`
  - `POST /api/auth/refresh`
  - `GET /api/auth/me`
  - `PUT /api/auth/password`
- Users
  - `POST /api/users/register/`
  - `GET /api/users/me/summary/`
- Sources
  - `GET /api/sources/`
  - `GET /api/sources/{source}/accuracy/`
- Funds（公共端点）
  - `GET /api/funds/`（分页结构）
  - `GET /api/funds/{fund_code}/`
  - `GET /api/funds/{fund_code}/estimate/`（404 分支已对齐）
  - `GET /api/funds/{fund_code}/accuracy/`（404 分支已对齐）
  - `POST /api/funds/batch_estimate/`
  - `POST /api/funds/batch_update_nav/`
  - `POST /api/funds/query_nav/`（404 分支已对齐）
- Accounts / Positions / Operations / Watchlists
  - `GET|POST /api/accounts/`
  - `GET|PUT|PATCH|DELETE /api/accounts/{id}/`
  - `GET /api/positions/`
  - `GET /api/positions/{id}/`
  - `POST /api/positions/recalculate/`
  - `GET /api/positions/history/`
  - `GET|POST /api/positions/operations/`
  - `GET|DELETE /api/positions/operations/{id}/`
  - `GET|POST /api/watchlists/`
  - `GET|PUT|PATCH|DELETE /api/watchlists/{id}/`
  - `POST /api/watchlists/{id}/items/`
  - `DELETE /api/watchlists/{id}/items/{fund_code}/`
  - `PUT /api/watchlists/{id}/reorder/`
- Nav history
  - `GET /api/nav-history/`
  - `GET /api/nav-history/{id}/`
  - `POST /api/nav-history/batch_query/`
  - `POST /api/nav-history/sync/`（分级权限逻辑存在；成功分支未合同测试覆盖）

### 2) 已实现但“成功分支”缺乏合同测试覆盖（中置信）

这些端点在合同测试里多为“缺失/404/空库”分支对照，真实数据场景仍存在潜在偏差：

- `GET /api/funds/{fund_code}/estimate/`（成功结构与字段精度）
- `GET /api/funds/{fund_code}/accuracy/`（成功结构：Python 是按 source 分组，带 records）
- `POST /api/funds/query_nav/`（成功结构：history/synced/latest 分支）
- `POST /api/nav-history/batch_query/`（成功结构与返回记录排序/字段）
- `POST /api/nav-history/sync/`（成功结构 + >15 的鉴权边界）
- `POST /api/funds/sync/`（管理员同步基金列表；通常依赖外网数据源）

建议在后续合同测试里通过“可控 seed 数据”补齐成功分支（见“整改建议”）。

---

## 前端页面能力对照（旧 React → Next.js）

旧版 `frontend/src/pages/*.jsx` 对应 Next 路由：

- Initialize / Login / Register ✅
- Dashboard ✅
- Accounts ✅
- Funds + Fund detail（含历史净值与同步入口）✅
- Positions（含操作流水 + 账户历史曲线 + 可视化）✅
- Watchlists ✅
- Settings / Server Config ✅
- Sources（Next 新增）✅

“历史净值属于基金信息”检查结论：
- Next 的“历史净值表格/曲线”只在 `funds/[fundCode]` 详情页出现；在 `sources/server-config/settings` 页面未发现相关渲染。

---

## 关键一致性审查：UUID vs 基金代码

已明确的契约约束：
- **请求创建操作**使用 `fund_code`（基金代码）定位基金；
- **操作流水响应**返回 `fund`（基金 UUID）与 `fund_name`；
- `fund_code` 在响应中是 write-only（不应返回），否则会破坏合同测试。

风险点与现状：
- 前端展示层已避免把 `fund(UUID)` 当作基金代码展示兜底；避免用户误读 UUID 为代码。
- 文档已明确“请求用 fund_code、响应返回 fund(UUID)”，减少混用风险。

---

## 代码质量 / 风险清单（按优先级）

### P0（阻断/会导致错误）— 已无

合同测试、Rust/Next 单测与 Rust clippy 均通过；仓库工作区干净。

### P1（高风险、建议尽快补齐）

1) **成功分支缺乏合同测试覆盖**
   - 影响：外网/真实数据/非空库时，Rust 端字段形状、排序、权限边界可能与 Python 偏差，回归难发现。
   - 覆盖建议：见下节“整改建议 1)~3)”。

2) **错误响应可能泄露内部细节**
   - Rust 多处直接返回 `{"error": e.to_string()}`（SQL/驱动错误可能包含内部信息）。
   - Python 端部分地方也会 `str(e)`；但生产环境建议统一“对外泛化 + 对内日志”。
   - 若要改动：需谨慎，避免破坏合同测试中依赖错误文本/结构的用例。

3) **部署安全默认值风险**
   - `SECRET_KEY` 默认 `"django-insecure-dev-only"`；CORS `*`。
   - 作为开发/本地可接受；生产建议强制从 env 提供或启用更严格策略。

### P2（中风险/可维护性）

1) Next 前端 `any` 较多
   - 影响：UUID/基金代码混用更难在编译期发现；字段变更更容易“静默坏”。
   - 建议：为关键 DTO（Fund/Position/Operation/NavHistory）建立统一类型与解码器。

2) positions.history 的成本小数处理差异
   - Rust 在 SELL 成本扣减时对成本做了 2 位 rescale；Python 的 Decimal 计算未显式量化但最终转 float。
   - 影响：数值极端情况下可能出现轻微差异（不影响 schema，但可能影响用户显示）。

---

## 下一步整改建议（可执行）

1) **为“成功分支”补齐合同测试（推荐优先）**
   - 在 `tools/contract-tests/src/seed.ts` 中同时向 golden/candidate DB 插入：
     - `fund`（已有）
     - `fund_nav_history`（多日）
     - `estimate_accuracy`（多源多日）
   - 新增 contract case：
     - `funds.estimate(success)`、`funds.accuracy(success)`
     - `funds.query_nav(success history/latest)`
     - `nav_history.batch_query(success)`
     - `nav_history.sync(<=15 与 >15 的权限边界)`

2) **统一错误响应策略（可选但建议）**
   - 对外：固定 `{"error":"服务器内部错误"}` / `{"detail":"Not found."}` 等稳定文案
   - 对内：`tracing::error!(...)` 打完整上下文

3) **生产部署保护（可选）**
   - 若 `DEBUG!=true` 时强制要求 `SECRET_KEY` 存在
   - CORS 白名单化（至少支持通过 env 配置）

---

## 结论

- 移植的“主路径能力”与页面功能已基本对齐，合同测试覆盖的端点一致性已建立并可重复运行。
- 深层风险主要集中在“成功分支缺少契约覆盖”（真实数据场景）与“错误响应/部署默认值”的生产化差距。

