from __future__ import annotations


def test_xalpha_trade_mul_summary_contains_total_row() -> None:
    import pandas as pd

    from app.qbot_xalpha_compat.mul import Mul
    from app.qbot_xalpha_compat.trade import Trade

    series = [
        {"date": "2026-01-02", "netvalue": 1.00},
        {"date": "2026-01-03", "netvalue": 1.10},
        {"date": "2026-01-04", "netvalue": 1.20},
    ]

    status = pd.DataFrame(
        {
            "date": ["2026-01-02", "2026-01-04"],
            "F000001": [100.0, -0.005],  # buy 100, then sell all
        }
    )

    t = Trade(
        code="F000001",
        name="Test Fund",
        price_series=series,
        status=status,
        buy_fee_rate=0.0015,
        sell_fee_rate=0.005,
        round_label=2,
    )
    m = Mul(t)

    df = m.summary("2026-01-04")
    assert list(df.columns)[:3] == ["基金名称", "基金代码", "当日净值"]
    assert (df["基金名称"] == "总计").any()

