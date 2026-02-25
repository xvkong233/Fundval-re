# Qbot（xalpha + fund-strategies）移植审计（FundVal-re）

日期：2026-02-24  
基准：`8775fd16bf36eb1b1b1a0b2b9a1dd6bf5c6fe8ff`（当前工作区 HEAD）

## 0. 结论摘要（先说清楚）

本仓库当前的“xalpha 移植”分三层：

1) **Qbot-xalpha（回测/策略层）已补齐到可运行**：在 `quant-service` 新增统一端点 `POST /api/quant/xalpha/backtest`，以“显式输入序列数据 + 显式交易日历（open_dates）”的方式实现 Qbot upstream 中 `policy.py` 与 `backtest.py` 的主要策略类（含网格/定投变体/指标触发/轮动/动态平衡/XIRR 退出）。  
2) **xalpha-like（指标/动作生成层）保留**：继续提供 `metrics/grid/scheduled/qdiipredict` 等纯函数端点，并被后端异步任务批量调用（基金分析 v2、批量指标计算等）。
3) **xalpha（上游全库源码 vendor）已落地**：将 Qbot 上游携带的 `xalpha/*` 完整拷贝到 `quant-service/xalpha/*`，并做了少量“可选依赖降级”与“导入副作用最小化”改造（例如不强制依赖 `pyecharts/scipy`，`import xalpha` 不再自动导入所有子模块）。

注意：本项目在“业务链路”里仍然**不依赖 xalpha 的抓取层**（上游 `xalpha.info/provider/universal/toolbox` 大量联网抓取/可视化能力）。所有净值/指数序列由本项目后端/DB 提供给 `quant-service`；vendor 的 xalpha 主要用于“策略/报表逻辑对齐与后续拓展”，而不是作为运行时的隐式数据源。

本仓库当前的 “fund-strategies 移植”聚焦在：

1) **MACD 指标与买卖点（calcMACD / txnByMacd）** 的核心逻辑对齐（Python 实现 + TS 兼容字段）；  
2) **TS 页面 index.tsx 的 onEachDay（止盈 + MACD 买点补仓）** 的“可复现”后端版本（`/api/quant/fund-strategies/ts`）；  
3) 在模拟盘新增 **`auto_topk_ts_timing`**（全市场 Top-K + 参考指数 MACD 择时 + 组合级止盈 overlay），用于系统级自动交易回测/模拟。

下面逐策略逐类对齐说明“移植程度/差异/可用性”。

---

## 1. 上游范围与术语

### 1.1 上游（Qbot）被审计的策略来源

**xalpha：**
- `.codex/_upstream/Qbot/pyfunds/backtest/xalpha/policy.py`
- `.codex/_upstream/Qbot/pyfunds/backtest/xalpha/backtest.py`
- `.codex/_upstream/Qbot/pyfunds/backtest/xalpha/toolbox.py`

**fund-strategies：**
- `.codex/_upstream/Qbot/pyfunds/fund-strategies/src/pages/index.tsx`（TS 策略入口：`onEachDay`）
- `.codex/_upstream/Qbot/pyfunds/fund-strategies/src/utils/fund-stragegy/fetch-fund-data.ts`（MACD 计算 + txnByMacd）
- `.codex/_upstream/Qbot/pyfunds/fund-strategies/src/utils/fund-stragegy/index.ts`（交易/费用/仓位快照模型）

### 1.2 状态标注

- **已移植（高对齐）**：关键规则/边界条件基本对齐上游，可在系统里直接用。
- **部分移植**：保留“核心思想”，但输入/输出/资金与仓位模型/交易日处理/回测引擎等与上游差异显著。
- **未移植**：当前仓库无对应实现或仅有同名但不等价的功能。

---

## 2. xalpha 移植对照（逐策略/逐类）

> 说明：本仓库 xalpha 相关实现主要在两处：
> - `quant-service/app/qbot_xalpha/*`：Qbot upstream 策略/回测（统一端点 `/api/quant/xalpha/backtest`）
> - `quant-service/app/xalpha_like/*`：指标/动作生成的最小子集（metrics/grid/scheduled/qdiipredict）
> - `quant-service/xalpha/*`：Qbot 上游携带的 xalpha 全库源码（已做“可选依赖降级 + 最小导入副作用”处理）

### 2.1 `policy.py`（策略生成：status table）

上游文件：`.codex/_upstream/Qbot/pyfunds/backtest/xalpha/policy.py`

| 上游类/策略 | 上游用途（摘要） | 本仓库对应 | 移植程度 | 关键差异 | 可用性（端到端） |
|---|---|---|---|---|---|
| `policy` | 基类：从 `infoobj.price` 迭代日期，生成 `status`（买入金额/卖出份额的序列） | **不按类体系复刻**；改为统一端点直接运行策略 | 部分移植 | 不产出上游矩阵 `status` 表；改为输出 `actions/summary`，并要求调用方显式提供 `open_dates + series` | 可用（`/api/quant/xalpha/backtest`） |
| `buyandhold` | 起始日一次性买入，特殊日处理分红再投（返回 0.05 等） | `quant-service/app/qbot_xalpha/policy_buyandhold.py`（`buyandhold`） | 部分移植 | 未实现 `specialdate/comment` 分红语义；仅实现“首日买入并持有到期末”的核心行为 | 可用 |
| `scheduled` | 给定日期列表固定金额定投（生成 status） | `quant-service/app/qbot_xalpha/policy_scheduled.py`（`scheduled`）+ `xalpha_like/scheduled.py`（简化） | 已移植（高对齐，双实现） | `qbot_xalpha` 版本按 `times + open_dates` 执行；`xalpha_like` 版本是“每 N 点”简化动作生成 | 可用 |
| `scheduled_tune` | 定期不定额：基于净值分段倍数买入 | `quant-service/app/qbot_xalpha/policy_scheduled_tune.py`（`scheduled_tune`） | 已移植（高对齐） | piece 匹配按输入顺序取首个 `nav<=threshold` | 可用 |
| `scheduled_window` | 定期不定额：基于窗口涨跌幅分段倍数买入（MAX/MIN/AVG） | `quant-service/app/qbot_xalpha/policy_scheduled_window.py`（`scheduled_window`） | 已移植（高对齐） | 交易日由 `open_dates` 控制；窗口与分段规则由调用方显式提供 | 可用 |
| `grid` | 多档网格：按 buypercent/sellpercent 分档，买入按等分资金，卖出按等分份额（需交易日过滤） | `quant-service/app/qbot_xalpha/strategy_grid.py`（`grid`）+ `xalpha_like/grid.py`（简化） | 已移植（高对齐，双实现） | `qbot_xalpha` 实现多档列表 + pos 状态机 + “卖出 1/pos”语义；`xalpha_like` 仍保留简化锚点网格 | 可用 |
| `indicator_cross` | 两指标交叉：上穿买入/下穿卖出（全仓进出） | `quant-service/app/qbot_xalpha/strategy_indicator_cross.py`（`indicator_cross`） | 已移植（高对齐） | 不生成指标列；要求输入序列包含对比字段 | 可用 |
| `indicator_points` | 指标阈值分档：买入/卖出点位列表 + 分档权重（支持只买不卖） | `quant-service/app/qbot_xalpha/strategy_indicator_points.py`（`indicator_points`） | 已移植（高对齐） | 不生成指标列；实现 buy/sell 权重归一化、selllevel 与分档卖出比例语义 | 可用 |

### 2.2 `backtest.py`（回测引擎示例类）

上游文件：`.codex/_upstream/Qbot/pyfunds/backtest/xalpha/backtest.py`

| 上游类/策略 | 上游用途（摘要） | 本仓库对应 | 移植程度 | 关键差异 | 可用性（端到端） |
|---|---|---|---|---|---|
| `BTE`/`trade`/`mul` 等 | 提供 backtest engine、买卖执行、组合汇总、现金流计算等 | `quant-service/app/qbot_xalpha/bte_engine.py`（`BteEngine`） | 部分移植 | 仅实现本项目所需的最小回测内核（现金/持仓/手续费/现金流）；未复刻上游 `trade/mul` 的完整报表/可视化/场内账单 | 可用 |
| `Scheduled(BTE)` | 无脑定投：date_range 中每次买入固定 value | 策略：`bte_scheduled`（`/api/quant/xalpha/backtest`） | 已移植（高对齐） | 上游依赖 `opendate_set`；本仓库用调用方传入 `open_dates` | 可用 |
| `AverageScheduled` | 价值平均定投：aim 累加，aim>current 买入差额，否则按净值卖出 | `quant-service/app/qbot_xalpha/strategy_bte_average_scheduled.py`（`bte_average_scheduled`） | 已移植（高对齐） | 不依赖上游 `mul.summary`，直接用引擎持仓市值 | 可用 |
| `ScheduledSellonXIRR` | 定投 + 年化收益率（XIRR）达阈值全卖出（周度检查、最短持有期） | `quant-service/app/qbot_xalpha/strategy_bte_scheduled_sell_on_xirr.py`（`bte_scheduled_sell_on_xirr`）+ `qbot_xalpha/xirr.py` | 已移植（高对齐） | xirr 为轻量实现（Newton+兜底扫描），保留周度检查与 holding_time | 可用 |
| `Tendency28` | 二八轮动：在 HS300/ZZ500/货基 之间按动量阈值切换 | `quant-service/app/qbot_xalpha/strategy_bte_tendency28.py`（`bte_tendency28`） | 已移植（中-高对齐） | check_dates 由调用方提供；动量按 “date 前 prev 点”计算；**切仓资金口径**：本实现按“卖出到账现金（含赎回费）”买入，上游实现更接近“按卖出前市值再投入” | 可用 |
| `Balance` | 动态平衡：按目标权重定期再平衡（含赎回费估算） | `quant-service/app/qbot_xalpha/strategy_bte_balance.py`（`bte_balance`） | 已移植（高对齐） | 建仓+再平衡逻辑保留；赎回费按 `sell_fee_rate` 估算份额 | 可用 |

### 2.3 `toolbox.py`（工具/预测类）

上游文件：`.codex/_upstream/Qbot/pyfunds/backtest/xalpha/toolbox.py`

| 上游类/函数 | 上游用途（摘要） | 本仓库对应 | 移植程度 | 关键差异 | 可用性（端到端） |
|---|---|---|---|---|---|
| `RTPredict` | 实时预测（依赖数据源/交易日/汇率/持仓） | 无 | 未移植 | 本仓库不在服务端隐式抓取外部行情/汇率 | 不可用 |
| `QDIIPredict` | QDII 预测（多腿持仓 + 汇率等，组合当日净值预测） | `quant-service/app/xalpha_like/qdiipredict.py`（`/api/quant/xalpha/qdiipredict`） | 部分移植（核心公式保留） | 纯函数组合加权：legs 的 `ratio/currency_ratio` 由调用方提供；不移植抓取/节假日/溢价等复杂处理 | 可用（纯函数） |
| `xirr/xnpv/myround` 等 | 现金流与数值工具（cons.py） | `quant-service/app/qbot_xalpha/rounding.py`、`quant-service/app/qbot_xalpha/xirr.py` | 部分移植 | 仅保留回测所需；不引入 scipy/requests/pyecharts 等重依赖 | 可用（回测内核） |
| 其它大量工具（交易日、估值、持仓、PEB/TEB 等） | 各类辅助与估值工具 | 无 | 未移植 | 本项目选择不移植抓取/可视化工具箱 | 不可用 |

### 2.4 本仓库“xalpha-like”端点清单（实际可用的能力）

实现：`quant-service/app/routes/xalpha_like.py`

| 端点 | 实现文件 | 能力 | 是否在任务队列可用 |
|---|---|---|---|
| `POST /api/quant/xalpha/metrics` | `quant-service/app/xalpha_like/metrics.py` | 指标：total_return / cagr / vol_annual / sharpe / max_drawdown + drawdown_series | 是（批量） |
| `POST /api/quant/xalpha/grid` | `quant-service/app/xalpha_like/grid.py` | 简化锚点网格 actions | 是（批量） |
| `POST /api/quant/xalpha/scheduled` | `quant-service/app/xalpha_like/scheduled.py` | 简化定投 actions（每 N 点） | 是（批量） |
| `POST /api/quant/xalpha/qdiipredict` | `quant-service/app/xalpha_like/qdiipredict.py` | QDII 纯函数预测（legs 加权） | 是（批量） |

后端接入（代理 + 批量异步）：
- `backend/crates/api/src/routes/quant.rs`
- `backend/crates/api/src/tasks.rs`（`quant_xalpha_*_batch`）

前端入口（用户可直接触发）：
- `frontend/src/app/funds/[fundCode]/page.tsx`：`基金分析 v2（Qbot/xalpha）`

### 2.5 本仓库“Qbot-xalpha”统一 backtest 端点（新增）

实现：`quant-service/app/routes/xalpha_backtest.py`（`POST /api/quant/xalpha/backtest`）

| strategy | 对齐上游 | 主要实现文件 |
|---|---|---|
| `buyandhold` | `policy.buyandhold` | `quant-service/app/qbot_xalpha/policy_buyandhold.py` |
| `scheduled` | `policy.scheduled` | `quant-service/app/qbot_xalpha/policy_scheduled.py` |
| `scheduled_tune` | `policy.scheduled_tune` | `quant-service/app/qbot_xalpha/policy_scheduled_tune.py` |
| `scheduled_window` | `policy.scheduled_window` | `quant-service/app/qbot_xalpha/policy_scheduled_window.py` |
| `grid` | `policy.grid` | `quant-service/app/qbot_xalpha/strategy_grid.py` |
| `indicator_cross` | `policy.indicator_cross` | `quant-service/app/qbot_xalpha/strategy_indicator_cross.py` |
| `indicator_points` | `policy.indicator_points` | `quant-service/app/qbot_xalpha/strategy_indicator_points.py` |
| `bte_scheduled` | `backtest.Scheduled` | `quant-service/app/routes/xalpha_backtest.py`（基于 `BteEngine`） |
| `bte_average_scheduled` | `backtest.AverageScheduled` | `quant-service/app/qbot_xalpha/strategy_bte_average_scheduled.py` |
| `bte_scheduled_sell_on_xirr` | `backtest.ScheduledSellonXIRR` | `quant-service/app/qbot_xalpha/strategy_bte_scheduled_sell_on_xirr.py` + `qbot_xalpha/xirr.py` |
| `bte_tendency28` | `backtest.Tendency28` | `quant-service/app/qbot_xalpha/strategy_bte_tendency28.py` |
| `bte_balance` | `backtest.Balance` | `quant-service/app/qbot_xalpha/strategy_bte_balance.py` |

补充：报表输出（trade/mul 子集）
- `POST /api/quant/xalpha/backtest` 支持 `params.output="summary"`：在原有 `actions/summary` 基础上额外返回 `report.summary`（JSON rows），便于前端/调试输出 xalpha 风格的组合汇总表。
- 对应实现：`quant-service/app/qbot_xalpha_compat/*`（基于 `pandas`，不引入 `pyecharts`，不包含抓取层）。

---

## 3. fund-strategies 移植对照（逐策略/逐模块）

### 3.1 MACD 指标与买卖点：`calcMACD` / `txnByMacd`

上游文件：
- `.codex/_upstream/Qbot/pyfunds/fund-strategies/src/utils/fund-stragegy/fetch-fund-data.ts`

上游要点（摘要）：
- `calcMACD`：EMA12/EMA26 → diff/dea → macd；并按“同侧波段”归一化得到 `macdPosition`（每段内 `abs(macd)/maxAbs(macd)`）。
- `txnByMacd(indexData, sellPosition, buyPosition)`：
  - 先按 `macd` 正负分段（波段）
  - 每波段找“阈值点”：在峰值后回撤到 `maxVal*sellPosition`（正段）或 `maxVal*buyPosition`（负段）时标记 sell/buy 的阈值 index
  - 支持“多峰多阈值点”的逻辑（峰值创新高且上一峰已出阈值，则开启新的阈值搜索）
  - 输出字段：给阈值日写入 `txnType = 'sell' | 'buy'`

本仓库对应：
- `quant-service/app/indicators/macd.py`
  - `calc_macd(series)`：输出 `ema12/ema26/diff/dea/macd/macd_position`，并兼容字段 `macdPosition`
  - `txn_by_macd(points, sell_position, buy_position)`：输出 `txn_type`，并兼容字段 `txnType`

移植程度：**已移植（高对齐）**  
主要差异/注意：
- 输入数据：本仓库要求 `[{date, val}]`，不负责抓取指数数据。
- 极小值处理：对 `macd≈0` 的分组边界做了数值保护（`1e-12`），避免分段异常。

可用性：
- 作为“参考指数择时”组件，已被用于基金分析 v2 与模拟盘策略 `auto_topk_ts_timing`（后端侧）。

### 3.2 交易/费用/仓位模型：`InvestmentStrategy` / `InvestDateSnapshot`

上游文件：
- `.codex/_upstream/Qbot/pyfunds/fund-strategies/src/utils/fund-stragegy/index.ts`

上游要点（摘要）：
- 买入手续费：`net_amount = amount / (1 + buyFeeRate)`，再用 `portion = net_amount / val`。
- 卖出手续费：先由金额/份额换算 `portion`，再 `amount_in = val * portion * (1 - sellFeeRate)`。
- 每日快照维护：`leftAmount/portion/cost/totalBuyAmount/totalSellAmount/maxPrincipal/maxAccumulatedProfit` 等。
- 分红日 `isBonus` 限制买卖（上游 `buy/sell` 会直接 return）。

本仓库对应：
- `quant-service/app/strategies/ts_invest.py` 的 `run_ts_strategy()`

移植程度：**部分移植（聚焦 TS 策略所需字段）**  
已对齐的关键点：
- 买/卖手续费计算公式与精度（2 位/4 位）对齐上游核心规则。
- `max_accumulated_profit`（即上游 `maxAccumulatedProfit` 的用途：配合“收益新高卖出”条件）。
- `salary`（每月 1 号入金）与 `fixedInvest`（weekly/monthly）行为与上游一致：按“自然日”逐日推进，而不是仅按交易日。
- `txnType` 事件语义：对“fallback 到前一交易日数据”的场景做了保护（不继承前一天 `txnType`，避免重复触发）。
- TS 的“0 表示关闭”语义已对齐：`sellMacdPoint/buyMacdPoint/profitRate` 为 0 时分别表示“不要求卖点 / 不触发买点 / 不设收益率阈值”。

与上游的主要差异：
- 未实现上游“分红日/bonus 日不允许交易”语义（本仓库输入不包含 bonus 信息）。
- 定投/工资的日期范围来源：上游以 `dateRange` 作为自然日范围；本仓库以输入 `fund_series` 的首尾日期作为自然日范围，并用“向前回退”规则补齐非交易日的净值与上证指数值。
- 本仓库实现是“策略执行器”返回 `actions + summary`：其中 `actions` 记录了**实际发生的买入/卖出**（含初始建仓 `initial`、定投 `fixed_invest`、止盈 `stop_profit`、补仓 `macd_buy`）。

可用性：
- 已作为后端策略端点：`POST /api/quant/fund-strategies/ts`（见下一节）。

### 3.3 TS 策略主体：`pages/index.tsx` 的 `onEachDay`

上游文件：
- `.codex/_upstream/Qbot/pyfunds/fund-strategies/src/pages/index.tsx`

上游要点（逐条对应）：
- **止盈（卖出）触发条件**：
  - 仓位 `level > fundPosition`
  - 上证指数 `curSzIndex.val > shCompositeIndex`
  - （可选）收益新高 `maxAccumulatedProfit.date === latestInvestment.date`
  - （可选）MACD 卖点 `curReferIndex.txnType === 'sell'`
  - 持有收益率 `profitRate > profitRateThreshold`
- **补仓（买入）触发条件**：
  - （可选）MACD 买点 `curReferIndex.txnType === 'buy'`
  - 买入金额：`buyAmountPercent <=100` 用 `leftAmount * pct`，否则为绝对值金额

本仓库对应：
- `quant-service/app/strategies/ts_invest.py` 的 `run_ts_strategy()`（包含同名参数：`sh_composite_index/fund_position/sell_at_top/sell_macd_point/buy_macd_point/buy_amount_percent/...`）
- 路由：`quant-service/app/routes/fund_strategies_ts.py`（端点 `POST /api/quant/fund-strategies/ts`）

移植程度：**已移植（高对齐，带 1 个重要修复）**
- 重要修复：`txnType` 不跨日继承（事件而非状态）。该修复用于解决“非交易日 fallback 数据”导致的重复触发买卖（本仓库服务端需要处理这种 fallback；上游页面以 map 取值，缺失日期则为空对象，不会继承）。
- 重要对齐：`sellMacdPoint/buyMacdPoint/profitRate = 0` 的“关闭开关”语义与上游一致（见 3.2）。

可用性：
- 后端任务会调用该端点产出 `actions` 作为“择时/止盈”的可解释信号之一（用于基金分析 v2 与模拟盘策略对齐）。

### 3.4 多策略对比（compare）与参考指数数据流

本仓库新增：
- `quant-service/app/routes/fund_strategies_compare.py`：`POST /api/quant/fund-strategies/compare`
  - 输入：`fund_series + shangzheng_series + (refer_index_points 或 refer_index_series) + strategies[]`
  - 输出：每个策略返回 `actions/summary/series`（`series` 为逐日快照，便于前端对齐展示）
  - **关键补齐**：当 `refer_index_points` 为空且提供 `refer_index_series` 时，服务端会按“每个策略自己的 `buy_macd_point/sell_macd_point`”计算 MACD `txnType`，从而允许多策略并行对比。
- `backend/crates/api/src/routes/indexes.rs`：`GET /api/indexes/daily`
  - 返回参考指数日线 close（落库表 `index_daily_price`，默认来源 `eastmoney`，缺失时可尝试抓取并 upsert）。
- `backend/crates/api/src/index_series.rs`：抽取“index_daily_price 读写 + Eastmoney 回填”的通用实现，供 sim/tasks/indexes 路由复用，避免三处重复实现与类型转换差异。
- `frontend/src/app/strategies/compare/page.tsx`：策略对比页已支持选择参考指数（上证/沪深300/中证500），并把指数序列传给 compare 端点。

移植程度：**已移植（高对齐，按系统架构做了“供数解耦”）**  
关键差异：上游 fund-strategies 前端直接拉取指数数据；本仓库把指数序列统一下沉到 backend（缓存/可审计），quant-service 只做计算。

---

## 4. 系统内的落地与可用性核对（不是“只有算法文件”）

### 4.1 quant-service（算法服务）

| 能力 | 文件/端点 |
|---|---|
| xalpha-like（metrics/grid/scheduled/qdiipredict） | `quant-service/app/routes/xalpha_like.py` |
| Qbot-xalpha（统一 backtest） | `quant-service/app/routes/xalpha_backtest.py` |
| Qbot-xalpha（trade/mul 报表子集） | `quant-service/app/qbot_xalpha_compat/*`（由 `/api/quant/xalpha/backtest` 的 `params.output="summary"` 触发输出） |
| xalpha vendor（上游全库源码） | `quant-service/xalpha/*`（`import xalpha` 不强制拉起可选依赖；需要时显式导入子模块） |
| fund-strategies TS 策略端点 | `quant-service/app/routes/fund_strategies_ts.py` |
| fund-strategies compare（多策略对比） | `quant-service/app/routes/fund_strategies_compare.py`（`POST /api/quant/fund-strategies/compare`） |
| MACD 计算与买卖点（TS 兼容字段） | `quant-service/app/indicators/macd.py` |
| pytrader（策略 registry + 简易回测） | `quant-service/app/routes/pytrader_backtest.py`（`GET /api/quant/pytrader/strategies`、`POST /api/quant/pytrader/backtest`） |

### 4.2 backend（异步任务队列批量计算 + 代理）

| 能力 | 文件 |
|---|---|
| 代理 xalpha-like 端点 | `backend/crates/api/src/routes/quant.rs` |
| 代理 Qbot-xalpha backtest 端点 | `backend/crates/api/src/routes/quant.rs`（`/api/quant/xalpha/backtest`） |
| xalpha-like 批量异步任务（`quant_xalpha_*_batch`） | `backend/crates/api/src/tasks.rs` |
| 基金分析 v2 任务中调用 metrics + macd + fund-strategies TS | `backend/crates/api/src/tasks.rs`（内部构造 `url_metrics/url_ts/...`） |
| 参考指数日线序列（供 compare / fund analysis / sim 复用） | `backend/crates/api/src/routes/indexes.rs`（`GET /api/indexes/daily`）+ 表 `index_daily_price` |

### 4.3 frontend（用户可见入口）

| 能力 | 文件 |
|---|---|
| 基金详情页的“基金分析 v2（Qbot/xalpha）”，支持参考指数切换（上证/沪深300/中证500）并可跳转任务日志 | `frontend/src/app/funds/[fundCode]/page.tsx` |
| 策略对比页（fund-strategies compare，支持选择参考指数：沪深300/中证500等） | `frontend/src/app/strategies/compare/page.tsx` |

---

## 5. 测试与证据（当前仓库已有覆盖）

> 说明：这里只列“文件位置”，便于你快速打开核对；测试结果以你本机实际执行为准。

- `quant-service`：策略/指标纯函数 pytest
  - `quant-service/tests/test_macd_calc.py`
  - `quant-service/tests/test_macd.py`
  - `quant-service/tests/test_fund_strategies_ts.py`
  - `quant-service/tests/test_vendor_xalpha_import.py`（vendor xalpha 可导入性）
  - `quant-service/tests/test_xalpha_like_metrics.py`
  - `quant-service/tests/test_xalpha_like_grid.py`
  - `quant-service/tests/test_xalpha_like_scheduled.py`
  - `quant-service/tests/test_xalpha_like_qdiipredict.py`
  - `quant-service/tests/test_xalpha_backtest_endpoint.py`
- `backend`：`auto_topk_ts_timing` 的行为测试：`backend/crates/api/tests/sim_auto_topk_ts_timing_test.rs`
- `backend`：路由鉴权/代理测试（包含 `/api/quant/xalpha/backtest`、`/api/indexes/daily`）：`backend/crates/api/tests/quant_routes_test.rs`
- `frontend`：已有单元测试（示例：`frontend/src/lib/__tests__/*`）

---

## 6. 差距清单（要做到“完整移植 xalpha”还缺什么）

“完整移植 xalpha”在本项目里拆成两个层面：

1) **源码是否完整可用**：已完成。`quant-service/xalpha/*` 已包含 Qbot 上游携带的 xalpha 全库源码，并做了少量改造以保证在缺少可选依赖时也能稳定导入（例如 `pyecharts/scipy`）。  
2) **是否把 xalpha 作为线上运行时数据源与可视化/抓取工具**：当前仍然刻意不做（业务链路继续走“后端供数 + quant-service 纯函数/回测”）。

因此，剩余差距主要是“集成策略”而不是“代码是否存在”：

- **抓取与数据源层**：xalpha 的 `info/provider/universal/toolbox` 仍以联网抓取为主；本项目已有自己的抓取/缓存/限流/任务队列体系，暂不建议在 quant-service 内引入隐式联网依赖。
- **报表体系对齐**：vendor xalpha 自带 `record/trade/mul` 等完整体系，但本项目的回测端点目前仍以 `BteEngine` + `qbot_xalpha_compat` 的轻量报表为主；若要做到“完全按 xalpha 报表输出”，需要把 `/api/quant/xalpha/backtest` 的结果映射到 xalpha 的 trade/mul 模型或直接复用其引擎。
- **交易日历口径**：vendor xalpha 仍携带 `caldate.csv`；而本项目策略回测要求调用方显式传入 `open_dates`（由后端保证正确性）。
