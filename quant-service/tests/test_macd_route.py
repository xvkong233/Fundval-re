from fastapi.testclient import TestClient

from app.main import app


def test_macd_route_marks_txn_types():
    c = TestClient(app)
    payload = {
        "points": [
            {"index": 0, "macd": 1.0},
            {"index": 1, "macd": 4.0},
            {"index": 2, "macd": 2.0},
            {"index": 3, "macd": 1.0},
        ],
        "sell_position": 0.5,
        "buy_position": 0.5,
    }
    r = c.post("/api/quant/macd", json=payload)
    assert r.status_code == 200
    data = r.json()
    assert data["points"][2]["txn_type"] == "sell"


def test_macd_route_accepts_series_and_returns_macd_fields():
    c = TestClient(app)
    payload = {
        "series": [
            {"date": "2026-01-01", "val": 1.0},
            {"date": "2026-01-02", "val": 1.0},
            {"date": "2026-01-03", "val": 1.0},
            {"date": "2026-01-04", "val": 1.0},
            {"date": "2026-01-05", "val": 1.0},
        ],
        "sell_position": 0.5,
        "buy_position": 0.5,
    }
    r = c.post("/api/quant/macd", json=payload)
    assert r.status_code == 200
    data = r.json()
    assert len(data["points"]) == 5
    assert "macd" in data["points"][0]
    assert abs(data["points"][0]["macd"]) < 1e-12

