from __future__ import annotations

from typing import Any

from app.qbot_xalpha.bte_engine import BteEngine, FeeCfg
from app.qbot_xalpha.rounding import myround


def run_grid_backtest(
    *,
    code: str,
    series: list[dict[str, Any]],
    open_dates: list[str],
    start: str,
    end: str,
    totmoney: float,
    buypercent: list[float],
    sellpercent: list[float],
    fee: FeeCfg,
) -> dict[str, Any]:
    if len(buypercent) != len(sellpercent) or not buypercent:
        return {"actions": [], "summary": {"final_equity": 0.0}}

    od = sorted([str(d).strip() for d in open_dates if str(d).strip()])
    od = [d for d in od if d >= start and d <= end]
    if not od:
        return {"actions": [], "summary": {"final_equity": 0.0}}

    engine = BteEngine(series_map={code: [dict(p) for p in series]}, fees={code: fee}, open_dates=od, initial_cash=float(totmoney))

    zero = engine.nav(code, start)
    if zero is None or zero <= 0:
        return {"actions": [], "summary": {"final_equity": 0.0}}

    tmp = [float(zero)]
    for bp in buypercent:
        tmp.append(tmp[-1] * (1.0 - float(bp) / 100.0))
    buypts = tmp[1:]
    sellpts = [tmp[i + 1] * (1.0 + float(sp) / 100.0) for i, sp in enumerate(sellpercent)]

    division = len(buypts)
    unit = myround(float(totmoney) / float(division))
    pos = 0

    prev_val: float | None = None
    for d in od:
        cur_val = engine.nav(code, d)
        if cur_val is None or cur_val <= 0:
            prev_val = cur_val
            continue

        if d == start:
            if float(buypercent[0]) == 0.0:
                pos += 1
                engine.buy(code, unit, d)
            prev_val = float(cur_val)
            continue

        if prev_val is None or prev_val <= 0:
            prev_val = float(cur_val)
            continue

        value = float(cur_val)
        valueb = float(prev_val)

        for i, buypt in enumerate(buypts):
            if (value - float(buypt)) <= 0 and (valueb - float(buypt)) > 0 and pos <= i:
                pos += 1
                engine.buy(code, unit, d)

        for j, sellpt in enumerate(sellpts):
            if (value - float(sellpt)) >= 0 and (valueb - float(sellpt)) < 0 and pos > j and pos > 0:
                ratio = 1.0 / float(pos)
                engine.sell(code, -0.005 * ratio, d)
                pos -= 1

        prev_val = value

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

