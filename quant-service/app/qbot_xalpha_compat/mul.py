from __future__ import annotations

from typing import Any

import pandas as pd

from app.qbot_xalpha_compat.trade import Trade
from app.qbot_xalpha.rounding import myround


class Mul:
    """
    极简版 xalpha.multiple.mul（显式输入 Trade 对象）。
    目标：提供 combsummary/summary(date) 作为“可读报表输出”。
    """

    def __init__(self, *trades: Trade) -> None:
        self.fundtradeobj = tuple(trades)

    def combsummary(self, date: Any) -> pd.DataFrame:
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
        rows: list[pd.DataFrame] = []
        for t in self.fundtradeobj:
            rows.append(t.dailyreport(date))
        summarydf = pd.concat(rows, ignore_index=True) if rows else pd.DataFrame([], columns=cols)

        tcurrentvalue = float(summarydf["基金现值"].sum()) if len(summarydf) else 0.0
        tpurchase = float(summarydf["基金总申购"].sum()) if len(summarydf) else 0.0
        tbtnk = float(summarydf["历史最大占用"].sum()) if len(summarydf) else 0.0
        tcost = float(summarydf["基金持有成本"].sum()) if len(summarydf) else 0.0
        toutput = float(summarydf["基金分红与赎回"].sum()) if len(summarydf) else 0.0
        tearn = float(summarydf["基金收益总额"].sum()) if len(summarydf) else 0.0
        trate = myround((tearn / tbtnk * 100.0) if tbtnk > 0 else 0.0, 4)

        total_row = pd.DataFrame(
            [
                {
                    "基金名称": "总计",
                    "基金代码": "total",
                    "当日净值": float("nan"),
                    "单位成本": float("nan"),
                    "持有份额": float("nan"),
                    "基金现值": tcurrentvalue,
                    "基金总申购": tpurchase,
                    "历史最大占用": tbtnk,
                    "基金持有成本": tcost,
                    "基金分红与赎回": toutput,
                    "基金收益总额": tearn,
                    "投资收益率": trate,
                }
            ],
            columns=cols,
        )
        summarydf = pd.concat([summarydf, total_row], ignore_index=True)
        return summarydf[cols]

    summary = combsummary
