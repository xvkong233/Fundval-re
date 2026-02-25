from app.indicators.macd import txn_by_macd


def test_txn_by_macd_marks_sell_threshold_in_positive_wave():
    points = [
        {"index": 0, "macd": 1.0},
        {"index": 1, "macd": 4.0},  # peak
        {"index": 2, "macd": 2.0},  # reaches 50% of peak -> sell
        {"index": 3, "macd": 1.0},
    ]
    out = txn_by_macd(points, sell_position=0.5, buy_position=0.5)
    assert out[2]["txn_type"] == "sell"
    assert out[2]["txnType"] == "sell"


def test_txn_by_macd_marks_buy_threshold_in_negative_wave():
    points = [
        {"index": 0, "macd": -1.0},
        {"index": 1, "macd": -4.0},  # peak abs
        {"index": 2, "macd": -2.0},  # reaches 50% -> buy
        {"index": 3, "macd": -1.0},
    ]
    out = txn_by_macd(points, sell_position=0.5, buy_position=0.5)
    assert out[2]["txn_type"] == "buy"
    assert out[2]["txnType"] == "buy"


def test_txn_by_macd_can_mark_sell_and_buy_in_one_series():
    points = [
        {"index": 0, "macd": 1.0},
        {"index": 1, "macd": 10.0},  # peak positive
        {"index": 2, "macd": 8.0},
        {"index": 3, "macd": 7.0},  # <= 0.75 * 10 -> sell
        {"index": 4, "macd": -1.0},  # cross
        {"index": 5, "macd": -4.0},  # peak negative abs
        {"index": 6, "macd": -3.0},
        {"index": 7, "macd": -2.0},  # <= 0.5 * 4 -> buy
    ]
    out = txn_by_macd(points, sell_position=0.75, buy_position=0.5)
    assert out[3]["txn_type"] == "sell"
    assert out[7]["txn_type"] == "buy"
