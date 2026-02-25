from __future__ import annotations

from typing import Any

import pandas as pd

from app.qbot_xalpha_compat.mul import Mul


def summary_json(m: Mul, date: Any) -> list[dict[str, Any]]:
    df = m.summary(date)
    if isinstance(df, pd.DataFrame):
        safe = df.astype(object).where(pd.notnull(df), None)
        return safe.to_dict(orient="records")
    return []
