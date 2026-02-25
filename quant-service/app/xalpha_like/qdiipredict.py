from __future__ import annotations

from typing import Any


def predict_from_legs(last_value: float, legs: list[dict[str, Any]]) -> dict[str, Any]:
    """
    一个可审计、纯函数的 QDII 预测：
    - 输入为调用方提供的各腿当日涨跌倍数 ratio，以及对应汇率倍数 currency_ratio
    - 计算组合当日净值倍数 delta，并输出 predicted_value = last_value * delta

    说明：这是对 xalpha QDIIPredict 的“核心组合涨跌计算”做最小可用封装，
    不在服务端隐式抓取外盘/汇率数据，避免不可控的外部依赖。
    """

    lv = float(last_value)
    if not legs:
        return {"last_value": lv, "delta": 1.0, "predicted_value": lv, "components": []}

    total_percent = 0.0
    weighted = 0.0
    components: list[dict[str, Any]] = []

    for leg in legs:
        code = str(leg.get("code", "")).strip()
        percent = float(leg.get("percent", 0.0))
        ratio = float(leg.get("ratio", 1.0))
        currency_ratio = float(leg.get("currency_ratio", 1.0))
        if percent <= 0.0:
            continue

        total_percent += percent
        contrib = (percent / 100.0) * ratio * currency_ratio
        weighted += contrib
        components.append(
            {
                "code": code,
                "percent": percent,
                "ratio": ratio,
                "currency_ratio": currency_ratio,
                "contrib": contrib,
            }
        )

    remain = max(0.0, 100.0 - total_percent)
    cash_contrib = remain / 100.0
    delta = weighted + cash_contrib
    predicted = lv * delta

    return {
        "last_value": lv,
        "delta": delta,
        "predicted_value": predicted,
        "cash_percent": remain,
        "components": components,
    }

