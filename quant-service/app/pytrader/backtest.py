from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal

from app.qbot_xalpha.bte_engine import BteEngine, FeeCfg


@dataclass(frozen=True)
class PytraderSignal:
    date: str
    type: Literal["buy", "sell"]
    reason: str = ""


def run_single_asset_backtest(
    *,
    series: list[dict[str, Any]],
    signals: list[PytraderSignal],
    totmoney: float,
    fee: FeeCfg,
    code: str = "ASSET",
) -> dict[str, Any]:
    open_dates = sorted([str(p.get("date", "")).strip()[:10] for p in series if str(p.get("date", "")).strip()])
    if not open_dates:
        return {"actions": [], "equity_curve": [], "summary": {"final_equity": float(totmoney)}}

    engine = BteEngine(series_map={code: [dict(p) for p in series]}, fees={code: fee}, open_dates=open_dates, initial_cash=float(totmoney))
    sig_map: dict[str, list[PytraderSignal]] = {}
    for s in signals:
        sig_map.setdefault(str(s.date).strip()[:10], []).append(s)

    equity_curve: list[dict[str, Any]] = []
    for d in open_dates:
        for s in sig_map.get(d, []):
            if s.type == "buy":
                engine.buy(code, engine.cash, d)
            elif s.type == "sell":
                engine.sell(code, -0.005, d)

        equity_curve.append({"date": d, "equity": engine.equity(d), "cash": engine.cash, "holding": engine.holdings.get(code, 0.0)})

    final_date = open_dates[-1]
    return {
        "actions": engine.actions,
        "equity_curve": equity_curve,
        "summary": {"final_date": final_date, "final_equity": engine.equity(final_date)},
    }

