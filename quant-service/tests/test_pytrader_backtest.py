from __future__ import annotations

from fastapi.testclient import TestClient

from app.main import app


def test_pytrader_backtest_macd_cross_basic() -> None:
    c = TestClient(app)
    r = c.post(
        "/api/quant/pytrader/backtest",
        json={
            "strategy": "macd_cross",
            "totmoney": 1000.0,
            "series": [
                {"date": "2026-01-01", "val": 1.0},
                {"date": "2026-01-02", "val": 1.1},
                {"date": "2026-01-03", "val": 1.0},
                {"date": "2026-01-04", "val": 1.2},
            ],
            "params": {"sell_position": 0.7, "buy_position": 0.7},
            "fees": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0, "round_label": 2},
        },
    )
    assert r.status_code == 200
    j = r.json()
    assert j["strategy"] == "macd_cross"
    assert isinstance(j["equity_curve"], list)
    assert j["equity_curve"][-1]["equity"] >= 0

