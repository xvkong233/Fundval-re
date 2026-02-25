from __future__ import annotations

from fastapi.testclient import TestClient

from app.main import app


def test_fund_strategies_compare_returns_series_per_strategy() -> None:
    c = TestClient(app)
    r = c.post(
        "/api/quant/fund-strategies/compare",
        json={
            "fund_series": [
                {"date": "2026-01-30", "val": 1.0},
                {"date": "2026-02-02", "val": 1.0},
            ],
            "shangzheng_series": [
                {"date": "2026-01-30", "val": 3000.0},
                {"date": "2026-02-02", "val": 3000.0},
            ],
            "refer_index_points": [],
            "strategies": [
                {"name": "策略A", "cfg": {"total_amount": 100.0, "salary": 10.0, "fixed_amount": 50.0, "period": ["monthly", 1]}},
                {"name": "策略B", "cfg": {"total_amount": 100.0, "salary": 0.0, "fixed_amount": 0.0, "period": ["monthly", 1]}},
            ],
        },
    )
    assert r.status_code == 200
    j = r.json()
    assert set(j["strategies"].keys()) == {"策略A", "策略B"}
    assert isinstance(j["strategies"]["策略A"]["series"], list)
    assert j["strategies"]["策略A"]["series"][0]["date"] == "2026-01-30"

