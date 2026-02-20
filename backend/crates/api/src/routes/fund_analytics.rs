use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;

use crate::analytics;
use crate::dbfmt;
use crate::routes::auth;
use crate::routes::errors;
use crate::sources;
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct FundAnalyticsQuery {
    pub range: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RiskFreeOut {
    pub tenor: String,
    pub rate_date: String,
    pub rate_percent: String,
    pub source: String,
    pub fetched_at: String,
}

#[derive(Debug, Serialize)]
pub struct MetricsOut {
    pub max_drawdown: String,
    pub ann_vol: String,
    pub sharpe: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FundAnalyticsOut {
    pub fund_code: String,
    pub range: String,
    pub source: String,
    pub rf: Option<RiskFreeOut>,
    pub metrics: MetricsOut,
    pub computed_at: String,
}

fn parse_trading_days(raw: Option<&str>) -> Result<i64, String> {
    let s = raw.unwrap_or("252T").trim();
    if s.is_empty() {
        return Ok(252);
    }
    if let Some(num) = s.strip_suffix('T') {
        let n = num
            .trim()
            .parse::<i64>()
            .map_err(|_| "range 必须是形如 252T 的交易日窗口".to_string())?;
        return Ok(n.clamp(2, 2000));
    }
    Err("range 必须是形如 252T 的交易日窗口".to_string())
}

fn fmt_f64(v: f64) -> String {
    if v.abs() < 1e-12 {
        return "0".to_string();
    }
    let s = format!("{v:.8}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() { "0".to_string() } else { s }
}

pub async fn retrieve(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(fund_code): axum::extract::Path<String>,
    Query(q): Query<FundAnalyticsQuery>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let code = fund_code.trim();
    if code.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "缺少 fund_code" })),
        )
            .into_response();
    }

    let range_raw = q.range.clone().unwrap_or_else(|| "252T".to_string());
    let n = match parse_trading_days(Some(&range_raw)) {
        Ok(v) => v,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    };

    let source_name_raw = q.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let Some(source_name) = sources::normalize_source_name(source_name_raw) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("数据源 {source_name_raw} 不存在") })),
        )
            .into_response();
    };

    let rows = sqlx::query(
        r#"
        SELECT
          CAST(h.unit_nav AS TEXT) as unit_nav
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE f.fund_code = $1 AND h.source_name = $2
        ORDER BY h.nav_date DESC
        LIMIT $3
        "#,
    )
    .bind(code)
    .bind(source_name)
    .bind(n)
    .fetch_all(pool)
    .await;

    let rows = match rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut navs: Vec<f64> = Vec::with_capacity(rows.len());
    for r in rows.into_iter().rev() {
        let s: String = r.get("unit_nav");
        if let Ok(v) = s.trim().parse::<f64>() {
            navs.push(v);
        }
    }

    if navs.len() < 2 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "净值数据不足" })),
        )
            .into_response();
    }

    let rf_row = sqlx::query(
        r#"
        SELECT
          CAST(rate_date AS TEXT) as rate_date,
          CAST(rate AS TEXT) as rate,
          source,
          CAST(fetched_at AS TEXT) as fetched_at
        FROM risk_free_rate_daily
        WHERE tenor = '3M'
        ORDER BY rate_date DESC, fetched_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await;

    let mut rf_percent: f64 = 0.0;
    let rf_out = match rf_row {
        Ok(Some(r)) => {
            let rate_raw: String = r.get("rate");
            rf_percent = rate_raw.trim().parse::<f64>().unwrap_or(0.0);
            let rate_percent = if rate_raw.trim().is_empty() {
                rate_raw
            } else {
                format!("{rf_percent:.4}")
            };
            Some(RiskFreeOut {
                tenor: "3M".to_string(),
                rate_date: r.get::<String, _>("rate_date"),
                rate_percent,
                source: r.get::<String, _>("source"),
                fetched_at: dbfmt::datetime_to_rfc3339(&r.get::<String, _>("fetched_at")),
            })
        }
        Ok(None) => None,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let metrics = analytics::metrics::compute_metrics_from_navs(&navs, rf_percent).unwrap();

    (
        StatusCode::OK,
        Json(FundAnalyticsOut {
            fund_code: code.to_string(),
            range: range_raw,
            source: source_name.to_string(),
            rf: rf_out,
            metrics: MetricsOut {
                max_drawdown: fmt_f64(metrics.max_drawdown),
                ann_vol: fmt_f64(metrics.ann_vol),
                sharpe: metrics.sharpe.map(fmt_f64),
            },
            computed_at: Utc::now().to_rfc3339_opts(SecondsFormat::AutoSi, false),
        }),
    )
        .into_response()
}
