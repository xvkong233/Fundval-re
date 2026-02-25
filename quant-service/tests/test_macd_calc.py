from app.indicators.macd import calc_macd


def test_calc_macd_constant_series_yields_zero_macd():
    series = [
        {"date": "2026-01-01", "val": 1.0},
        {"date": "2026-01-02", "val": 1.0},
        {"date": "2026-01-03", "val": 1.0},
        {"date": "2026-01-04", "val": 1.0},
        {"date": "2026-01-05", "val": 1.0},
    ]
    points = calc_macd(series)
    assert len(points) == len(series)
    assert all(abs(p["macd"]) < 1e-12 for p in points)
    assert points[0]["macd_position"] == 0

