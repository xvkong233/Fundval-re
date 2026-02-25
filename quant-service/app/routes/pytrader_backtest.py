from __future__ import annotations

from typing import Any

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, Field

from app.pytrader.backtest import run_single_asset_backtest
from app.pytrader.registry import StrategySpec, get_strategy, list_strategies, register_strategy
from app.pytrader.strategies.macd_cross import generate_signals as macd_cross_signals
from app.qbot_xalpha.bte_engine import FeeCfg

router = APIRouter()


# registry bootstrap (minimal subset for now)
register_strategy(StrategySpec(id="macd_cross", title="MACD 交叉（txnByMacd）", fn=macd_cross_signals))


class FeeBody(BaseModel):
    buy_fee_rate: float = 0.0
    sell_fee_rate: float = 0.0
    round_label: int = 2


class BacktestBody(BaseModel):
    strategy: str
    totmoney: float = 0.0
    series: list[dict[str, Any]] = Field(default_factory=list)
    params: dict[str, Any] = Field(default_factory=dict)
    fees: FeeBody = Field(default_factory=FeeBody)


@router.get("/api/quant/pytrader/strategies")
def pytrader_strategies():
    return {"strategies": list_strategies()}


@router.post("/api/quant/pytrader/backtest")
def pytrader_backtest(body: BacktestBody):
    sid = str(body.strategy or "").strip()
    if not sid:
        raise HTTPException(status_code=400, detail="missing strategy")
    spec = get_strategy(sid)
    if spec is None:
        raise HTTPException(status_code=400, detail=f"unsupported strategy: {sid}")
    if not body.series:
        raise HTTPException(status_code=400, detail="missing series")

    signals = spec.fn([dict(p) for p in body.series], dict(body.params))
    fee = FeeCfg(buy_fee_rate=body.fees.buy_fee_rate, sell_fee_rate=body.fees.sell_fee_rate, round_label=body.fees.round_label)
    out = run_single_asset_backtest(series=[dict(p) for p in body.series], signals=signals, totmoney=float(body.totmoney), fee=fee, code="ASSET")
    return {"strategy": sid, "signals": [s.__dict__ for s in signals], **out}

