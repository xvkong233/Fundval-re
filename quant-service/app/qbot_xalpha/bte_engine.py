from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from app.qbot_xalpha.rounding import myround
from app.qbot_xalpha.series import SeriesPoint, normalize_series, value_on_or_after


@dataclass(frozen=True)
class FeeCfg:
    buy_fee_rate: float = 0.0
    sell_fee_rate: float = 0.0
    round_label: int = 1


class BteEngine:
    """
    简化版 xalpha BTE 执行器（不含抓取层）：
    - 输入：open_dates + 多标的净值序列 + 每标的费率
    - 输出：actions + equity_curve + summary
    """

    def __init__(
        self,
        *,
        series_map: dict[str, list[dict[str, Any]]],
        fees: dict[str, FeeCfg],
        open_dates: list[str],
        initial_cash: float,
    ) -> None:
        self._points: dict[str, list[SeriesPoint]] = {k: normalize_series(v) for k, v in series_map.items()}
        self._fees = dict(fees)
        self.open_dates = sorted([str(d).strip() for d in open_dates if str(d).strip()])
        self.cash = float(initial_cash)
        self.holdings: dict[str, float] = {}
        self.actions: list[dict[str, Any]] = []
        # trade cashflows only（不含闲置现金）：buy 为负，sell 为正
        self.cashflows: list[dict[str, Any]] = []

    def fee(self, code: str) -> FeeCfg:
        return self._fees.get(code) or FeeCfg()

    def nav(self, code: str, date: str) -> float | None:
        pts = self._points.get(code) or []
        return value_on_or_after(pts, date)

    def buy(self, code: str, amount_out: float, date: str) -> None:
        amt = float(amount_out)
        if amt <= 0:
            return
        if self.cash + 1e-9 < amt:
            return
        nav = self.nav(code, date)
        if nav is None or nav <= 0:
            return
        fee = self.fee(code)
        net_amount = myround(amt / (1.0 + float(fee.buy_fee_rate)))
        share = myround(net_amount / float(nav), int(fee.round_label))
        if share <= 0:
            return
        self.cash = myround(self.cash - amt)
        self.holdings[code] = myround(self.holdings.get(code, 0.0) + share)
        self.actions.append({"date": date, "code": code, "type": "buy", "amount": amt, "share": share})
        self.cashflows.append({"date": date, "code": code, "cash": -myround(amt)})

    def sell(self, code: str, share: float, date: str) -> None:
        cur = float(self.holdings.get(code, 0.0))
        if cur <= 0:
            return
        s = float(share)
        if s == 0:
            return
        # xalpha 语义：-0.005 表示全卖，绝对值 <0.005 表示按比例卖
        if s < 0 and abs(s) <= 0.005:
            ratio = abs(s) / 0.005
            s = cur * ratio
        if s <= 0:
            return
        if s - cur > 1e-9:
            return
        nav = self.nav(code, date)
        if nav is None or nav <= 0:
            return
        fee = self.fee(code)
        amount_in = myround(float(nav) * s * (1.0 - float(fee.sell_fee_rate)))
        self.cash = myround(self.cash + amount_in)
        self.holdings[code] = myround(cur - s)
        self.actions.append({"date": date, "code": code, "type": "sell", "amount": amount_in, "share": s})
        self.cashflows.append({"date": date, "code": code, "cash": myround(amount_in)})

    def equity(self, date: str) -> float:
        total = float(self.cash)
        for code, share in self.holdings.items():
            if share <= 0:
                continue
            nav = self.nav(code, date)
            if nav is None or nav <= 0:
                continue
            total += float(nav) * float(share)
        return myround(total)
