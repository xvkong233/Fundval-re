from __future__ import annotations

from dataclasses import dataclass
from datetime import date as _date
from typing import Any

import pandas as pd

from app.qbot_xalpha.rounding import myround


def _to_date(x: Any) -> _date:
    if isinstance(x, _date):
        return x
    s = str(x).strip()[:10]
    return _date.fromisoformat(s)


def _to_datestr(x: Any) -> str:
    return _to_date(x).isoformat()


def _bottleneck(cftable: pd.DataFrame) -> float:
    if cftable is None or len(cftable) == 0:
        return 0.0
    # cash: buy negative, sell positive
    inputl = [-float(cftable.iloc[: i + 1]["cash"].sum()) for i in range(len(cftable))]
    return myround(max(inputl) if inputl else 0.0)


@dataclass(frozen=True)
class PricePoint:
    date: str
    netvalue: float


class Trade:
    """
    极简版 xalpha.trade.trade（显式序列输入版本）：
    - price_series: [{date, netvalue}] 升序或乱序均可
    - status: DataFrame(date, <code>)；正数=买入金额；负数=卖出语义（-0.005 全卖 / abs(x)<0.005 按比例卖）
    """

    def __init__(
        self,
        *,
        code: str,
        name: str,
        price_series: list[dict[str, Any]],
        status: pd.DataFrame,
        buy_fee_rate: float = 0.0,
        sell_fee_rate: float = 0.0,
        round_label: int = 2,
    ) -> None:
        self.code = str(code).strip()
        self.name = str(name).strip() or self.code
        self.buy_fee_rate = float(buy_fee_rate)
        self.sell_fee_rate = float(sell_fee_rate)
        self.round_label = int(round_label)

        pts: list[PricePoint] = []
        for row in price_series:
            d = str(row.get("date", "")).strip()[:10]
            if not d:
                continue
            nv = float(row.get("netvalue", row.get("val", 0.0)))
            if nv <= 0:
                continue
            pts.append(PricePoint(date=d, netvalue=nv))
        pts.sort(key=lambda p: p.date)
        self.price = pts
        self._price_map = {p.date: p.netvalue for p in pts}

        if "date" not in status.columns:
            raise ValueError("status missing date column")
        if self.code not in status.columns:
            raise ValueError(f"status missing code column: {self.code}")
        self.status = status.copy()
        self.status["date"] = self.status["date"].map(_to_datestr)
        self.status = self.status.sort_values("date").reset_index(drop=True)

        self.cftable = pd.DataFrame(columns=["date", "cash"])
        self._simulate()  # build holdings/cftable

    def _nav_on_or_before(self, d: str) -> float | None:
        # xalpha fundinfo uses ">= date" due to T+1 etc; here use <= date for simplicity.
        # We choose <= to match fund_series acting as close-of-day.
        if d in self._price_map:
            return float(self._price_map[d])
        # fallback to previous available
        prev = [p for p in self.price if p.date <= d]
        if not prev:
            return None
        return float(prev[-1].netvalue)

    def _simulate(self) -> None:
        share = 0.0
        cost = 0.0  # avg cost per share (includes buy fee in cash out)
        total_buy = 0.0
        total_sell = 0.0
        cfs: list[tuple[str, float]] = []

        for _, r in self.status.iterrows():
            d = str(r["date"]).strip()[:10]
            action = float(r[self.code])
            nav = self._nav_on_or_before(d)
            if nav is None or nav <= 0:
                continue

            if action > 0:
                amount_out = float(action)
                total_buy = myround(total_buy + amount_out)
                net_amount = myround(amount_out / (1.0 + self.buy_fee_rate))
                buy_share = myround(net_amount / nav, self.round_label)
                if buy_share <= 0:
                    continue
                prev_cost_amount = myround(cost * share)
                share = myround(share + buy_share, self.round_label)
                cost = myround((prev_cost_amount + amount_out) / share, 4) if share > 0 else 0.0
                cfs.append((d, -myround(amount_out)))

            elif action < 0:
                s = float(action)
                # xalpha: -0.005 => sell all; abs(x)<0.005 => ratio; else treat as absolute share
                sell_share = 0.0
                if abs(s) <= 0.005 and s < 0:
                    ratio = abs(s) / 0.005
                    sell_share = share * ratio
                else:
                    sell_share = abs(s)
                sell_share = myround(sell_share, self.round_label)
                if sell_share <= 0 or sell_share - share > 1e-9:
                    continue
                amount_in = myround(nav * sell_share * (1.0 - self.sell_fee_rate))
                share = myround(share - sell_share, self.round_label)
                total_sell = myround(total_sell + amount_in)
                cfs.append((d, myround(amount_in)))

        self._share = float(share)
        self._cost = float(cost)
        self._total_buy = float(total_buy)
        self._total_sell = float(total_sell)
        self.cftable = pd.DataFrame([{"date": d, "cash": c} for d, c in cfs])

    def briefdailyreport(self, date: Any) -> dict[str, Any]:
        d = _to_datestr(date)
        nav = self._nav_on_or_before(d) or 0.0
        current_value = myround(float(nav) * float(self._share))
        cost_amount = myround(float(self._cost) * float(self._share))
        earn = myround(current_value - float(self._total_buy) + float(self._total_sell))
        btnk = _bottleneck(self.cftable[self.cftable["date"] <= d]) if len(self.cftable) else 0.0
        rate = myround((earn / btnk * 100.0) if btnk > 0 else 0.0, 4)
        return {
            "date": d,
            "netvalue": float(nav),
            "currentshare": float(self._share),
            "currentvalue": float(current_value),
            "unitcost": float(self._cost),
            "costamount": float(cost_amount),
            "purchase": float(self._total_buy),
            "output": float(self._total_sell),
            "earn": float(earn),
            "bottleneck": float(btnk),
            "rate": float(rate),
        }

    def dailyreport(self, date: Any) -> pd.DataFrame:
        d = _to_datestr(date)
        r = self.briefdailyreport(d)
        # 对齐 mul.combsummary 常用列子集（不含换手率等高级字段）
        cols = [
            "基金名称",
            "基金代码",
            "当日净值",
            "单位成本",
            "持有份额",
            "基金现值",
            "基金总申购",
            "历史最大占用",
            "基金持有成本",
            "基金分红与赎回",
            "基金收益总额",
            "投资收益率",
        ]
        row = {
            "基金名称": self.name,
            "基金代码": self.code,
            "当日净值": r["netvalue"],
            "单位成本": r["unitcost"],
            "持有份额": r["currentshare"],
            "基金现值": r["currentvalue"],
            "基金总申购": r["purchase"],
            "历史最大占用": r["bottleneck"],
            "基金持有成本": r["costamount"],
            "基金分红与赎回": r["output"],
            "基金收益总额": r["earn"],
            "投资收益率": r["rate"],
        }
        return pd.DataFrame([row], columns=cols)

