from fastapi.testclient import TestClient

from app.main import app


def test_xalpha_backtest_scheduled_basic():
    """
    最小可用性：给定序列 + 指定 open_dates，scheduled 策略能产出买入动作并计算最终权益。
    """
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "scheduled",
            "start": "2026-01-01",
            "end": "2026-01-03",
            "totmoney": 1_000_000,
            "calendar": {"open_dates": ["2026-01-01", "2026-01-02", "2026-01-03"]},
            "series": {
                "F000001": [
                    {"date": "2026-01-01", "val": 1.0},
                    {"date": "2026-01-02", "val": 1.0},
                    {"date": "2026-01-03", "val": 1.0},
                ]
            },
            "params": {"code": "F000001", "value": 1000.0, "times": ["2026-01-01", "2026-01-03"]},
            "fees": {"F000001": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    assert data["strategy"] == "scheduled"
    assert data["actions"][0]["type"] == "buy"
    assert len(data["actions"]) == 2
    assert data["summary"]["final_equity"] == 2000.0


def test_xalpha_backtest_bte_scheduled_basic():
    """
    BTE 语义：totmoney 作为初始现金，定投 value 从现金扣减，final_equity=现金+持仓市值。
    """
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "bte_scheduled",
            "start": "2026-01-01",
            "end": "2026-01-03",
            "totmoney": 10_000.0,
            "calendar": {"open_dates": ["2026-01-01", "2026-01-02", "2026-01-03"]},
            "series": {
                "F000001": [
                    {"date": "2026-01-01", "val": 1.0},
                    {"date": "2026-01-02", "val": 1.0},
                    {"date": "2026-01-03", "val": 1.0},
                ]
            },
            "params": {"code": "F000001", "value": 1000.0, "times": ["2026-01-01", "2026-01-03"]},
            "fees": {"F000001": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    assert data["strategy"] == "bte_scheduled"
    assert len(data["actions"]) == 2
    assert data["summary"]["final_equity"] == 10_000.0
    assert data["summary"]["cash"] == 8000.0


def test_xalpha_backtest_buyandhold_policy_basic():
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "buyandhold",
            "start": "2026-01-01",
            "end": "2026-01-03",
            "totmoney": 1000.0,
            "calendar": {"open_dates": ["2026-01-01", "2026-01-02", "2026-01-03"]},
            "series": {
                "F000001": [
                    {"date": "2026-01-01", "val": 1.0},
                    {"date": "2026-01-02", "val": 1.5},
                    {"date": "2026-01-03", "val": 2.0},
                ]
            },
            "params": {"code": "F000001"},
            "fees": {"F000001": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    assert data["strategy"] == "buyandhold"
    assert len(data["actions"]) == 1
    assert data["summary"]["final_equity"] == 2000.0


def test_xalpha_backtest_buyandhold_summary_report_output():
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "buyandhold",
            "totmoney": 1000.0,
            "calendar": {"open_dates": ["2026-01-02", "2026-01-03"]},
            "series": {
                "F000001": [
                    {"date": "2026-01-02", "netvalue": 1.0},
                    {"date": "2026-01-03", "netvalue": 1.1},
                ]
            },
            "params": {"code": "F000001", "output": "summary"},
            "fees": {"F000001": {"buy_fee_rate": 0.0015, "sell_fee_rate": 0.005, "round_label": 2}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    rows = (data.get("report") or {}).get("summary") or []
    assert isinstance(rows, list)
    assert any(row.get("基金名称") == "总计" for row in rows)


def test_xalpha_backtest_scheduled_tune_policy_basic():
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "scheduled_tune",
            "start": "2026-01-01",
            "end": "2026-01-03",
            "calendar": {"open_dates": ["2026-01-01", "2026-01-02", "2026-01-03"]},
            "series": {
                "F000001": [
                    {"date": "2026-01-01", "val": 0.8},
                    {"date": "2026-01-02", "val": 1.2},
                    {"date": "2026-01-03", "val": 2.5},
                ]
            },
            "params": {
                "code": "F000001",
                "value": 1000.0,
                "times": ["2026-01-01", "2026-01-02", "2026-01-03"],
                "piece": [[1.0, 2.0], [2.0, 1.0]],
            },
            "fees": {"F000001": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    assert data["strategy"] == "scheduled_tune"
    assert len(data["actions"]) == 2
    assert data["summary"]["final_equity"] == 8333.33


def test_xalpha_backtest_scheduled_window_policy_basic():
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "scheduled_window",
            "start": "2026-01-01",
            "end": "2026-01-05",
            "calendar": {
                "open_dates": [
                    "2026-01-01",
                    "2026-01-02",
                    "2026-01-03",
                    "2026-01-04",
                    "2026-01-05",
                ]
            },
            "series": {
                "F000001": [
                    {"date": "2026-01-01", "val": 100.0},
                    {"date": "2026-01-02", "val": 100.0},
                    {"date": "2026-01-03", "val": 80.0},
                    {"date": "2026-01-04", "val": 80.0},
                    {"date": "2026-01-05", "val": 80.0},
                ]
            },
            "params": {
                "code": "F000001",
                "value": 1000.0,
                "times": [
                    "2026-01-01",
                    "2026-01-02",
                    "2026-01-03",
                    "2026-01-04",
                    "2026-01-05",
                ],
                "window": 2,
                "window_dist": 1,
                "method": "AVG",
                "piece": [[-10.0, 2.0], [0.0, 1.0], [10.0, 0.5]],
            },
            "fees": {"F000001": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    assert data["strategy"] == "scheduled_window"
    assert len(data["actions"]) == 3
    assert data["summary"]["final_equity"] == 5000.0


def test_xalpha_backtest_grid_basic():
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "grid",
            "start": "2026-01-01",
            "end": "2026-01-03",
            "totmoney": 2000.0,
            "calendar": {"open_dates": ["2026-01-01", "2026-01-02", "2026-01-03"]},
            "series": {
                "F000001": [
                    {"date": "2026-01-01", "val": 100.0},
                    {"date": "2026-01-02", "val": 80.0},
                    {"date": "2026-01-03", "val": 90.0},
                ]
            },
            "params": {"code": "F000001", "buypercent": [0.0, 20.0], "sellpercent": [10.0, 10.0]},
            "fees": {"F000001": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    assert data["strategy"] == "grid"
    assert data["summary"]["final_equity"] == 2025.0


def test_xalpha_backtest_indicator_cross_basic():
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "indicator_cross",
            "start": "2026-01-01",
            "end": "2026-01-03",
            "totmoney": 1000.0,
            "calendar": {"open_dates": ["2026-01-01", "2026-01-02", "2026-01-03"]},
            "series": {
                "F000001": [
                    {"date": "2026-01-01", "val": 1.0, "ma": 1.1},
                    {"date": "2026-01-02", "val": 1.2, "ma": 1.1},
                    {"date": "2026-01-03", "val": 1.0, "ma": 1.1},
                ]
            },
            "params": {"code": "F000001", "col": ["val", "ma"]},
            "fees": {"F000001": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    assert data["strategy"] == "indicator_cross"
    assert len(data["actions"]) == 2
    assert data["summary"]["final_equity"] == 833.33


def test_xalpha_backtest_indicator_points_basic():
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "indicator_points",
            "start": "2026-01-01",
            "end": "2026-01-03",
            "totmoney": 1000.0,
            "calendar": {"open_dates": ["2026-01-01", "2026-01-02", "2026-01-03"]},
            "series": {
                "F000001": [
                    {"date": "2026-01-01", "val": 1.2},
                    {"date": "2026-01-02", "val": 1.0},
                    {"date": "2026-01-03", "val": 1.1},
                ]
            },
            "params": {
                "code": "F000001",
                "col": "val",
                "buylow": True,
                "buy": [[1.0, 1.0]],
                "sell": [[1.1, 1.0]],
            },
            "fees": {"F000001": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    assert data["strategy"] == "indicator_points"
    assert len(data["actions"]) == 2
    assert data["summary"]["final_equity"] == 1100.0


def test_xalpha_backtest_bte_average_scheduled_basic():
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "bte_average_scheduled",
            "start": "2026-01-01",
            "end": "2026-01-03",
            "totmoney": 10_000.0,
            "calendar": {"open_dates": ["2026-01-01", "2026-01-02", "2026-01-03"]},
            "series": {
                "F000001": [
                    {"date": "2026-01-01", "val": 1.0},
                    {"date": "2026-01-02", "val": 4.0},
                    {"date": "2026-01-03", "val": 4.0},
                ]
            },
            "params": {"code": "F000001", "value": 1000.0, "times": ["2026-01-01", "2026-01-02"]},
            "fees": {"F000001": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    assert data["strategy"] == "bte_average_scheduled"
    assert len(data["actions"]) == 2
    assert data["summary"]["final_equity"] == 13000.0


def test_xalpha_backtest_bte_scheduled_sell_on_xirr_basic():
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "bte_scheduled_sell_on_xirr",
            "start": "2026-01-01",
            "end": "2026-01-03",
            "totmoney": 10_000.0,
            "calendar": {"open_dates": ["2026-01-01", "2026-01-02", "2026-01-03"]},
            "series": {
                "F000001": [
                    {"date": "2026-01-01", "val": 1.0},
                    {"date": "2026-01-02", "val": 1.1},
                    {"date": "2026-01-03", "val": 1.1},
                ]
            },
            "params": {
                "code": "F000001",
                "value": 1000.0,
                "times": ["2026-01-01", "2026-01-02", "2026-01-03"],
                "threhold": 0.05,
                "holding_time": 0,
                "check_weekday": 4,
            },
            "fees": {"F000001": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    assert data["strategy"] == "bte_scheduled_sell_on_xirr"
    assert len(data["actions"]) == 2
    assert data["actions"][0]["type"] == "buy"
    assert data["actions"][1]["type"] == "sell"
    assert data["summary"]["final_equity"] == 10100.0


def test_xalpha_backtest_bte_tendency28_basic():
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "bte_tendency28",
            "start": "2026-01-01",
            "end": "2026-01-05",
            "totmoney": 1000.0,
            "calendar": {
                "open_dates": [
                    "2026-01-01",
                    "2026-01-02",
                    "2026-01-03",
                    "2026-01-04",
                    "2026-01-05",
                ]
            },
            "series": {
                "AIM0": [
                    {"date": "2026-01-01", "val": 1.0},
                    {"date": "2026-01-02", "val": 1.0},
                    {"date": "2026-01-03", "val": 1.0},
                    {"date": "2026-01-04", "val": 1.0},
                    {"date": "2026-01-05", "val": 1.0},
                ],
                "AIM1": [
                    {"date": "2026-01-01", "val": 100.0},
                    {"date": "2026-01-02", "val": 105.0},
                    {"date": "2026-01-03", "val": 100.0},
                    {"date": "2026-01-04", "val": 110.0},
                    {"date": "2026-01-05", "val": 110.0},
                ],
                "AIM2": [
                    {"date": "2026-01-01", "val": 100.0},
                    {"date": "2026-01-02", "val": 100.0},
                    {"date": "2026-01-03", "val": 100.0},
                    {"date": "2026-01-04", "val": 100.0},
                    {"date": "2026-01-05", "val": 100.0},
                ],
            },
            "params": {
                "aim0": "AIM0",
                "aim1": "AIM1",
                "aim2": "AIM2",
                "check_dates": ["2026-01-03"],
                "prev": 1,
                "upthrehold": 1.0,
                "diffthrehold": 1.0,
                "initial_money": 500.0,
            },
            "fees": {"AIM0": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}, "AIM1": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}, "AIM2": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    assert data["strategy"] == "bte_tendency28"
    assert data["summary"]["final_equity"] == 1100.0


def test_xalpha_backtest_bte_balance_basic():
    c = TestClient(app)
    r = c.post(
        "/api/quant/xalpha/backtest",
        json={
            "strategy": "bte_balance",
            "start": "2026-01-01",
            "end": "2026-01-02",
            "totmoney": 1000.0,
            "calendar": {"open_dates": ["2026-01-01", "2026-01-02"]},
            "series": {
                "A": [{"date": "2026-01-01", "val": 1.0}, {"date": "2026-01-02", "val": 2.0}],
                "B": [{"date": "2026-01-01", "val": 1.0}, {"date": "2026-01-02", "val": 1.0}],
            },
            "params": {
                "check_dates": ["2026-01-02"],
                "portfolio_dict": {"A": 0.5, "B": 0.5},
            },
            "fees": {"A": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}, "B": {"buy_fee_rate": 0.0, "sell_fee_rate": 0.0}},
        },
    )
    assert r.status_code == 200
    data = r.json()
    assert data["strategy"] == "bte_balance"
    assert len(data["actions"]) == 4
    assert data["summary"]["final_equity"] == 1500.0
