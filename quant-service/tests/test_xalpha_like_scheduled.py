from fastapi.testclient import TestClient

from app.main import app


def test_scheduled_buys_every_n_days():
    c = TestClient(app)
    payload = {
        "series": [
            {"date": "2026-01-01", "val": 1.00},
            {"date": "2026-01-02", "val": 1.01},
            {"date": "2026-01-03", "val": 1.02},
            {"date": "2026-01-04", "val": 1.03},
            {"date": "2026-01-05", "val": 1.04},
        ],
        "every_n": 2,
    }
    r = c.post("/api/quant/xalpha/scheduled", json=payload)
    assert r.status_code == 200
    actions = r.json()["actions"]
    assert [a["index"] for a in actions] == [0, 2, 4]

