from __future__ import annotations

from typing import Any

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, Field

from app.qbot_xalpha.bte_engine import BteEngine, FeeCfg
from app.qbot_xalpha.policy_buyandhold import run_buyandhold_policy
from app.qbot_xalpha.policy_scheduled import run_scheduled_policy
from app.qbot_xalpha.policy_scheduled_tune import run_scheduled_tune_policy
from app.qbot_xalpha.policy_scheduled_window import run_scheduled_window_policy
from app.qbot_xalpha.strategy_grid import run_grid_backtest
from app.qbot_xalpha.strategy_indicator_cross import run_indicator_cross_backtest
from app.qbot_xalpha.strategy_indicator_points import run_indicator_points_backtest
from app.qbot_xalpha.strategy_bte_average_scheduled import run_average_scheduled_backtest
from app.qbot_xalpha.strategy_bte_scheduled_sell_on_xirr import run_scheduled_sell_on_xirr_backtest
from app.qbot_xalpha.strategy_bte_tendency28 import run_tendency28_backtest
from app.qbot_xalpha.strategy_bte_balance import run_balance_backtest

router = APIRouter()


class CalendarBody(BaseModel):
    open_dates: list[str] = Field(default_factory=list)


class FeeBody(BaseModel):
    buy_fee_rate: float = 0.0
    sell_fee_rate: float = 0.0
    round_label: int = 1


class BacktestBody(BaseModel):
    strategy: str
    start: str | None = None
    end: str | None = None
    totmoney: float | None = None

    calendar: CalendarBody | None = None
    series: dict[str, list[dict[str, Any]]] = Field(default_factory=dict)
    params: dict[str, Any] = Field(default_factory=dict)
    fees: dict[str, FeeBody] = Field(default_factory=dict)


@router.post("/api/quant/xalpha/backtest")
def xalpha_backtest(body: BacktestBody):
    strategy = (body.strategy or "").strip()
    if not strategy:
        raise HTTPException(status_code=400, detail="missing strategy")

    output = str(body.params.get("output", "actions") or "actions").strip().lower()

    def attach_report(out: dict[str, Any]) -> dict[str, Any]:
        if output != "summary":
            return {"strategy": strategy, **out}

        try:
            import pandas as pd

            from app.qbot_xalpha_compat.mul import Mul
            from app.qbot_xalpha_compat.report import summary_json
            from app.qbot_xalpha_compat.trade import Trade
        except Exception as e:
            raise HTTPException(status_code=500, detail=f"report deps missing: {e}")

        actions = out.get("actions") or []
        if not isinstance(actions, list):
            actions = []

        series_map = {str(k).strip(): [dict(p) for p in v] for k, v in (body.series or {}).items() if str(k).strip()}
        if not series_map:
            return {"strategy": strategy, **out, "report": {"summary": []}}

        report_date: str = ""
        if isinstance(out.get("summary"), dict):
            report_date = str(out["summary"].get("final_date") or "").strip()
        if not report_date:
            report_date = str(body.end or "").strip()
        if not report_date and body.calendar and body.calendar.open_dates:
            report_date = str(body.calendar.open_dates[-1]).strip()
        if not report_date:
            any_series = next(iter(series_map.values()))
            report_date = str(any_series[-1].get("date", "")).strip() if any_series else ""
        report_date = report_date[:10]

        status_rows_by_code: dict[str, list[dict[str, Any]]] = {}

        def infer_code(act: dict[str, Any]) -> str | None:
            raw = str(act.get("code") or "").strip()
            if raw:
                return raw
            if len(series_map) == 1:
                return next(iter(series_map.keys()))
            return None

        for act in actions:
            if not isinstance(act, dict):
                continue
            d = str(act.get("date") or "").strip()[:10]
            t = str(act.get("type") or "").strip().lower()
            code = infer_code(act)
            if not d or not code or code not in series_map:
                continue
            status_rows_by_code.setdefault(code, [])
            if t == "buy":
                status_rows_by_code[code].append({"date": d, code: float(act.get("amount") or 0.0)})
            elif t == "sell":
                status_rows_by_code[code].append({"date": d, code: -float(act.get("share") or 0.0)})

        trades: list[Trade] = []
        for code, series in series_map.items():
            fee = body.fees.get(code) or FeeBody()
            rows = status_rows_by_code.get(code) or []
            if rows:
                status = pd.DataFrame(rows)
            else:
                status = pd.DataFrame({"date": [], code: []})
            trades.append(
                Trade(
                    code=code,
                    name=code,
                    price_series=series,
                    status=status,
                    buy_fee_rate=float(fee.buy_fee_rate),
                    sell_fee_rate=float(fee.sell_fee_rate),
                    round_label=int(fee.round_label),
                )
            )

        m = Mul(*trades)
        return {"strategy": strategy, **out, "report": {"summary": summary_json(m, report_date)}}

    # 目前先按 TDD 最小实现 scheduled（policy.py）
    if strategy == "buyandhold":
        code = str(body.params.get("code", "")).strip()
        if not code:
            raise HTTPException(status_code=400, detail="buyandhold: missing params.code")
        series = body.series.get(code) or []
        if not series:
            raise HTTPException(status_code=400, detail="buyandhold: missing series[code]")
        if body.calendar and body.calendar.open_dates:
            open_dates = list(body.calendar.open_dates)
        else:
            open_dates = [str(p.get("date", "")).strip() for p in series if str(p.get("date", "")).strip()]
        fee = body.fees.get(code) or FeeBody()
        out = run_buyandhold_policy(
            series=[dict(p) for p in series],
            open_dates=open_dates,
            amount=float(body.totmoney or 0.0),
            buy_fee_rate=float(fee.buy_fee_rate),
            round_label=int(fee.round_label),
        )
        return attach_report(out)

    if strategy == "scheduled":
        code = str(body.params.get("code", "")).strip()
        if not code:
            raise HTTPException(status_code=400, detail="scheduled: missing params.code")
        series = body.series.get(code) or []
        if not series:
            raise HTTPException(status_code=400, detail="scheduled: missing series[code]")
        times = body.params.get("times") or []
        if not isinstance(times, list):
            raise HTTPException(status_code=400, detail="scheduled: params.times must be list")
        value = float(body.params.get("value", 0.0))
        if body.calendar and body.calendar.open_dates:
            open_dates = list(body.calendar.open_dates)
        else:
            # 没给就使用 series 里的日期集合（已排序）
            open_dates = [str(p.get("date", "")).strip() for p in series if str(p.get("date", "")).strip()]

        fee = body.fees.get(code) or FeeBody()
        out = run_scheduled_policy(
            series=[dict(p) for p in series],
            open_dates=open_dates,
            times=[str(x) for x in times],
            value=value,
            buy_fee_rate=float(fee.buy_fee_rate),
            round_label=int(fee.round_label),
        )
        return attach_report(out)

    if strategy == "scheduled_tune":
        code = str(body.params.get("code", "")).strip()
        if not code:
            raise HTTPException(status_code=400, detail="scheduled_tune: missing params.code")
        series = body.series.get(code) or []
        if not series:
            raise HTTPException(status_code=400, detail="scheduled_tune: missing series[code]")
        times = body.params.get("times") or []
        if not isinstance(times, list):
            raise HTTPException(status_code=400, detail="scheduled_tune: params.times must be list")
        piece_raw = body.params.get("piece") or []
        if not isinstance(piece_raw, list):
            raise HTTPException(status_code=400, detail="scheduled_tune: params.piece must be list")
        piece: list[tuple[float, float]] = []
        for it in piece_raw:
            if not isinstance(it, (list, tuple)) or len(it) != 2:
                continue
            try:
                piece.append((float(it[0]), float(it[1])))
            except Exception:
                continue

        value = float(body.params.get("value", 0.0))
        if body.calendar and body.calendar.open_dates:
            open_dates = list(body.calendar.open_dates)
        else:
            open_dates = [str(p.get("date", "")).strip() for p in series if str(p.get("date", "")).strip()]

        fee = body.fees.get(code) or FeeBody()
        out = run_scheduled_tune_policy(
            series=[dict(p) for p in series],
            open_dates=open_dates,
            times=[str(x) for x in times],
            value=value,
            piece=piece,
            buy_fee_rate=float(fee.buy_fee_rate),
            round_label=int(fee.round_label),
        )
        return attach_report(out)

    if strategy == "scheduled_window":
        code = str(body.params.get("code", "")).strip()
        if not code:
            raise HTTPException(status_code=400, detail="scheduled_window: missing params.code")
        series = body.series.get(code) or []
        if not series:
            raise HTTPException(status_code=400, detail="scheduled_window: missing series[code]")
        times = body.params.get("times") or []
        if not isinstance(times, list):
            raise HTTPException(status_code=400, detail="scheduled_window: params.times must be list")
        piece_raw = body.params.get("piece") or []
        if not isinstance(piece_raw, list):
            raise HTTPException(status_code=400, detail="scheduled_window: params.piece must be list")
        piece: list[tuple[float, float]] = []
        for it in piece_raw:
            if not isinstance(it, (list, tuple)) or len(it) != 2:
                continue
            try:
                piece.append((float(it[0]), float(it[1])))
            except Exception:
                continue
        value = float(body.params.get("value", 0.0))
        window = int(body.params.get("window", 1))
        window_dist = int(body.params.get("window_dist", 1))
        method = str(body.params.get("method", "AVG") or "AVG").strip().upper()
        if body.calendar and body.calendar.open_dates:
            open_dates = list(body.calendar.open_dates)
        else:
            open_dates = [str(p.get("date", "")).strip() for p in series if str(p.get("date", "")).strip()]
        fee = body.fees.get(code) or FeeBody()
        out = run_scheduled_window_policy(
            series=[dict(p) for p in series],
            open_dates=open_dates,
            times=[str(x) for x in times],
            value=value,
            window=window,
            window_dist=window_dist,
            method=method if method in ("MAX", "MIN", "AVG") else "AVG",
            piece=piece,
            buy_fee_rate=float(fee.buy_fee_rate),
            round_label=int(fee.round_label),
        )
        return attach_report(out)

    if strategy == "grid":
        code = str(body.params.get("code", "")).strip()
        if not code:
            raise HTTPException(status_code=400, detail="grid: missing params.code")
        series = body.series.get(code) or []
        if not series:
            raise HTTPException(status_code=400, detail="grid: missing series[code]")
        buypercent = body.params.get("buypercent") or []
        sellpercent = body.params.get("sellpercent") or []
        if not isinstance(buypercent, list) or not isinstance(sellpercent, list):
            raise HTTPException(status_code=400, detail="grid: buypercent/sellpercent must be list")
        buypercent_f = [float(x) for x in buypercent]
        sellpercent_f = [float(x) for x in sellpercent]
        start = str(body.start or "").strip() or (body.calendar.open_dates[0] if body.calendar and body.calendar.open_dates else "")
        end = str(body.end or "").strip() or (body.calendar.open_dates[-1] if body.calendar and body.calendar.open_dates else "")
        if not start or not end:
            raise HTTPException(status_code=400, detail="grid: missing start/end")
        open_dates = list(body.calendar.open_dates) if body.calendar and body.calendar.open_dates else [str(p.get("date", "")).strip() for p in series]
        fee_body = body.fees.get(code) or FeeBody()
        out = run_grid_backtest(
            code=code,
            series=[dict(p) for p in series],
            open_dates=open_dates,
            start=start,
            end=end,
            totmoney=float(body.totmoney or 0.0),
            buypercent=buypercent_f,
            sellpercent=sellpercent_f,
            fee=FeeCfg(buy_fee_rate=fee_body.buy_fee_rate, sell_fee_rate=fee_body.sell_fee_rate, round_label=fee_body.round_label),
        )
        return attach_report(out)

    if strategy == "indicator_cross":
        code = str(body.params.get("code", "")).strip()
        if not code:
            raise HTTPException(status_code=400, detail="indicator_cross: missing params.code")
        series = body.series.get(code) or []
        if not series:
            raise HTTPException(status_code=400, detail="indicator_cross: missing series[code]")
        col_raw = body.params.get("col") or []
        if not isinstance(col_raw, list) or len(col_raw) != 2:
            raise HTTPException(status_code=400, detail="indicator_cross: params.col must be [left,right]")
        left_col = str(col_raw[0]).strip()
        right_col = str(col_raw[1]).strip()
        if not left_col or not right_col:
            raise HTTPException(status_code=400, detail="indicator_cross: invalid params.col")
        start = str(body.start or "").strip() or (body.calendar.open_dates[0] if body.calendar and body.calendar.open_dates else "")
        end = str(body.end or "").strip() or (body.calendar.open_dates[-1] if body.calendar and body.calendar.open_dates else "")
        if not start or not end:
            raise HTTPException(status_code=400, detail="indicator_cross: missing start/end")
        open_dates = list(body.calendar.open_dates) if body.calendar and body.calendar.open_dates else [str(p.get("date", "")).strip() for p in series]
        fee_body = body.fees.get(code) or FeeBody()
        out = run_indicator_cross_backtest(
            code=code,
            series=[dict(p) for p in series],
            open_dates=open_dates,
            start=start,
            end=end,
            totmoney=float(body.totmoney or 0.0),
            col=(left_col, right_col),
            fee=FeeCfg(buy_fee_rate=fee_body.buy_fee_rate, sell_fee_rate=fee_body.sell_fee_rate, round_label=fee_body.round_label),
        )
        return attach_report(out)

    if strategy == "indicator_points":
        code = str(body.params.get("code", "")).strip()
        if not code:
            raise HTTPException(status_code=400, detail="indicator_points: missing params.code")
        series = body.series.get(code) or []
        if not series:
            raise HTTPException(status_code=400, detail="indicator_points: missing series[code]")
        col = str(body.params.get("col", "")).strip()
        if not col:
            raise HTTPException(status_code=400, detail="indicator_points: missing params.col")
        buylow = bool(body.params.get("buylow", True))
        buy_raw = body.params.get("buy") or []
        if not isinstance(buy_raw, list) or not buy_raw:
            raise HTTPException(status_code=400, detail="indicator_points: params.buy must be list")
        buy: list[tuple[float, float]] = []
        for it in buy_raw:
            if not isinstance(it, (list, tuple)) or len(it) != 2:
                continue
            buy.append((float(it[0]), float(it[1])))
        sell_raw = body.params.get("sell", None)
        sell: list[tuple[float, float]] | None = None
        if sell_raw is not None:
            if not isinstance(sell_raw, list):
                raise HTTPException(status_code=400, detail="indicator_points: params.sell must be list")
            sell = []
            for it in sell_raw:
                if not isinstance(it, (list, tuple)) or len(it) != 2:
                    continue
                sell.append((float(it[0]), float(it[1])))
        start = str(body.start or "").strip() or (body.calendar.open_dates[0] if body.calendar and body.calendar.open_dates else "")
        end = str(body.end or "").strip() or (body.calendar.open_dates[-1] if body.calendar and body.calendar.open_dates else "")
        if not start or not end:
            raise HTTPException(status_code=400, detail="indicator_points: missing start/end")
        open_dates = list(body.calendar.open_dates) if body.calendar and body.calendar.open_dates else [str(p.get("date", "")).strip() for p in series]
        fee_body = body.fees.get(code) or FeeBody()
        out = run_indicator_points_backtest(
            code=code,
            series=[dict(p) for p in series],
            open_dates=open_dates,
            start=start,
            end=end,
            totmoney=float(body.totmoney or 0.0),
            col=col,
            buy=buy,
            sell=sell,
            buylow=buylow,
            fee=FeeCfg(buy_fee_rate=fee_body.buy_fee_rate, sell_fee_rate=fee_body.sell_fee_rate, round_label=fee_body.round_label),
        )
        return attach_report(out)

    if strategy == "bte_scheduled":
        code = str(body.params.get("code", "")).strip()
        if not code:
            raise HTTPException(status_code=400, detail="bte_scheduled: missing params.code")
        series = body.series.get(code) or []
        if not series:
            raise HTTPException(status_code=400, detail="bte_scheduled: missing series[code]")
        times = body.params.get("times") or []
        if not isinstance(times, list):
            raise HTTPException(status_code=400, detail="bte_scheduled: params.times must be list")
        value = float(body.params.get("value", 0.0))
        if body.calendar and body.calendar.open_dates:
            open_dates = list(body.calendar.open_dates)
        else:
            open_dates = [str(p.get("date", "")).strip() for p in series if str(p.get("date", "")).strip()]
        open_dates = sorted([d for d in open_dates if d])

        fee_body = body.fees.get(code) or FeeBody()
        fees = {code: FeeCfg(buy_fee_rate=fee_body.buy_fee_rate, sell_fee_rate=fee_body.sell_fee_rate, round_label=fee_body.round_label)}
        engine = BteEngine(series_map={code: [dict(p) for p in series]}, fees=fees, open_dates=open_dates, initial_cash=float(body.totmoney or 0.0))
        times_set = {str(d).strip() for d in times if str(d).strip()}
        for d in open_dates:
            if d in times_set:
                engine.buy(code, value, d)

        final_date = open_dates[-1] if open_dates else ""
        out = {
            "actions": engine.actions,
            "summary": {
                "final_date": final_date,
                "final_equity": engine.equity(final_date) if final_date else engine.cash,
                "cash": engine.cash,
                "holdings": engine.holdings,
            },
        }
        return attach_report(out)

    if strategy == "bte_average_scheduled":
        code = str(body.params.get("code", "")).strip()
        if not code:
            raise HTTPException(status_code=400, detail="bte_average_scheduled: missing params.code")
        series = body.series.get(code) or []
        if not series:
            raise HTTPException(status_code=400, detail="bte_average_scheduled: missing series[code]")
        times = body.params.get("times") or []
        if not isinstance(times, list):
            raise HTTPException(status_code=400, detail="bte_average_scheduled: params.times must be list")
        value = float(body.params.get("value", 0.0))
        if body.calendar and body.calendar.open_dates:
            open_dates = list(body.calendar.open_dates)
        else:
            open_dates = [str(p.get("date", "")).strip() for p in series if str(p.get("date", "")).strip()]
        start = str(body.start or "").strip() or open_dates[0]
        end = str(body.end or "").strip() or open_dates[-1]
        fee_body = body.fees.get(code) or FeeBody()
        out = run_average_scheduled_backtest(
            code=code,
            series=[dict(p) for p in series],
            open_dates=open_dates,
            start=start,
            end=end,
            totmoney=float(body.totmoney or 0.0),
            times=[str(x) for x in times],
            value=value,
            fee=FeeCfg(buy_fee_rate=fee_body.buy_fee_rate, sell_fee_rate=fee_body.sell_fee_rate, round_label=fee_body.round_label),
        )
        return attach_report(out)

    if strategy == "bte_scheduled_sell_on_xirr":
        code = str(body.params.get("code", "")).strip()
        if not code:
            raise HTTPException(status_code=400, detail="bte_scheduled_sell_on_xirr: missing params.code")
        series = body.series.get(code) or []
        if not series:
            raise HTTPException(status_code=400, detail="bte_scheduled_sell_on_xirr: missing series[code]")
        times = body.params.get("times") or []
        if not isinstance(times, list):
            raise HTTPException(status_code=400, detail="bte_scheduled_sell_on_xirr: params.times must be list")
        value = float(body.params.get("value", 0.0))
        threhold = float(body.params.get("threhold", 0.2))
        holding_time = int(body.params.get("holding_time", 180))
        check_weekday = int(body.params.get("check_weekday", 4))
        if body.calendar and body.calendar.open_dates:
            open_dates = list(body.calendar.open_dates)
        else:
            open_dates = [str(p.get("date", "")).strip() for p in series if str(p.get("date", "")).strip()]
        start = str(body.start or "").strip() or open_dates[0]
        end = str(body.end or "").strip() or open_dates[-1]
        fee_body = body.fees.get(code) or FeeBody()
        out = run_scheduled_sell_on_xirr_backtest(
            code=code,
            series=[dict(p) for p in series],
            open_dates=open_dates,
            start=start,
            end=end,
            totmoney=float(body.totmoney or 0.0),
            times=[str(x) for x in times],
            value=value,
            threhold=threhold,
            holding_time=holding_time,
            check_weekday=check_weekday,
            fee=FeeCfg(buy_fee_rate=fee_body.buy_fee_rate, sell_fee_rate=fee_body.sell_fee_rate, round_label=fee_body.round_label),
        )
        return attach_report(out)

    if strategy == "bte_tendency28":
        aim0 = str(body.params.get("aim0", "")).strip()
        aim1 = str(body.params.get("aim1", "")).strip()
        aim2 = str(body.params.get("aim2", "")).strip()
        if not aim0 or not aim1 or not aim2:
            raise HTTPException(status_code=400, detail="bte_tendency28: missing aim0/aim1/aim2")
        check_dates = body.params.get("check_dates") or []
        if not isinstance(check_dates, list) or not check_dates:
            raise HTTPException(status_code=400, detail="bte_tendency28: params.check_dates must be list")
        upthrehold = float(body.params.get("upthrehold", 1.0))
        diffthrehold = float(body.params.get("diffthrehold", upthrehold))
        prev = int(body.params.get("prev", 10))
        initial_money = float(body.params.get("initial_money", float(body.totmoney or 0.0) / 2.0))
        if body.calendar and body.calendar.open_dates:
            open_dates = list(body.calendar.open_dates)
        else:
            # union of all series dates
            all_dates: set[str] = set()
            for s in body.series.values():
                for p in s:
                    d = str(p.get("date", "")).strip()
                    if d:
                        all_dates.add(d)
            open_dates = sorted(list(all_dates))
        start = str(body.start or "").strip() or open_dates[0]
        end = str(body.end or "").strip() or open_dates[-1]

        series_map = {k: [dict(p) for p in v] for k, v in body.series.items()}
        fees: dict[str, FeeCfg] = {}
        for k, v in body.fees.items():
            fees[k] = FeeCfg(buy_fee_rate=v.buy_fee_rate, sell_fee_rate=v.sell_fee_rate, round_label=v.round_label)
        out = run_tendency28_backtest(
            series_map=series_map,
            fees=fees,
            open_dates=open_dates,
            start=start,
            end=end,
            totmoney=float(body.totmoney or 0.0),
            aim0=aim0,
            aim1=aim1,
            aim2=aim2,
            check_dates=[str(x) for x in check_dates],
            upthrehold=upthrehold,
            diffthrehold=diffthrehold,
            prev=prev,
            initial_money=initial_money,
        )
        return attach_report(out)

    if strategy == "bte_balance":
        check_dates = body.params.get("check_dates") or []
        if not isinstance(check_dates, list) or not check_dates:
            raise HTTPException(status_code=400, detail="bte_balance: params.check_dates must be list")
        portfolio_dict = body.params.get("portfolio_dict") or {}
        if not isinstance(portfolio_dict, dict) or not portfolio_dict:
            raise HTTPException(status_code=400, detail="bte_balance: params.portfolio_dict must be object")
        portfolio = {str(k).strip(): float(v) for k, v in portfolio_dict.items() if str(k).strip()}
        if not portfolio:
            raise HTTPException(status_code=400, detail="bte_balance: empty portfolio_dict")

        if body.calendar and body.calendar.open_dates:
            open_dates = list(body.calendar.open_dates)
        else:
            all_dates: set[str] = set()
            for s in body.series.values():
                for p in s:
                    d = str(p.get("date", "")).strip()
                    if d:
                        all_dates.add(d)
            open_dates = sorted(list(all_dates))
        start = str(body.start or "").strip() or open_dates[0]
        end = str(body.end or "").strip() or open_dates[-1]

        series_map = {k: [dict(p) for p in v] for k, v in body.series.items()}
        fees: dict[str, FeeCfg] = {}
        for k, v in body.fees.items():
            fees[k] = FeeCfg(buy_fee_rate=v.buy_fee_rate, sell_fee_rate=v.sell_fee_rate, round_label=v.round_label)
        out = run_balance_backtest(
            series_map=series_map,
            fees=fees,
            open_dates=open_dates,
            start=start,
            end=end,
            totmoney=float(body.totmoney or 0.0),
            portfolio_dict=portfolio,
            check_dates=[str(x) for x in check_dates],
        )
        return attach_report(out)

    raise HTTPException(status_code=400, detail=f"unsupported strategy: {strategy}")
