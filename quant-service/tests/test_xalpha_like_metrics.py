from fastapi.testclient import TestClient

from app.main import app


def test_metrics_constant_series_has_zero_drawdown_and_return():
    c = TestClient(app)
    payload = {
        "series": [
            {"date": "2026-01-01", "val": 1.0},
            {"date": "2026-01-02", "val": 1.0},
            {"date": "2026-01-03", "val": 1.0},
        ]
    }
    r = c.post("/api/quant/xalpha/metrics", json=payload)
    assert r.status_code == 200
    data = r.json()
    assert abs(data["metrics"]["total_return"]) < 1e-12
    assert abs(data["metrics"]["max_drawdown"]) < 1e-12
    assert all(abs(p["drawdown"]) < 1e-12 for p in data["drawdown_series"])

