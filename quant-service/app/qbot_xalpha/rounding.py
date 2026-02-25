from __future__ import annotations

from decimal import Decimal, ROUND_DOWN, ROUND_HALF_UP


def myround(num: float, label: int = 1) -> float:
    """
    xalpha 的 round 语义（精确到 2 位小数）：
    - label=1: ROUND_HALF_UP
    - label=2: ROUND_DOWN
    """
    if label == 2:
        return float(Decimal(str(num)).quantize(Decimal("0.01"), rounding=ROUND_DOWN))
    return float(Decimal(str(num)).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP))


def myround4(num: float) -> float:
    return float(Decimal(str(num)).quantize(Decimal("0.0001"), rounding=ROUND_HALF_UP))

