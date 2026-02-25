from fastapi.testclient import TestClient

from app.main import app


def test_grid_emits_buy_then_sell_on_step_crossing():
    c = TestClient(app)
    payload = {
        "series": [
            {"date": "2026-01-01", "val": 1.00},
            {"date": "2026-01-02", "val": 0.98},  # -2% -> buy
            {"date": "2026-01-03", "val": 1.00},  # +2% from 0.98 -> sell
        ],
        "grid_step_pct": 0.02,
    }
    r = c.post("/api/quant/xalpha/grid", json=payload)
    assert r.status_code == 200
    actions = r.json()["actions"]
    assert [a["action"] for a in actions] == ["buy", "sell"]

