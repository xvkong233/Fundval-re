from __future__ import annotations

from typing import Any

from app.qbot_xalpha.bte_engine import BteEngine, FeeCfg
from app.qbot_xalpha.rounding import myround


def _normalize_levels(levels: list[tuple[float, float]]) -> list[tuple[float, float]]:
    div = sum(float(w) for _, w in levels) if levels else 0.0
    if div <= 0:
        return []
    return [(float(p), float(w) / div) for p, w in levels]


def run_indicator_points_backtest(
    *,
    code: str,
    series: list[dict[str, Any]],
    open_dates: list[str],
    start: str,
    end: str,
    totmoney: float,
    col: str,
    buy: list[tuple[float, float]],
    sell: list[tuple[float, float]] | None,
    buylow: bool,
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

    buy_terms = _normalize_levels(buy)
    sell_terms = _normalize_levels(sell or []) if sell is not None else None

    engine = BteEngine(series_map={code: [dict(p) for p in series]}, fees={code: fee}, open_dates=od, initial_cash=float(totmoney))

    pos = 0.0
    selllevel = 0
    judge = 1.0 if buylow else -1.0

    prev_row: dict[str, Any] | None = None
    for d in od:
        row = rows_by_date.get(d)
        if row is None:
            continue
        if prev_row is None:
            prev_row = row
            continue
        try:
            value = float(row.get(col))
            valueb = float(prev_row.get(col))
        except Exception:
            prev_row = row
            continue

        # buy levels
        for i, (pt, frac) in enumerate(buy_terms):
            tail = sum(f for _, f in buy_terms[i:])
            if judge * (value - pt) <= 0 < judge * (valueb - pt) and pos + tail <= 1.0 + 1e-9:
                pos += frac
                buy_amount = myround(float(totmoney) * float(frac))
                engine.buy(code, buy_amount, d)
                selllevel = 0

        # sell levels
        if sell_terms is not None:
            for i, (pt, frac) in enumerate(sell_terms):
                if not (judge * (value - pt) >= 0 > judge * (valueb - pt)):
                    continue
                if pos <= 0:
                    continue
                if selllevel > i:
                    continue
                denom = sum(f for _, f in sell_terms[i:])
                if denom <= 0:
                    continue
                delta = myround(float(frac) / float(denom))
                if delta <= 0:
                    continue
                engine.sell(code, -0.005 * float(delta), d)
                pos = (1.0 - float(delta)) * pos
                selllevel = i + 1

        prev_row = row

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

