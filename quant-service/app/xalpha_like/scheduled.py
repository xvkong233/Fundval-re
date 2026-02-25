from __future__ import annotations

from typing import Any


def scheduled_actions(series: list[dict[str, Any]], every_n: int, amount: float) -> list[dict[str, Any]]:
    if not series:
        return []

    n = int(every_n)
    if n <= 0:
        return []

    amt = float(amount)

    actions: list[dict[str, Any]] = []
    for i, row in enumerate(series):
        if i % n != 0:
            continue
        actions.append(
            {
                "index": int(row.get("index", i)),
                "date": str(row.get("date", "")).strip(),
                "val": float(row.get("val", 0.0)),
                "action": "buy",
                "amount": amt,
            }
        )
    return actions

