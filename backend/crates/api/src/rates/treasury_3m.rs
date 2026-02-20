use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Treasury3mRate {
    pub rate_date: String,
    /// 年化收益率（百分比单位），例如 `1.3428` 表示 1.3428%
    pub rate_percent: f64,
}

#[derive(Debug, Deserialize)]
struct ChinabondCurvePayload {
    #[serde(default)]
    worktime: String,
    #[serde(default, rename = "seriesData")]
    series_data: Vec<[f64; 2]>,
}

pub fn parse_chinabond_curve_json(raw: &str) -> Result<Treasury3mRate, String> {
    let payload: ChinabondCurvePayload =
        serde_json::from_str(raw).map_err(|e| format!("invalid json: {e}"))?;

    let date = payload.worktime.trim().to_string();
    if date.is_empty() {
        return Err("missing worktime".to_string());
    }

    // 中债国债收益率曲线返回的 x 轴单位为“年”。3M = 0.25 年。
    let mut found: Option<f64> = None;
    for [years, rate] in payload.series_data {
        if (years - 0.25).abs() < 1e-9 {
            found = Some(rate);
            break;
        }
    }

    let Some(rate) = found else {
        return Err("missing 3M(0.25y) point".to_string());
    };

    Ok(Treasury3mRate {
        rate_date: date,
        rate_percent: rate,
    })
}

