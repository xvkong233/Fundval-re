from __future__ import annotations


def test_ts_strategy_emits_buy_and_sell_actions() -> None:
    from app.strategies.ts_invest import TsStrategyConfig, run_ts_strategy

    fund_series = [
        {"date": "2026-01-02", "val": 1.0000},
        {"date": "2026-01-03", "val": 1.0500},
        {"date": "2026-01-04", "val": 1.0800},
        {"date": "2026-01-05", "val": 1.0600},
        {"date": "2026-01-06", "val": 1.0700},
    ]

    shangzheng_series = [
        {"date": "2026-01-02", "val": 3500.0},
        {"date": "2026-01-03", "val": 3600.0},
        {"date": "2026-01-04", "val": 3650.0},
        {"date": "2026-01-05", "val": 3620.0},
        {"date": "2026-01-06", "val": 3630.0},
    ]

    # Provide refer index points directly to keep the test deterministic.
    refer_index_points = [
        {"date": "2026-01-02", "val": 100.0},
        {"date": "2026-01-04", "val": 101.0, "txnType": "sell"},
        {"date": "2026-01-05", "val": 99.0, "txnType": "buy"},
    ]

    cfg = TsStrategyConfig(
        total_amount=10000.0,
        salary=0.0,
        purchased_fund_amount=9000.0,
        fixed_amount=0.0,
        period=("monthly", 1),
        sh_composite_index=3000.0,
        fund_position=70.0,
        sell_at_top=False,
        sell_num=10.0,
        sell_unit="fundPercent",
        profit_rate=0.0,
        buy_amount_percent=20.0,
        sell_macd_point=75.0,
        buy_macd_point=50.0,
    )

    out = run_ts_strategy(
        fund_series=fund_series,
        shangzheng_series=shangzheng_series,
        refer_index_points=refer_index_points,
        cfg=cfg,
    )

    actions = out["actions"]
    assert len(actions) == 3
    assert actions[0]["type"] == "buy"
    assert actions[0]["reason"] == "initial"
    assert actions[0]["date"] == "2026-01-02"
    assert actions[1]["type"] == "sell"
    assert actions[1]["reason"] == "stop_profit"
    assert actions[1]["date"] == "2026-01-04"
    assert actions[2]["type"] == "buy"
    assert actions[2]["reason"] == "macd_buy"
    assert actions[2]["date"] == "2026-01-05"


def test_ts_strategy_salary_on_non_trading_first_of_month() -> None:
    """
    上游 fund-strategies（InvestDateSnapshot.income）是在“逐日”循环里判断每月 1 号发工资，
    即使该日不是交易日也会入金。

    本测试用仅包含交易日的 fund_series（缺失 2026-02-01）验证该语义。
    """
    from app.strategies.ts_invest import TsStrategyConfig, run_ts_strategy

    fund_series = [
        {"date": "2026-01-30", "val": 1.0},
        {"date": "2026-02-02", "val": 1.0},
    ]
    shangzheng_series = [
        {"date": "2026-01-30", "val": 3000.0},
        {"date": "2026-02-02", "val": 3000.0},
    ]

    out = run_ts_strategy(
        fund_series=fund_series,
        shangzheng_series=shangzheng_series,
        refer_index_points=[],
        cfg=TsStrategyConfig(
            total_amount=100.0,
            salary=10.0,
            purchased_fund_amount=0.0,
            fixed_amount=0.0,
            period=("monthly", 1),
            sh_composite_index=10_000.0,  # disable sell via impossible threshold
            fund_position=100.0,
            sell_at_top=False,
            profit_rate=0.0,
            buy_macd_point=None,
            sell_macd_point=None,
        ),
    )

    assert out["actions"] == []
    assert out["summary"]["left_amount"] == 110.0


def test_ts_strategy_fixed_invest_can_happen_on_non_trading_day() -> None:
    """
    上游 fixedInvest 是按自然日遍历，命中定投日即 buy(amount, date)，
    若该日为非交易日，getFundByDate 会回退到前一有效交易日净值。

    因此定投动作的 date 应保持为“定投日”（非交易日），而不是顺延到下一交易日。
    """
    from app.strategies.ts_invest import TsStrategyConfig, run_ts_strategy

    fund_series = [
        {"date": "2026-01-30", "val": 1.0},
        {"date": "2026-02-02", "val": 1.0},
    ]
    shangzheng_series = [
        {"date": "2026-01-30", "val": 3000.0},
        {"date": "2026-02-02", "val": 3000.0},
    ]

    out = run_ts_strategy(
        fund_series=fund_series,
        shangzheng_series=shangzheng_series,
        refer_index_points=[],
        cfg=TsStrategyConfig(
            total_amount=100.0,
            salary=0.0,
            purchased_fund_amount=0.0,
            fixed_amount=50.0,
            period=("monthly", 1),
            sh_composite_index=10_000.0,
            fund_position=100.0,
            sell_at_top=False,
            profit_rate=0.0,
            buy_macd_point=None,
            sell_macd_point=None,
        ),
    )

    assert len(out["actions"]) == 1
    assert out["actions"][0]["type"] == "buy"
    assert out["actions"][0]["date"] == "2026-02-01"
    assert out["actions"][0]["amount"] == 50.0


def test_ts_strategy_buy_macd_point_zero_disables_buy() -> None:
    """
    上游逻辑：if (buyMacdPoint && txnType === 'buy') 才触发买入。
    buyMacdPoint=0 时应视为“关闭”。
    """
    from app.strategies.ts_invest import TsStrategyConfig, run_ts_strategy

    fund_series = [
        {"date": "2026-01-05", "val": 1.0},
    ]
    shangzheng_series = [
        {"date": "2026-01-05", "val": 3000.0},
    ]
    refer_index_points = [{"date": "2026-01-05", "val": 100.0, "txnType": "buy"}]

    out = run_ts_strategy(
        fund_series=fund_series,
        shangzheng_series=shangzheng_series,
        refer_index_points=refer_index_points,
        cfg=TsStrategyConfig(
            total_amount=100.0,
            salary=0.0,
            purchased_fund_amount=0.0,
            fixed_amount=0.0,
            period=("monthly", 1),
            sh_composite_index=10_000.0,
            fund_position=100.0,
            sell_at_top=False,
            profit_rate=0.0,
            buy_macd_point=0.0,  # disabled
            buy_amount_percent=20.0,
            sell_macd_point=None,
        ),
    )

    assert out["actions"] == []


def test_ts_strategy_sell_macd_point_zero_disables_sell_gating_and_profit_rate_zero_disables_threshold() -> None:
    """
    上游逻辑：(!sellMacdPoint || txnType === 'sell')，sellMacdPoint=0 时不要求 txnType。
    同时 profitRate 阈值是 (profitRate/100 || -100)，profitRate=0 时应视为不设阈值（永真）。
    """
    from app.strategies.ts_invest import TsStrategyConfig, run_ts_strategy

    fund_series = [
        {"date": "2026-01-02", "val": 1.0},
        {"date": "2026-01-03", "val": 1.2},
    ]
    shangzheng_series = [
        {"date": "2026-01-02", "val": 2000.0},
        {"date": "2026-01-03", "val": 4000.0},
    ]
    # 注意：没有 txnType='sell' 也应允许卖出
    refer_index_points = [{"date": "2026-01-03", "val": 100.0, "txnType": "buy"}]

    out = run_ts_strategy(
        fund_series=fund_series,
        shangzheng_series=shangzheng_series,
        refer_index_points=refer_index_points,
        cfg=TsStrategyConfig(
            total_amount=100.0,
            salary=0.0,
            purchased_fund_amount=100.0,  # first day buy => high position
            fixed_amount=0.0,
            period=("monthly", 1),
            sh_composite_index=3000.0,
            fund_position=10.0,
            sell_at_top=False,
            sell_num=10.0,
            sell_unit="fundPercent",
            profit_rate=0.0,  # disabled threshold
            sell_macd_point=0.0,  # disabled gating
            buy_macd_point=None,
        ),
    )

    assert len(out["actions"]) == 2
    assert out["actions"][0]["type"] == "buy"
    assert out["actions"][0]["reason"] == "initial"
    assert out["actions"][0]["date"] == "2026-01-02"
    assert out["actions"][1]["type"] == "sell"
    assert out["actions"][1]["reason"] == "stop_profit"
    assert out["actions"][1]["date"] == "2026-01-03"
