from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable

from app.pytrader.backtest import PytraderSignal


StrategyFn = Callable[[list[dict[str, Any]], dict[str, Any]], list[PytraderSignal]]


@dataclass(frozen=True)
class StrategySpec:
    id: str
    title: str
    fn: StrategyFn


_REGISTRY: dict[str, StrategySpec] = {}


def register_strategy(spec: StrategySpec) -> None:
    _REGISTRY[spec.id] = spec


def get_strategy(strategy_id: str) -> StrategySpec | None:
    return _REGISTRY.get(strategy_id)


def list_strategies() -> list[dict[str, str]]:
    return [{"id": s.id, "title": s.title} for s in sorted(_REGISTRY.values(), key=lambda x: x.id)]

