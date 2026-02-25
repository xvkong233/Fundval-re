from __future__ import annotations

from typing import Any, Literal


def grid_actions(series: list[dict[str, Any]], grid_step_pct: float) -> list[dict[str, Any]]:
    if not series:
        return []
    step = float(grid_step_pct)
    if step <= 0:
        return []

    points: list[dict[str, Any]] = []
    for i, row in enumerate(series):
        points.append(
            {
                "index": int(row.get("index", i)),
                "date": str(row.get("date", "")).strip(),
                "val": float(row.get("val", 0.0)),
            }
        )

    anchor = float(points[0]["val"])
    actions: list[dict[str, Any]] = []
    for p in points[1:]:
        v = float(p["val"])
        if anchor <= 0:
            anchor = v
            continue

        down_th = anchor * (1.0 - step)
        up_th = anchor * (1.0 + step)
        action: Literal["buy", "sell"] | None = None
        if v <= down_th:
            action = "buy"
        elif v >= up_th:
            action = "sell"

        if action:
            actions.append({"index": int(p["index"]), "date": p["date"], "val": v, "action": action})
            anchor = v

    return actions

