# Qbot Quant 模块继续移植（1+2+3）Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在不依赖外部抓取的前提下，继续移植 Qbot 的量化能力：补齐 xalpha 的 trade/mul/report 体系、补齐 fund-strategies 的 compare 能力、并引入 pytrader 的可扩展策略接口与若干代表性策略。

**Architecture:** 保持当前 FundVal-re 的“显式输入序列 + 显式交易日历/自然日历”架构：backend 负责从 DB 提供净值/指数/行情序列，quant-service 负责纯计算与回测。为最大程度还原 Qbot 逻辑，quant-service 允许引入 `numpy/pandas`（必要时再引入 `scipy/bs4`），但不启用联网抓取路径。

**Tech Stack:** FastAPI + Pydantic（quant-service），Rust Axum（backend 代理与任务队列），Python `numpy/pandas`（策略计算/报表），pytest（回归测试）。

---

### Task 1: quant-service 引入 Qbot 所需 Python 依赖（numpy/pandas）

**Files:**
- Modify: `quant-service/pyproject.toml`
- Modify: `quant-service/Dockerfile`

**Step 1: 写一个“能 import pandas/numpy”的 failing test**

Test: `quant-service/tests/test_deps_import.py`

```python
def test_numpy_pandas_importable():
    import numpy  # noqa: F401
    import pandas  # noqa: F401
```

**Step 2: 运行测试验证失败**

Run: `Push-Location quant-service; python -m pytest -q tests/test_deps_import.py; Pop-Location`  
Expected: FAIL（ImportError）

**Step 3: 修改依赖并最小化 Docker 构建影响**

- `pyproject.toml` 增加：
  - `numpy`
  - `pandas`
- Dockerfile 仍 `pip install .`，确保依赖被安装进镜像。

**Step 4: 运行测试验证通过**

Run: `Push-Location quant-service; python -m pytest -q tests/test_deps_import.py; Pop-Location`  
Expected: PASS

---

### Task 2: xalpha “trade/mul/report” 兼容层（显式序列输入版）

**Files:**
- Create: `quant-service/app/qbot_xalpha_compat/trade.py`
- Create: `quant-service/app/qbot_xalpha_compat/mul.py`
- Create: `quant-service/app/qbot_xalpha_compat/report.py`
- Create: `quant-service/app/qbot_xalpha_compat/__init__.py`
- Modify: `quant-service/app/routes/xalpha_backtest.py`
- Test: `quant-service/tests/test_xalpha_trade_mul_report.py`

**Step 1: 写 failing test（单标的：scheduled → trade → summary）**

```python
def test_trade_summary_contains_total_row():
    # build status actions; run trade; summary(date) returns dataframe-like rows
    assert total_row["基金名称"] == "总计"
```

**Step 2: 运行测试验证失败**

Run: `Push-Location quant-service; python -m pytest -q tests/test_xalpha_trade_mul_report.py; Pop-Location`  
Expected: FAIL（模块不存在/字段缺失）

**Step 3: 最小实现 trade/mul 核心**

约束：
- 不做抓取：`infoobj.price` 由调用方传入 `{date, netvalue}` 序列。
- 支持 xalpha 关键语义：
  - buy：金额申购
  - sell：份额赎回；`-0.005` 全卖；`abs(x)<0.005` 按比例卖
  - 手续费：buy/sell fee rate
- 输出 `summary(date)` 需对齐 Qbot/xalpha 常用列（至少）：
  - 基金代码、基金名称、当日净值、基金份额、基金现值、基金收益、基金收益率
  - “总计” 行（组合汇总）

**Step 4: 接入 `/api/quant/xalpha/backtest` 的可选输出**

- 新增 `params.output = "actions" | "status" | "summary"`（默认 actions）
- 当 output 为 `summary` 时，返回 summary（JSON rows）以便前端/调试。

**Step 5: 运行测试验证通过**

Run: `Push-Location quant-service; python -m pytest -q tests/test_xalpha_trade_mul_report.py; Pop-Location`  
Expected: PASS

---

### Task 3: fund-strategies compare（服务端端点 + 可复用输出结构）

**Files:**
- Create: `quant-service/app/routes/fund_strategies_compare.py`
- Modify: `quant-service/app/main.py`
- Test: `quant-service/tests/test_fund_strategies_compare.py`

**Step 1: 写 failing test（两个策略同一基金序列对比）**

```python
def test_compare_returns_series_per_strategy():
    assert set(out["strategies"].keys()) == {"策略A","策略B"}
```

**Step 2: 运行测试验证失败**

Run: `Push-Location quant-service; python -m pytest -q tests/test_fund_strategies_compare.py; Pop-Location`  
Expected: FAIL（404 / router not included）

**Step 3: 实现 compare 端点（纯计算）**

输入：
- fund_series（主基金净值）
- shangzheng_series（上证）
- refer_index_points（参考指数 MACD 点）
- strategies：若干策略配置（复用 `TsStrategyConfig` 结构）

输出：
- 对每个策略返回：`actions` + `summary` + `series`（逐日快照：total_amount/left_amount/profit_rate/position/...）

**Step 4: 运行测试验证通过**

Run: `Push-Location quant-service; python -m pytest -q tests/test_fund_strategies_compare.py; Pop-Location`  
Expected: PASS

---

### Task 4: backend 代理与前端入口（最小落地）

**Files:**
- Modify: `backend/crates/api/src/routes/quant.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`
- Create: `frontend/src/app/strategies/compare/page.tsx`
- Modify: `frontend/src/components/AuthedLayout.tsx`（增加入口）

**Step 1: 写 backend 路由鉴权 failing test**

Test: `backend/crates/api/tests/quant_routes_test.rs`

**Step 2: 实现代理 `/api/quant/fund-strategies/compare`**

**Step 3: 前端新增 compare 页面（发布级 UI，滚动容器与布局）**

---

### Task 5: pytrader 策略接口（可扩展 registry + 代表性策略）

**Files:**
- Create: `quant-service/app/pytrader/registry.py`
- Create: `quant-service/app/pytrader/strategies/macd_cross.py`
- Create: `quant-service/app/pytrader/strategies/rsi_departure.py`
- Create: `quant-service/app/routes/pytrader_backtest.py`
- Modify: `quant-service/app/main.py`
- Test: `quant-service/tests/test_pytrader_backtest.py`

**Step 1: 写 failing test（策略注册 + backtest 返回 equity_curve）**

**Step 2: 最小实现（输入 OHLC/close 序列，输出 signals + 简易回测）**

**Step 3: 运行测试验证通过**

---

### Task 6: 全量回归与交付文档

**Files:**
- Modify: `docs/reviews/2026-02-24-qbot-xalpha-fund-strategies-migration-audit.md`（补齐新模块对照）

**Step 1: 跑 quant-service 全测试**

Run: `Push-Location quant-service; python -m pytest -q; Pop-Location`

**Step 2: 跑 backend 关键路由测试**

Run: `Push-Location backend/crates/api; cargo test -p api --test quant_routes_test; Pop-Location`

