from __future__ import annotations

from typing import Any, Literal

from app.qbot_xalpha.rounding import myround
from app.qbot_xalpha.series import SeriesPoint, normalize_series, value_on_or_after


def run_scheduled_window_policy(
    *,
    series: list[dict[str, Any]],
    open_dates: list[str],
    times: list[str],
    value: float,
    window: int,
    window_dist: int,
    piece: list[tuple[float, float]],
    method: Literal["MAX", "MIN", "AVG"] = "AVG",
    buy_fee_rate: float = 0.0,
    round_label: int = 1,
) -> dict[str, Any]:
    pts = normalize_series(series)
    od = [str(d).strip() for d in open_dates if str(d).strip()]
    od.sort()
    times_norm = [str(d).strip() for d in times if str(d).strip()]
    times_norm.sort()
    times_set = set(times_norm)

    w = int(window)
    wd = int(window_dist)
    if w < 1 or wd < 1:
        return {"actions": [], "summary": {"final_equity": 0.0}}
    if method not in ("MAX", "MIN", "AVG"):
        method = "AVG"

    skip_count = w + wd - 1
    skip_set = set(times_norm[:skip_count])

    actions: list[dict[str, Any]] = []
    portion = 0.0

    def points_before(date: str) -> list[SeriesPoint]:
        return [p for p in pts if p.date < date]

    for d in od:
        if d not in times_set:
            continue
        if d in skip_set:
            continue
        pr = points_before(d)
        if len(pr) < skip_count:
            continue
        nav = value_on_or_after(pts, d)
        if nav is None or nav <= 0:
            continue
        # window values: from [-wd] .. [- (w+wd-1)]
        wvals: list[float] = []
        for i in range(wd, w + wd):
            idx = -1 * i
            if abs(idx) > len(pr):
                break
            wvals.append(float(pr[idx].val))
        if len(wvals) != w:
            continue
        if method == "MAX":
            base = max(wvals)
        elif method == "MIN":
            base = min(wvals)
        else:
            base = sum(wvals) / float(len(wvals))
        if base <= 0:
            continue
        pct = (float(nav) - float(base)) / float(base) * 100.0

        mult = 0.0
        for thr, m in piece:
            if pct <= float(thr):
                mult = float(m)
                break
        if mult <= 0:
            continue

        amount_out = float(value) * mult
        if amount_out <= 0:
            continue
        net_amount = myround(amount_out / (1.0 + float(buy_fee_rate)))
        share = myround(net_amount / float(nav), int(round_label))
        if share <= 0:
            continue
        portion = myround(portion + share)
        actions.append({"date": d, "code": "", "type": "buy", "amount": amount_out, "share": share})

    last_date = od[-1] if od else (pts[-1].date if pts else "")
    last_nav = value_on_or_after(pts, last_date) if last_date else (pts[-1].val if pts else None)
    final_equity = 0.0
    if last_nav is not None and last_nav > 0:
        final_equity = myround(float(last_nav) * portion)

    return {
        "actions": actions,
        "summary": {
            "final_date": last_date,
            "final_nav": float(last_nav) if last_nav is not None else None,
            "final_equity": final_equity,
            "portion": portion,
        },
    }

