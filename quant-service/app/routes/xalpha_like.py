from __future__ import annotations

from typing import Any

from fastapi import APIRouter
from pydantic import BaseModel, Field

from app.xalpha_like.grid import grid_actions
from app.xalpha_like.metrics import metrics_from_series
from app.xalpha_like.qdiipredict import predict_from_legs
from app.xalpha_like.scheduled import scheduled_actions

router = APIRouter()


class _SeriesBody(BaseModel):
    series: list[dict[str, Any]] = Field(default_factory=list)


class MetricsBody(_SeriesBody):
    risk_free_annual: float = 0.0


@router.post("/api/quant/xalpha/metrics")
def metrics(body: MetricsBody):
    out = metrics_from_series([dict(p) for p in body.series], risk_free_annual=body.risk_free_annual)
    return out


class GridBody(_SeriesBody):
    grid_step_pct: float = 0.02


@router.post("/api/quant/xalpha/grid")
def grid(body: GridBody):
    actions = grid_actions([dict(p) for p in body.series], grid_step_pct=body.grid_step_pct)
    return {"actions": actions}


class ScheduledBody(_SeriesBody):
    every_n: int = 20
    amount: float = 1.0


@router.post("/api/quant/xalpha/scheduled")
def scheduled(body: ScheduledBody):
    actions = scheduled_actions(
        [dict(p) for p in body.series],
        every_n=body.every_n,
        amount=body.amount,
    )
    return {"actions": actions}


class QdiiPredictLeg(BaseModel):
    code: str
    percent: float
    ratio: float
    currency_ratio: float = 1.0


class QdiiPredictBody(BaseModel):
    last_value: float
    legs: list[QdiiPredictLeg] = Field(default_factory=list)


@router.post("/api/quant/xalpha/qdiipredict")
def qdiipredict(body: QdiiPredictBody):
    out = predict_from_legs(
        last_value=float(body.last_value),
        legs=[leg.model_dump() for leg in body.legs],
    )
    return out
