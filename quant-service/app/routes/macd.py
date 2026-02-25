from __future__ import annotations

from typing import Any

from fastapi import APIRouter
from pydantic import BaseModel, Field

from app.indicators.macd import calc_macd, txn_by_macd

router = APIRouter()


class MacdTxnBody(BaseModel):
    points: list[dict[str, Any]] = Field(default_factory=list)
    series: list[dict[str, Any]] = Field(default_factory=list)
    sell_position: float = 0.75
    buy_position: float = 0.5


@router.post("/api/quant/macd")
def macd_txn(body: MacdTxnBody):
    if body.series:
        points = calc_macd([dict(p) for p in body.series])
    else:
        points = [dict(p) for p in body.points]

    points = txn_by_macd(
        points,
        sell_position=body.sell_position,
        buy_position=body.buy_position,
    )
    return {"points": points}

