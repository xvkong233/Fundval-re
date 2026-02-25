from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable


@dataclass(frozen=True)
class SeriesPoint:
    date: str
    val: float


def normalize_series(series: Iterable[dict]) -> list[SeriesPoint]:
    pts: list[SeriesPoint] = []
    for row in series:
        d = str(row.get("date", "")).strip()
        if not d:
            continue
        try:
            v = float(row.get("val", 0.0))
        except Exception:
            continue
        pts.append(SeriesPoint(date=d, val=v))
    pts.sort(key=lambda p: p.date)
    return pts


def value_on_or_after(points: list[SeriesPoint], date: str) -> float | None:
    """
    对齐 xalpha 常见用法：当 date 不存在时取下一可用点（price[date >= target].iloc[0]）。
    points 必须 date 升序。
    """
    if not points:
        return None
    date = str(date).strip()
    for p in points:
        if p.date >= date:
            return float(p.val)
    return None

