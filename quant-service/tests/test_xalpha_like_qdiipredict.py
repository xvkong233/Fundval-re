from fastapi.testclient import TestClient

from app.main import app


def test_qdiipredict_multiplies_last_value_by_weighted_delta():
    c = TestClient(app)
    payload = {
        "last_value": 1.0,
        "legs": [
            {"code": "IDX_US", "percent": 60.0, "ratio": 1.02, "currency_ratio": 1.0},
            {"code": "IDX_EU", "percent": 30.0, "ratio": 0.99, "currency_ratio": 1.0},
        ],
    }
    r = c.post("/api/quant/xalpha/qdiipredict", json=payload)
    assert r.status_code == 200
    data = r.json()
    # delta = 0.6*1.02 + 0.3*0.99 + 0.1*1.0 = 1.009
    assert abs(data["delta"] - 1.009) < 1e-12
    assert abs(data["predicted_value"] - 1.009) < 1e-12


def test_qdiipredict_applies_currency_ratio_per_leg():
    c = TestClient(app)
    payload = {
        "last_value": 2.0,
        "legs": [
            {"code": "IDX_US", "percent": 100.0, "ratio": 1.01, "currency_ratio": 1.002},
        ],
    }
    r = c.post("/api/quant/xalpha/qdiipredict", json=payload)
    assert r.status_code == 200
    data = r.json()
    assert abs(data["delta"] - (1.01 * 1.002)) < 1e-12
    assert abs(data["predicted_value"] - (2.0 * 1.01 * 1.002)) < 1e-12

