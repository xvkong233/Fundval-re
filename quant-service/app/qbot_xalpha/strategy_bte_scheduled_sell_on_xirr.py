from __future__ import annotations

from datetime import date as _date
from typing import Any

from app.qbot_xalpha.bte_engine import BteEngine, FeeCfg
from app.qbot_xalpha.xirr import xirr


def _weekday(date_str: str) -> int:
    return _date.fromisoformat(str(date_str).strip()[:10]).weekday()


def run_scheduled_sell_on_xirr_backtest(
    *,
    code: str,
    series: list[dict[str, Any]],
    open_dates: list[str],
    start: str,
    end: str,
    totmoney: float,
    times: list[str],
    value: float,
    threhold: float,
    holding_time: int,
    check_weekday: int,
    fee: FeeCfg,
) -> dict[str, Any]:
    od = sorted([str(d).strip() for d in open_dates if str(d).strip()])
    od = [d for d in od if d >= start and d <= end]
    if not od:
        return {"actions": [], "summary": {"final_equity": float(totmoney)}}

    engine = BteEngine(series_map={code: [dict(p) for p in series]}, fees={code: fee}, open_dates=od, initial_cash=float(totmoney))
    times_set = {str(d).strip() for d in times if str(d).strip()}
    sold = False

    start_dt = _date.fromisoformat(start[:10])
    for d in od:
        if not sold and int(_weekday(d)) == int(check_weekday) and ( _date.fromisoformat(d[:10]) - start_dt).days > int(holding_time):
            nav = engine.nav(code, d)
            if nav is not None and nav > 0:
                share = float(engine.holdings.get(code, 0.0))
                redemption = share * float(nav) * (1.0 - float(fee.sell_fee_rate))
                cfs = [(str(it["date"]), float(it["cash"])) for it in engine.cashflows if str(it.get("code","")) == code]
                if redemption > 0:
                    cfs.append((d, float(redemption)))
                rate = xirr(cfs, guess=0.1) if len(cfs) >= 2 else 0.0
                if rate > float(threhold):
                    engine.sell(code, -0.005, d)
                    sold = True

        if not sold and d in times_set:
            engine.buy(code, float(value), d)

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

