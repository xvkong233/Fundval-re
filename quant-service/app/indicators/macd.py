from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal, TypedDict


class TxnPoint(TypedDict, total=False):
    index: int
    date: str
    val: float
    ema12: float
    ema26: float
    diff: float
    dea: float
    macd: float
    macd_position: float
    txn_type: Literal["buy", "sell"]


@dataclass
class _ThresholdItem:
    max_val: float
    threshold: int


def calc_macd(series: list[dict[str, Any]]) -> list[dict[str, Any]]:
    if not series:
        return []

    points: list[dict[str, Any]] = []

    prev_ema12: float | None = None
    prev_ema26: float | None = None
    prev_dea: float | None = None

    for i, row in enumerate(series):
        date = str(row.get("date", "")).strip()
        close = float(row.get("val", 0.0))

        if i == 0:
            ema12 = close
            ema26 = close
            diff = 0.0
            dea = 0.0
            macd = 0.0
        else:
            assert prev_ema12 is not None and prev_ema26 is not None and prev_dea is not None
            ema12 = (2.0 * close + 11.0 * prev_ema12) / 13.0
            ema26 = (2.0 * close + 25.0 * prev_ema26) / 27.0
            diff = ema12 - ema26
            dea = (2.0 * diff + 8.0 * prev_dea) / 10.0
            macd = 2.0 * (diff - dea)

        prev_ema12 = ema12
        prev_ema26 = ema26
        prev_dea = dea

        points.append(
            {
                "index": i,
                "date": date,
                "val": close,
                "ema12": ema12,
                "ema26": ema26,
                "diff": diff,
                "dea": dea,
                "macd": macd,
                "macd_position": 0.0,
                "macdPosition": 0.0,  # TS compat
            }
        )

    if len(points) == 1:
        return points

    groups: list[list[dict[str, Any]]] = []
    for i in range(1, len(points)):
        prev = points[i - 1]
        cur = points[i]
        prev_macd = float(prev["macd"])
        cur_macd = float(cur["macd"])
        if abs(prev_macd) < 1e-12:
            groups.append([cur])
            continue

        if prev_macd * cur_macd < 0:
            groups.append([cur])
        else:
            if not groups:
                groups.append([cur])
            else:
                groups[-1].append(cur)

    for g in groups:
        if not g:
            continue
        max_abs = max(abs(float(p["macd"])) for p in g)
        if max_abs < 1e-12:
            for p in g:
                p["macd_position"] = 0.0
                p["macdPosition"] = 0.0
        else:
            for p in g:
                # TS 实现 roundToFix(position, 2)
                pos = abs(float(p["macd"])) / max_abs
                pos = round(pos, 2)
                p["macd_position"] = pos
                p["macdPosition"] = pos

    return points


def txn_by_macd(
    points: list[dict[str, Any]],
    sell_position: float,
    buy_position: float,
) -> list[dict[str, Any]]:
    if not points:
        return []

    for i, p in enumerate(points):
        if "index" not in p:
            p["index"] = i

    groups: list[list[dict[str, Any]]] = [[points[0]]]
    for cur in points[1:]:
        prev = groups[-1][-1]
        is_same_side = float(prev.get("macd", 0.0)) * float(cur.get("macd", 0.0))
        if is_same_side < 0:
            groups.append([cur])
        else:
            groups[-1].append(cur)

    for group in groups:
        if not group:
            continue
        if max(abs(float(p.get("macd", 0.0))) for p in group) < 1e-12:
            continue
        is_positive = float(group[0].get("macd", 0.0)) > 0
        if (is_positive and not sell_position) or ((not is_positive) and not buy_position):
            continue

        threshold_points: list[_ThresholdItem] = [_ThresholdItem(max_val=0.0, threshold=-1)]
        for cur in group:
            latest = threshold_points[-1]
            cur_abs = abs(float(cur.get("macd", 0.0)))

            if cur_abs >= latest.max_val:
                if latest.threshold != -1:
                    threshold_points.append(_ThresholdItem(max_val=cur_abs, threshold=-1))
                    latest = threshold_points[-1]
                else:
                    latest.max_val = cur_abs

            if is_positive:
                if cur_abs <= latest.max_val * float(sell_position) and latest.threshold == -1:
                    latest.threshold = int(cur["index"])
            else:
                if cur_abs <= latest.max_val * float(buy_position) and latest.threshold == -1:
                    latest.threshold = int(cur["index"])

        last = threshold_points[-1]
        if last.threshold == -1:
            last.threshold = int(group[-1]["index"]) + 1

        for item in threshold_points:
            idx = int(item.threshold)
            if idx < 0 or idx >= len(points):
                continue
            txn = "sell" if is_positive else "buy"
            points[idx]["txn_type"] = txn
            points[idx]["txnType"] = txn  # TS compat

    return points
