from __future__ import annotations

from typing import Any

from app.qbot_xalpha.bte_engine import BteEngine, FeeCfg
from app.qbot_xalpha.series import normalize_series


def _momentum_up_pct(points: list[dict[str, Any]], *, date: str, prev: int) -> float:
    pts = normalize_series(points)
    hist = [p for p in pts if p.date < date]
    if len(hist) < prev + 1:
        return 0.0
    last = float(hist[-1].val)
    base = float(hist[-1 - prev].val)
    if base <= 0:
        return 0.0
    return (last - base) / base * 100.0


def run_tendency28_backtest(
    *,
    series_map: dict[str, list[dict[str, Any]]],
    fees: dict[str, FeeCfg],
    open_dates: list[str],
    start: str,
    end: str,
    totmoney: float,
    aim0: str,
    aim1: str,
    aim2: str,
    check_dates: list[str],
    upthrehold: float,
    diffthrehold: float,
    prev: int,
    initial_money: float,
) -> dict[str, Any]:
    od = sorted([str(d).strip() for d in open_dates if str(d).strip()])
    od = [d for d in od if d >= start and d <= end]
    if not od:
        return {"actions": [], "summary": {"final_equity": float(totmoney)}}

    engine = BteEngine(series_map={k: [dict(p) for p in v] for k, v in series_map.items()}, fees=fees, open_dates=od, initial_cash=float(totmoney))
    status = 0  # 0: aim0, 1: aim1, 2: aim2

    if initial_money > 0:
        engine.buy(aim0, float(initial_money), start)

    check_set = {str(d).strip() for d in check_dates if str(d).strip()}
    p = int(prev)
    for d in od:
        if d not in check_set:
            continue
        up1 = _momentum_up_pct(series_map.get(aim1, []), date=d, prev=p)
        up2 = _momentum_up_pct(series_map.get(aim2, []), date=d, prev=p)

        if up1 < float(upthrehold) and up2 < float(upthrehold):
            if status == 1:
                engine.sell(aim1, -0.005, d)
                status = 0
                engine.buy(aim0, engine.cash, d)
            elif status == 2:
                engine.sell(aim2, -0.005, d)
                status = 0
                engine.buy(aim0, engine.cash, d)
        elif up1 > float(upthrehold) and up1 > up2:
            if status == 0:
                engine.sell(aim0, -0.005, d)
                status = 1
                engine.buy(aim1, engine.cash, d)
            elif status == 2 and (up1 - up2) > float(diffthrehold):
                engine.sell(aim2, -0.005, d)
                status = 1
                engine.buy(aim1, engine.cash, d)
        elif up2 > float(upthrehold) and up2 > up1:
            if status == 0:
                engine.sell(aim0, -0.005, d)
                status = 2
                engine.buy(aim2, engine.cash, d)
            elif status == 1 and (up2 - up1) > float(diffthrehold):
                engine.sell(aim1, -0.005, d)
                status = 2
                engine.buy(aim2, engine.cash, d)

    final_date = od[-1]
    return {
        "actions": engine.actions,
        "summary": {
            "final_date": final_date,
            "final_equity": engine.equity(final_date),
            "cash": engine.cash,
            "holdings": engine.holdings,
            "status": status,
        },
    }

