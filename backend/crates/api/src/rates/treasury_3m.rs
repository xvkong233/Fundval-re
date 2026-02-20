use serde::Deserialize;
use uuid::Uuid;

pub const DEFAULT_CHINABOND_CURVE_URL: &str =
    "https://indices.chinabond.com.cn/cbweb-czb-web/czb/czbChartIndex";

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

pub async fn fetch_chinabond_3m(client: &reqwest::Client, url: &str) -> Result<Treasury3mRate, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("missing url".to_string());
    }

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("upstream status={status}"));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| format!("read body failed: {e}"))?;
    parse_chinabond_curve_json(&text)
}

pub async fn upsert_risk_free_rate_3m(
    pool: &sqlx::AnyPool,
    rate: &Treasury3mRate,
    source: &str,
) -> Result<(), String> {
    let rate_date = rate.rate_date.trim();
    let source = source.trim();
    if rate_date.is_empty() || source.is_empty() {
        return Err("invalid rate_date/source".to_string());
    }

    let id = Uuid::new_v4().to_string();

    let sql_pg = r#"
        INSERT INTO risk_free_rate_daily (id, rate_date, tenor, rate, source, fetched_at, created_at, updated_at)
        VALUES (CAST($1 AS uuid), $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (rate_date, tenor, source) DO UPDATE
          SET rate = EXCLUDED.rate,
              fetched_at = CURRENT_TIMESTAMP,
              updated_at = CURRENT_TIMESTAMP
    "#;

    let sql_any = r#"
        INSERT INTO risk_free_rate_daily (id, rate_date, tenor, rate, source, fetched_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (rate_date, tenor, source) DO UPDATE
          SET rate = EXCLUDED.rate,
              fetched_at = CURRENT_TIMESTAMP,
              updated_at = CURRENT_TIMESTAMP
    "#;

    let r = sqlx::query(sql_pg)
        .bind(&id)
        .bind(rate_date)
        .bind("3M")
        .bind(rate.rate_percent.to_string())
        .bind(source)
        .execute(pool)
        .await;

    if r.is_ok() {
        return Ok(());
    }

    sqlx::query(sql_any)
        .bind(&id)
        .bind(rate_date)
        .bind("3M")
        .bind(rate.rate_percent.to_string())
        .bind(source)
        .execute(pool)
        .await
        .map_err(|e| format!("upsert risk_free_rate_daily failed: {e}"))?;

    Ok(())
}
