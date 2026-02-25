from __future__ import annotations

from typing import Any

from app.qbot_xalpha.bte_engine import BteEngine, FeeCfg


def run_indicator_cross_backtest(
    *,
    code: str,
    series: list[dict[str, Any]],
    open_dates: list[str],
    start: str,
    end: str,
    totmoney: float,
    col: tuple[str, str],
    fee: FeeCfg,
) -> dict[str, Any]:
    od = sorted([str(d).strip() for d in open_dates if str(d).strip()])
    od = [d for d in od if d >= start and d <= end]
    if len(od) < 2:
        return {"actions": [], "summary": {"final_equity": float(totmoney)}}

    rows_by_date: dict[str, dict[str, Any]] = {}
    for row in series:
        d = str(row.get("date", "")).strip()
        if d:
            rows_by_date[d] = dict(row)

    engine = BteEngine(series_map={code: [dict(p) for p in series]}, fees={code: fee}, open_dates=od, initial_cash=float(totmoney))
    holding = False

    left_col, right_col = col
    prev_row: dict[str, Any] | None = None
    prev_date: str | None = None
    for d in od:
        row = rows_by_date.get(d)
        if row is None:
            continue
        if prev_row is None:
            prev_row = row
            prev_date = d
            continue

        try:
            valuel = float(row.get(left_col))
            valuer = float(row.get(right_col))
            valuelb = float(prev_row.get(left_col))
            valuerb = float(prev_row.get(right_col))
        except Exception:
            prev_row = row
            prev_date = d
            continue

        cond = (valuerb - valuelb) * (valuer - valuel)
        if cond > 0:
            prev_row = row
            prev_date = d
            continue
        if cond == 0 and (valuer - valuel == 0):
            prev_row = row
            prev_date = d
            continue

        # cross confirmed on this date
        if valuer > valuel:
            if holding:
                engine.sell(code, -0.005, d)
                holding = False
        else:
            if not holding and engine.cash > 0:
                engine.buy(code, engine.cash, d)
                holding = True

        prev_row = row
        prev_date = d

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

