# Sim `auto_topk_ts_timing`（全市场 Top-K + 参考指数择时）

> **目标**：在模拟盘回测中新增策略 `auto_topk_ts_timing`：复用“全市场按信号打分选 Top-K”的选基逻辑，并叠加“参考指数 MACD 择时”来控制买入/调仓时机；参考指数支持 `1.000001/1.000300/1.000905`。

## 架构概览

- **选基**：沿用 `fund_signal_snapshot` 的线性打分（pos/dip/magic 权重），按 `as_of_date` 选 Top-K。
- **择时**：对 `refer_index_code` 的指数收盘价序列调用 quant-service 的 `/api/quant/macd`，得到 `buy/sell` 事件日：
  - `buy_macd_point` 非空：仅在 `BUY` 事件日允许建仓/调仓（择时开关）。
  - `sell_macd_point` 非空：止盈 overlay 可按 `SELL` 事件日额外约束（可选）。
- **买入预算**：`buy_amount_percent` 控制每次买点日投入预算：
  - `<= 100`：按“剩余现金百分比”投入
  - `> 100`：按“固定金额（元）”投入
- **TS 日级行为（更贴近 Qbot TS）**：
  - 当开启 `buy_macd_point` 时：每个 `BUY` 事件日都会按预算“追加买入”，不要求到调仓日。
  - 调仓不再“全清仓再买入”，而是只卖出“出榜基金”，并对入选列表按预算买入/加仓。
  - 当日同时触发止盈与 BUY 信号时：**优先止盈，不买入**（避免同日卖出又加仓的反复横跳）。
- **止盈 overlay（组合级近似）**：当满足阈值组合条件（上证指数、仓位、收益率、是否权益新高、可选 SELL 事件）时按 `sell_unit/sell_num` 做减仓。

## 相关接口

- 创建运行：`POST /api/sim/runs`
  - `strategy=auto_topk_ts_timing`
  - `fund_codes` 允许空数组（表示全市场 universe）
- 运行回测：`POST /api/sim/runs/{id}/run`

## 参数（创建时 payload）

- 选基/调仓：`top_k`, `rebalance_every`, `weights?`
- 择时：`refer_index_code`, `buy_macd_point?`, `sell_macd_point?`
- 止盈/补仓：`sh_composite_index`, `fund_position`, `sell_at_top`, `sell_num`, `sell_unit`, `profit_rate`, `buy_amount_percent`

## 测试

- `backend/crates/api/tests/sim_auto_topk_ts_timing_test.rs`
  - stub quant-service `/api/quant/macd`：仅在某日返回 `txnType=buy`
  - 断言该日 `sim_daily_equity.positions_value > 0`（只在买点入场）
