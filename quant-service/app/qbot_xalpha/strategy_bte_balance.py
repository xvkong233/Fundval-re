from __future__ import annotations

from typing import Any

from app.qbot_xalpha.bte_engine import BteEngine, FeeCfg


def run_balance_backtest(
    *,
    series_map: dict[str, list[dict[str, Any]]],
    fees: dict[str, FeeCfg],
    open_dates: list[str],
    start: str,
    end: str,
    totmoney: float,
    portfolio_dict: dict[str, float],
    check_dates: list[str],
) -> dict[str, Any]:
    od = sorted([str(d).strip() for d in open_dates if str(d).strip()])
    od = [d for d in od if d >= start and d <= end]
    if not od:
        return {"actions": [], "summary": {"final_equity": float(totmoney)}}

    engine = BteEngine(series_map={k: [dict(p) for p in v] for k, v in series_map.items()}, fees=fees, open_dates=od, initial_cash=float(totmoney))
    check_set = {str(d).strip() for d in check_dates if str(d).strip()}
    nill = True

    for d in od:
        if nill:
            for fund, ratio in portfolio_dict.items():
                engine.buy(fund, float(ratio) * float(totmoney), d)
            nill = False

        if d in check_set:
            total_value = engine.equity(d)
            for fund, ratio in portfolio_dict.items():
                nav = engine.nav(fund, d)
                if nav is None or nav <= 0:
                    continue
                cur_share = float(engine.holdings.get(fund, 0.0))
                cur_val = cur_share * float(nav)
                target = float(total_value) * float(ratio)
                delta = cur_val - target
                if delta > 0:
                    fee = fees.get(fund) or FeeCfg()
                    denom = (1.0 - float(fee.sell_fee_rate)) * float(nav)
                    if denom <= 0:
                        continue
                    share_to_sell = float(delta) / denom
                    engine.sell(fund, share_to_sell, d)
                elif delta < 0:
                    buy_amount = min(-float(delta), float(engine.cash))
                    if buy_amount > 0:
                        engine.buy(fund, buy_amount, d)

    final_date = od[-1]
    return {
        "actions": engine.actions,
        "summary": {
            "final_date": final_date,
            "final_equity": engine.equity(final_date),
            "cash": engine.cash,
            "holdings": engine.holdings,
        },
    }

