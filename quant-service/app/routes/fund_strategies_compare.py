from __future__ import annotations

from typing import Any

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, Field

from app.indicators.macd import calc_macd, txn_by_macd
from app.routes.fund_strategies_schemas import StrategySpec
from app.strategies.ts_invest import TsStrategyConfig, run_ts_strategy

router = APIRouter()


class CompareBody(BaseModel):
    fund_series: list[dict[str, Any]] = Field(default_factory=list)
    shangzheng_series: list[dict[str, Any]] = Field(default_factory=list)
    refer_index_points: list[dict[str, Any]] = Field(default_factory=list)
    refer_index_series: list[dict[str, Any]] = Field(default_factory=list)
    strategies: list[StrategySpec] = Field(default_factory=list)


@router.post("/api/quant/fund-strategies/compare")
def fund_strategies_compare(body: CompareBody):
    if not body.strategies:
        raise HTTPException(status_code=400, detail="missing strategies")

    out: dict[str, Any] = {}
    for spec in body.strategies:
        name = str(spec.name or "").strip()
        if not name:
            continue
        cfg = TsStrategyConfig(**spec.cfg.model_dump())

        refer_points = [dict(p) for p in body.refer_index_points]
        if not refer_points and body.refer_index_series:
            sell_pos = float(cfg.sell_macd_point or 0.0) / 100.0
            buy_pos = float(cfg.buy_macd_point or 0.0) / 100.0
            sell_pos = max(0.0, min(1.0, sell_pos))
            buy_pos = max(0.0, min(1.0, buy_pos))
            points = calc_macd([dict(p) for p in body.refer_index_series])
            refer_points = txn_by_macd(points, sell_position=sell_pos, buy_position=buy_pos)

        out[name] = run_ts_strategy(
            fund_series=[dict(p) for p in body.fund_series],
            shangzheng_series=[dict(p) for p in body.shangzheng_series],
            refer_index_points=refer_points,
            cfg=cfg,
            include_series=True,
        )

    if not out:
        raise HTTPException(status_code=400, detail="no valid strategy names")

    return {"strategies": out}
