from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel, Field, field_validator


class TsCfg(BaseModel):
    total_amount: float = 10000.0
    salary: float = 10000.0
    purchased_fund_amount: float = 0.0
    fixed_amount: float = 1000.0
    period: tuple[Literal["weekly", "monthly"], int] = ("monthly", 1)

    sh_composite_index: float = 3000.0
    fund_position: float = 70.0
    sell_at_top: bool = True
    sell_num: float = 10.0
    sell_unit: Literal["amount", "fundPercent"] = "fundPercent"
    profit_rate: float = 5.0

    sell_macd_point: float | None = None
    buy_macd_point: float | None = None
    buy_amount_percent: float = 20.0

    buy_fee_rate: float = 0.0015
    sell_fee_rate: float = 0.005

    @field_validator("period", mode="before")
    @classmethod
    def _period_tuple(cls, v: Any):
        if v is None:
            return ("monthly", 1)
        if isinstance(v, (list, tuple)) and len(v) == 2:
            return (str(v[0]), int(v[1]))
        return v


class StrategySpec(BaseModel):
    name: str
    cfg: TsCfg = Field(default_factory=TsCfg)

