from __future__ import annotations

from typing import Any

from app.qbot_xalpha.rounding import myround
from app.qbot_xalpha.series import normalize_series, value_on_or_after


def run_scheduled_tune_policy(
    *,
    series: list[dict[str, Any]],
    open_dates: list[str],
    times: list[str],
    value: float,
    piece: list[tuple[float, float]],
    buy_fee_rate: float = 0.0,
    round_label: int = 1,
) -> dict[str, Any]:
    """
    xalpha.policy.scheduled_tune 的最小实现（不含抓取层）：
    - 在 times 中的日期，根据当日净值落在哪个 piece 阈值内，决定买入 value 的倍数
    - piece: [(threshold_netvalue, multiple), ...]（按从低到高/从小到大均可，按输入顺序匹配）
    """
    pts = normalize_series(series)
    times_set = {str(d).strip() for d in times if str(d).strip()}
    od = [str(d).strip() for d in open_dates if str(d).strip()]
    od.sort()

    actions: list[dict[str, Any]] = []
    portion = 0.0

    for d in od:
        if d not in times_set:
            continue
        nav = value_on_or_after(pts, d)
        if nav is None or nav <= 0:
            continue
        mult = 0.0
        for thr, m in piece:
            if float(nav) <= float(thr):
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

