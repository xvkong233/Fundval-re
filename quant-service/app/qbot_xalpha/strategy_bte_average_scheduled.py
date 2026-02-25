from __future__ import annotations

from typing import Any

from app.qbot_xalpha.bte_engine import BteEngine, FeeCfg


def run_average_scheduled_backtest(
    *,
    code: str,
    series: list[dict[str, Any]],
    open_dates: list[str],
    start: str,
    end: str,
    totmoney: float,
    times: list[str],
    value: float,
    fee: FeeCfg,
) -> dict[str, Any]:
    od = sorted([str(d).strip() for d in open_dates if str(d).strip()])
    od = [d for d in od if d >= start and d <= end]
    if not od:
        return {"actions": [], "summary": {"final_equity": float(totmoney)}}

    engine = BteEngine(series_map={code: [dict(p) for p in series]}, fees={code: fee}, open_dates=od, initial_cash=float(totmoney))
    times_set = {str(d).strip() for d in times if str(d).strip()}
    aim = 0.0

    for d in od:
        if d not in times_set:
            continue
        aim += float(value)
        nav = engine.nav(code, d)
        if nav is None or nav <= 0:
            continue
        cur_share = float(engine.holdings.get(code, 0.0))
        current_value = cur_share * float(nav)
        if aim > current_value:
            engine.buy(code, aim - current_value, d)
        elif aim < current_value:
            share_to_sell = (current_value - aim) / float(nav)
            engine.sell(code, share_to_sell, d)

    final_date = od[-1]
    return {
        "actions": engine.actions,
        "summary": {
            "final_date": final_date,
            "final_equity": engine.equity(final_date),
            "cash": engine.cash,
            "holdings": engine.holdings,
        },
    }

