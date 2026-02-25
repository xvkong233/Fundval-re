from __future__ import annotations

from typing import Any

from app.qbot_xalpha.rounding import myround
from app.qbot_xalpha.series import normalize_series, value_on_or_after


def run_buyandhold_policy(
    *,
    series: list[dict[str, Any]],
    open_dates: list[str],
    amount: float,
    buy_fee_rate: float = 0.0,
    round_label: int = 1,
) -> dict[str, Any]:
    pts = normalize_series(series)
    od = [str(d).strip() for d in open_dates if str(d).strip()]
    od.sort()
    if not od and pts:
        od = [p.date for p in pts]
    if not od:
        return {"actions": [], "summary": {"final_equity": 0.0}}

    start_date = od[0]
    nav0 = value_on_or_after(pts, start_date)
    if nav0 is None or nav0 <= 0:
        return {"actions": [], "summary": {"final_equity": 0.0}}

    amt = float(amount)
    if amt <= 0:
        return {"actions": [], "summary": {"final_equity": 0.0}}

    net_amount = myround(amt / (1.0 + float(buy_fee_rate)))
    share = myround(net_amount / float(nav0), int(round_label))
    actions = [{"date": start_date, "code": "", "type": "buy", "amount": amt, "share": share}]

    last_date = od[-1]
    last_nav = value_on_or_after(pts, last_date)
    final_equity = 0.0
    if last_nav is not None and last_nav > 0:
        final_equity = myround(float(last_nav) * share)

    return {
        "actions": actions,
        "summary": {
            "final_date": last_date,
            "final_nav": float(last_nav) if last_nav is not None else None,
            "final_equity": final_equity,
            "portion": share,
        },
    }

