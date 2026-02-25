from app.xalpha_like.metrics import metrics_from_series


def test_metrics_from_series_accepts_non_date_points():
    series = [
        {"index": 0, "date": "f+1", "val": 1.0},
        {"index": 1, "date": "f+2", "val": 1.01},
        {"index": 2, "date": "f+3", "val": 1.02},
    ]
    out = metrics_from_series(series, risk_free_annual=0.0)
    assert "metrics" in out
    assert out["metrics"]["total_return"] > 0
    assert isinstance(out["metrics"]["cagr"], float)

