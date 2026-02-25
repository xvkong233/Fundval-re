from __future__ import annotations

from typing import Any

from app.indicators.macd import calc_macd, txn_by_macd
from app.pytrader.backtest import PytraderSignal


def generate_signals(series: list[dict[str, Any]], params: dict[str, Any]) -> list[PytraderSignal]:
    sell_position = float(params.get("sell_position", 0.7))
    buy_position = float(params.get("buy_position", 0.7))

    points = calc_macd([{"date": p.get("date"), "val": p.get("val", p.get("close", p.get("netvalue")))} for p in series])
    points = txn_by_macd(points, sell_position=sell_position, buy_position=buy_position)

    out: list[PytraderSignal] = []
    for p in points:
        t = str(p.get("txn_type") or p.get("txnType") or "").strip().lower()
        if t == "buy":
            out.append(PytraderSignal(date=str(p["date"])[:10], type="buy", reason="macd_buy"))
        elif t == "sell":
            out.append(PytraderSignal(date=str(p["date"])[:10], type="sell", reason="macd_sell"))
    return out

