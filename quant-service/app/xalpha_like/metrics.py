from __future__ import annotations

import math
from datetime import date
from typing import Any


def _try_parse_date(s: str) -> date | None:
    s = (s or "").strip()
    if not s:
        return None
    try:
        # accept YYYY-MM-DD
        return date.fromisoformat(s[:10])
    except Exception:
        return None


def _sample_std(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0
    m = sum(values) / float(len(values))
    var = sum((x - m) ** 2 for x in values) / float(len(values) - 1)
    return math.sqrt(var)


def metrics_from_series(series: list[dict[str, Any]], risk_free_annual: float = 0.0) -> dict[str, Any]:
    if not series:
        return {
            "metrics": {
                "total_return": 0.0,
                "cagr": 0.0,
                "vol_annual": 0.0,
                "sharpe": 0.0,
                "max_drawdown": 0.0,
            },
            "drawdown_series": [],
        }

    points = []
    for i, row in enumerate(series):
        points.append(
            {
                "index": int(row.get("index", i)),
                "date": str(row.get("date", "")).strip(),
                "val": float(row.get("val", 0.0)),
            }
        )

    # keep input order unless all dates parseable then sort by date
    parsed_dates = [_try_parse_date(p["date"]) for p in points]
    if all(d is not None for d in parsed_dates):
        points = [p for _, p in sorted(zip(parsed_dates, points, strict=True), key=lambda x: x[0])]
        parsed_dates = sorted([d for d in parsed_dates if d is not None])

    first_val = float(points[0]["val"])
    last_val = float(points[-1]["val"])
    total_return = 0.0 if first_val <= 0 else (last_val / first_val - 1.0)

    # drawdown
    peak = float(points[0]["val"])
    max_drawdown = 0.0
    drawdown_series: list[dict[str, Any]] = []
    for p in points:
        v = float(p["val"])
        if v > peak:
            peak = v
        dd = 0.0 if peak <= 0 else (v / peak - 1.0)
        if dd < max_drawdown:
            max_drawdown = dd
        drawdown_series.append({"index": int(p["index"]), "date": p["date"], "drawdown": dd})

    # daily returns
    daily_returns: list[float] = []
    for prev, cur in zip(points[:-1], points[1:], strict=False):
        a = float(prev["val"])
        b = float(cur["val"])
        if a > 0:
            daily_returns.append(b / a - 1.0)
        else:
            daily_returns.append(0.0)

    mean_daily = sum(daily_returns) / float(len(daily_returns)) if daily_returns else 0.0
    std_daily = _sample_std(daily_returns)
    vol_annual = std_daily * math.sqrt(252.0)

    rf_daily = float(risk_free_annual) / 252.0
    sharpe = 0.0
    if std_daily > 1e-12:
        sharpe = ((mean_daily - rf_daily) / std_daily) * math.sqrt(252.0)

    cagr = 0.0
    if first_val > 0:
        # Prefer real calendar dates when we have at least two parseable ones,
        # otherwise fallback to assuming one point per day (common for forecast curves).
        parsed_non_null = [d for d in parsed_dates if d is not None]
        if len(parsed_non_null) >= 2:
            days = max((max(parsed_non_null) - min(parsed_non_null)).days, 0)
        else:
            days = max(len(points) - 1, 0)
        if days > 0:
            cagr = (last_val / first_val) ** (365.0 / float(days)) - 1.0

    return {
        "metrics": {
            "total_return": total_return,
            "cagr": cagr,
            "vol_annual": vol_annual,
            "sharpe": sharpe,
            "max_drawdown": max_drawdown,
        },
        "drawdown_series": drawdown_series,
    }
