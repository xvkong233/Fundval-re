from __future__ import annotations

from dataclasses import dataclass
from datetime import date


def _parse_date(s: str) -> date:
    return date.fromisoformat(str(s).strip()[:10])


def xnpv(rate: float, cashflows: list[tuple[str, float]]) -> float:
    if not cashflows:
        return 0.0
    r = float(rate)
    items = [(_parse_date(d), float(c)) for d, c in cashflows]
    items.sort(key=lambda x: x[0])
    t0 = items[0][0]
    total = 0.0
    for t, cf in items:
        dt = (t - t0).days / 365.0
        total += cf / (1.0 + r) ** dt
    return total


def xnpv_derivative(rate: float, cashflows: list[tuple[str, float]]) -> float:
    if not cashflows:
        return 0.0
    r = float(rate)
    items = [(_parse_date(d), float(c)) for d, c in cashflows]
    items.sort(key=lambda x: x[0])
    t0 = items[0][0]
    total = 0.0
    for t, cf in items:
        dt = (t - t0).days / 365.0
        if abs(1.0 + r) < 1e-12:
            continue
        total += (-dt) * cf / (1.0 + r) ** (dt + 1.0)
    return total


def xirr(cashflows: list[tuple[str, float]], guess: float = 0.1) -> float:
    """
    轻量 xirr（Newton + 兜底扫描），用于 ScheduledSellonXIRR 等策略。
    """
    if not cashflows:
        return 0.0
    if len(cashflows) < 2:
        return 0.0

    # Newton
    r = float(guess)
    r = max(r, -0.95)
    for _ in range(64):
        f = xnpv(r, cashflows)
        if abs(f) < 1e-7:
            return r
        df = xnpv_derivative(r, cashflows)
        if abs(df) < 1e-12:
            break
        r2 = r - f / df
        if r2 <= -0.9999:
            r2 = -0.9999
        if abs(r2 - r) < 1e-10:
            return r2
        r = r2

    # fallback: simple scan to find sign change, then bisect
    lo, hi = -0.9, 10.0
    flo = xnpv(lo, cashflows)
    fhi = xnpv(hi, cashflows)
    if flo == 0.0:
        return lo
    if fhi == 0.0:
        return hi
    if flo * fhi > 0:
        return r

    for _ in range(80):
        mid = (lo + hi) / 2.0
        fmid = xnpv(mid, cashflows)
        if abs(fmid) < 1e-7:
            return mid
        if flo * fmid <= 0:
            hi = mid
            fhi = fmid
        else:
            lo = mid
            flo = fmid
    return (lo + hi) / 2.0

