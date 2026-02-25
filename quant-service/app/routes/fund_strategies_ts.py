from __future__ import annotations

from typing import Any

from fastapi import APIRouter
from pydantic import BaseModel, Field

from app.routes.fund_strategies_schemas import TsCfg
from app.strategies.ts_invest import TsStrategyConfig, run_ts_strategy

router = APIRouter()


class TsBody(BaseModel):
    fund_series: list[dict[str, Any]] = Field(default_factory=list)
    shangzheng_series: list[dict[str, Any]] = Field(default_factory=list)
    refer_index_points: list[dict[str, Any]] = Field(default_factory=list)
    cfg: TsCfg = Field(default_factory=TsCfg)


@router.post("/api/quant/fund-strategies/ts")
def fund_strategies_ts(body: TsBody):
    cfg = TsStrategyConfig(**body.cfg.model_dump())
    return run_ts_strategy(
        fund_series=[dict(p) for p in body.fund_series],
        shangzheng_series=[dict(p) for p in body.shangzheng_series],
        refer_index_points=[dict(p) for p in body.refer_index_points],
        cfg=cfg,
    )
