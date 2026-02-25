from __future__ import annotations

from dataclasses import dataclass
from datetime import date
from datetime import timedelta
from typing import Any, Literal


def _parse_date(s: str) -> date:
    return date.fromisoformat(str(s).strip())


def _round2(x: float) -> float:
    return round(float(x), 2)


def _round4(x: float) -> float:
    return round(float(x), 4)


def _safe_float(v: Any, default: float = 0.0) -> float:
    try:
        x = float(v)
        if x != x:  # nan
            return default
        return x
    except Exception:
        return default


@dataclass(frozen=True)
class TsStrategyConfig:
    # Investment strategy parameters (aligned with fund-strategies SearchForm)
    total_amount: float = 10000.0
    salary: float = 10000.0
    purchased_fund_amount: float = 0.0
    fixed_amount: float = 1000.0
    period: tuple[Literal["weekly", "monthly"], int] = ("monthly", 1)

    # Stop-profit strategy parameters
    sh_composite_index: float = 3000.0
    fund_position: float = 70.0  # percent
    sell_at_top: bool = True
    sell_num: float = 10.0
    sell_unit: Literal["amount", "fundPercent"] = "fundPercent"
    profit_rate: float = 5.0  # percent

    # MACD timing parameters (percentile threshold used by txnByMacd)
    sell_macd_point: float | None = None  # 0..100, None => disabled
    buy_macd_point: float | None = None  # 0..100, None => disabled
    buy_amount_percent: float = 20.0  # <=100: % of leftAmount, >100: absolute amount

    # Fees (same defaults as TS)
    buy_fee_rate: float = 0.0015
    sell_fee_rate: float = 0.005


def run_ts_strategy(
    *,
    fund_series: list[dict[str, Any]],
    shangzheng_series: list[dict[str, Any]],
    refer_index_points: list[dict[str, Any]],
    cfg: TsStrategyConfig,
    include_series: bool = False,
) -> dict[str, Any]:
    """
    Best-effort port of Qbot fund-strategies InvestmentStrategy + pages/index.tsx onEachDay logic.
    - fund_series: [{date:'YYYY-MM-DD', val: <unit_nav>}...] ascending
    - shangzheng_series: [{date, val}...] ascending
    - refer_index_points: output points of /api/quant/macd (can include txnType)
    """
    if not fund_series:
        return {"actions": [], "summary": {"days": 0}}

    # Sort by date asc
    fund_series = sorted(fund_series, key=lambda p: str(p.get("date", "")))
    shangzheng_series = sorted(shangzheng_series, key=lambda p: str(p.get("date", "")))
    refer_index_points = sorted(refer_index_points, key=lambda p: str(p.get("date", "")))

    start_date = str(fund_series[0].get("date", "")).strip()
    end_date = str(fund_series[-1].get("date", "")).strip()
    if not start_date or not end_date:
        return {"actions": [], "summary": {"days": 0}}

    # Pointers for "getFundByDate fallback to previous valid date"
    fund_i = 0
    cur_nav_val: float | None = None
    sz_i = 0
    cur_sz_val: float | None = None
    ri_i = 0
    cur_ri_point: dict[str, Any] | None = None

    def get_fund_nav(cur_date: str) -> float:
        nonlocal fund_i, cur_nav_val
        while fund_i < len(fund_series) and str(fund_series[fund_i].get("date", "")) <= cur_date:
            cur_nav_val = _safe_float(fund_series[fund_i].get("val"), 0.0)
            fund_i += 1
        return float(cur_nav_val or 0.0)

    def get_sz_val(cur_date: str) -> float:
        nonlocal sz_i, cur_sz_val
        while sz_i < len(shangzheng_series) and str(shangzheng_series[sz_i].get("date", "")) <= cur_date:
            cur_sz_val = _safe_float(shangzheng_series[sz_i].get("val"), 0.0)
            sz_i += 1
        return float(cur_sz_val or 0.0)

    def get_refer_point(cur_date: str) -> dict[str, Any]:
        nonlocal ri_i, cur_ri_point
        while ri_i < len(refer_index_points) and str(refer_index_points[ri_i].get("date", "")) <= cur_date:
            cur_ri_point = dict(refer_index_points[ri_i])
            ri_i += 1
        p = dict(cur_ri_point or {"date": cur_date})
        # txnType 是“事件”而不是“状态”：如果这里是 fallback 到前一个交易日，
        # 则不要沿用前一天的 txnType，否则会导致重复触发买卖。
        if str(p.get("date", "")) != cur_date:
            p.pop("txnType", None)
            p.pop("txn_type", None)
        return p

    def should_fixed_invest(cur: date) -> bool:
        period, date_or_week = cfg.period
        if period == "monthly":
            return cur.day == int(date_or_week)
        # weekly: JS getDay() Monday=1..Friday=5; python weekday Monday=0
        return cur.weekday() == int(date_or_week) - 1

    # Portfolio state (mirrors InvestDateSnapshot)
    portion = 0.0
    cost = 0.0
    total_buy_amount = 0.0
    total_sell_amount = 0.0
    left_amount = float(cfg.total_amount)
    max_principal = 0.0
    max_acc_profit_amount = 0.0
    max_acc_profit_date = start_date

    actions: list[dict[str, Any]] = []
    series_out: list[dict[str, Any]] = []

    def fund_amount(cur_nav: float) -> float:
        return _round2(cur_nav * portion)

    def total_amount(cur_nav: float) -> float:
        return _round2(left_amount + fund_amount(cur_nav))

    def cost_amount() -> float:
        return _round2(cost * portion)

    def profit_rate(cur_nav: float) -> float:
        if cost_amount() == 0:
            return 0.0
        if cost <= 0:
            return 0.0
        return _round4(cur_nav / cost - 1.0)

    def accumulated_profit(cur_nav: float) -> float:
        return _round2(fund_amount(cur_nav) - total_buy_amount + total_sell_amount)

    def buy(amount_out: float, cur_nav: float) -> None:
        nonlocal portion, cost, total_buy_amount, left_amount, max_principal
        amount_out = float(amount_out)
        if amount_out <= 0:
            return
        if left_amount < amount_out:
            return

        total_buy_amount = _round2(total_buy_amount + amount_out)
        # fulfillBuyTxn: net amount excludes buy fee rate
        net_amount = _round2(amount_out / (1.0 + float(cfg.buy_fee_rate)))
        buy_portion = _round2(net_amount / cur_nav) if cur_nav > 0 else 0.0
        if buy_portion <= 0:
            return

        prev_cost_amount = cost_amount()
        portion = _round2(portion + buy_portion)
        cost = _round4((prev_cost_amount + amount_out) / portion) if portion > 0 else 0.0
        left_amount = _round2(left_amount - amount_out)

        if max_principal < cost_amount():
            max_principal = cost_amount()

    def sell(amount_target: float, cur_nav: float) -> None:
        nonlocal portion, total_sell_amount, left_amount
        amount_target = float(amount_target)
        if amount_target <= 0:
            return
        if cur_nav <= 0:
            return
        if portion <= 0:
            return

        # fulfillSellTxn: amount_target -> portion rounded to 2
        sell_portion = _round2(amount_target / cur_nav)
        if sell_portion <= 0:
            return
        if portion - sell_portion < -1e-9:
            return

        amount_in = _round2(cur_nav * sell_portion * (1.0 - float(cfg.sell_fee_rate)))
        portion = _round2(portion - sell_portion)
        left_amount = _round2(left_amount + amount_in)
        total_sell_amount = _round2(total_sell_amount + amount_in)

    cur_dt = _parse_date(start_date)
    end_dt = _parse_date(end_date)
    day_idx = 0

    while cur_dt <= end_dt:
        d = cur_dt.isoformat()
        cur_nav = get_fund_nav(d)
        if cur_nav <= 0:
            cur_dt += timedelta(days=1)
            day_idx += 1
            continue

        # income() - salary on day 1
        if cur_dt.day == 1 and cfg.salary:
            left_amount = _round2(left_amount + float(cfg.salary))

        day_buy_amount_out = 0.0
        day_sell_amount_in = 0.0

        def try_buy(amount_out: float, *, reason: str) -> None:
            nonlocal day_buy_amount_out
            prev_portion = float(portion)
            prev_left = float(left_amount)
            buy(float(amount_out), cur_nav)
            if portion > prev_portion + 1e-9 and left_amount < prev_left - 1e-9:
                day_buy_amount_out = _round2(day_buy_amount_out + float(amount_out))
                actions.append({"date": d, "type": "buy", "amount": float(amount_out), "reason": reason})

        def try_sell(amount_target: float, *, reason: str) -> None:
            nonlocal day_sell_amount_in
            prev_portion = float(portion)
            prev_left = float(left_amount)
            sell(float(amount_target), cur_nav)
            if portion < prev_portion - 1e-9 and left_amount > prev_left + 1e-9:
                day_sell_amount_in = _round2(day_sell_amount_in + (left_amount - prev_left))
                actions.append({"date": d, "type": "sell", "amount": float(amount_target), "reason": reason})

        # initial purchased fund amount on the first day
        if day_idx == 0 and cfg.purchased_fund_amount:
            try_buy(float(cfg.purchased_fund_amount), reason="initial")

        # fixed invest before daily strategy
        if cfg.fixed_amount and should_fixed_invest(cur_dt):
            try_buy(float(cfg.fixed_amount), reason="fixed_invest")

        # maxAccumulatedProfit computed before onEachDay actions (matches TS usage)
        cur_acc_profit = accumulated_profit(cur_nav)
        if day_idx == 0:
            max_acc_profit_amount = cur_acc_profit
            max_acc_profit_date = d
        else:
            if cur_acc_profit >= max_acc_profit_amount:
                max_acc_profit_amount = cur_acc_profit
                max_acc_profit_date = d

        # onEachDay logic (pages/index.tsx)
        level = 0.0
        ta = total_amount(cur_nav)
        if ta > 0:
            level = _round2(fund_amount(cur_nav) / ta)

        cur_sz = get_sz_val(d)
        cur_refer = get_refer_point(d)
        txn_type = str(cur_refer.get("txnType") or cur_refer.get("txn_type") or "").strip().lower()

        # TS 语义：sellMacdPoint/buyMacdPoint 为 0 时视为“关闭”
        sell_macd_enabled = cfg.sell_macd_point is not None and float(cfg.sell_macd_point) > 0.0
        buy_macd_enabled = cfg.buy_macd_point is not None and float(cfg.buy_macd_point) > 0.0
        sell_allowed = (not sell_macd_enabled) or txn_type == "sell"
        buy_allowed = buy_macd_enabled and txn_type == "buy"

        # TS 语义：profitRate 阈值为 (profitRate/100 || -100)
        profit_threshold = (float(cfg.profit_rate) / 100.0) if float(cfg.profit_rate) != 0.0 else -1e9

        if (
            level > float(cfg.fund_position) / 100.0
            and cur_sz > float(cfg.sh_composite_index)
            and (not cfg.sell_at_top or max_acc_profit_date == d)
            and sell_allowed
            and profit_rate(cur_nav) > profit_threshold
        ):
            if cfg.sell_unit == "amount":
                amt = float(cfg.sell_num)
            else:
                amt = _round2(float(cfg.sell_num) / 100.0 * fund_amount(cur_nav))

            if amt > 0:
                try_sell(amt, reason="stop_profit")

        if buy_allowed:
            if float(cfg.buy_amount_percent) <= 100.0:
                amt = round(left_amount * float(cfg.buy_amount_percent) / 100.0)
            else:
                amt = float(cfg.buy_amount_percent)
            if amt > 0:
                try_buy(float(amt), reason="macd_buy")

        if include_series:
            ta = total_amount(cur_nav)
            fa = fund_amount(cur_nav)
            pos = 0.0
            if ta > 0:
                pos = _round4(fa / ta)
            series_out.append(
                {
                    "date": d,
                    "nav": float(cur_nav),
                    "left_amount": float(left_amount),
                    "portion": float(portion),
                    "cost": float(cost),
                    "fund_amount": float(fa),
                    "total_amount": float(ta),
                    "profit_rate": float(profit_rate(cur_nav)),
                    "accumulated_profit": float(accumulated_profit(cur_nav)),
                    "max_principal": float(max_principal),
                    "total_profit_rate": float(
                        _round4(
                            (accumulated_profit(cur_nav) / max_principal) if max_principal > 0 else 0.0
                        )
                    ),
                    "position": float(pos),
                    "date_buy_amount": float(day_buy_amount_out),
                    "date_sell_amount": float(day_sell_amount_in),
                }
            )

        cur_dt += timedelta(days=1)
        day_idx += 1

    # final snapshot summary
    last_date = end_date or max_acc_profit_date
    last_nav = get_fund_nav(last_date)
    summary = {
        "days": day_idx,
        "last_date": last_date,
        "last_nav": last_nav,
        "left_amount": left_amount,
        "portion": portion,
        "cost": cost,
        "fund_amount": fund_amount(last_nav) if last_nav > 0 else 0.0,
        "total_amount": total_amount(last_nav) if last_nav > 0 else left_amount,
        "profit_rate": profit_rate(last_nav) if last_nav > 0 else 0.0,
        "accumulated_profit": accumulated_profit(last_nav) if last_nav > 0 else 0.0,
        "max_principal": max_principal,
        "total_profit_rate": _round4(
            (accumulated_profit(last_nav) / max_principal) if max_principal > 0 and last_nav > 0 else 0.0
        ),
        "max_accumulated_profit": {"date": max_acc_profit_date, "amount": max_acc_profit_amount},
    }

    return {
        "actions": actions,
        "summary": summary,
        **({"series": series_out} if include_series else {}),
        "params": {
            "refer_index_macd": {
                "sell_macd_point": cfg.sell_macd_point,
                "buy_macd_point": cfg.buy_macd_point,
            }
        },
    }
