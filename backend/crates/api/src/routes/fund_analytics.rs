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
    pub gamma: Option<f64>,
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
    pub value_score: Option<ValueScoreOut>,
    pub value_scores: Option<Vec<ValueScoreOut>>,
    pub ce: Option<CeOut>,
    pub short_term: Option<ShortTermOut>,
    pub computed_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ValueScoreComponentOut {
    pub name: String,
    pub percentile_0_100: f64,
    pub weight: f64,
    pub weighted: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct ValueScoreOut {
    pub peer_kind: String,
    pub peer_name: String,
    pub peer_code: Option<String>,
    // legacy: keep field name for old clients, mirrors peer_name
    pub fund_type: String,
    pub score_0_100: f64,
    pub percentile_0_100: f64,
    pub sample_size: i64,
    pub components: Vec<ValueScoreComponentOut>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CeOut {
    pub gamma: f64,
    pub ce: f64,
    pub ann_excess: f64,
    pub ann_var: f64,
    pub percentile_0_100: f64,
}

#[derive(Debug, Serialize)]
pub struct ShortTermTrendOut {
    pub direction: String,
    pub strength_0_1: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ShortTermMeanReversionOut {
    pub bucket: String,
    pub score_0_1: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ShortTermCombinedOut {
    pub bucket: String,
    pub action_hint: String,
    pub rationale: String,
}

#[derive(Debug, Serialize)]
pub struct ShortTermOut {
    pub trend: ShortTermTrendOut,
    pub mean_reversion: ShortTermMeanReversionOut,
    pub combined: ShortTermCombinedOut,
}

struct PeerComputeCtx<'a> {
    source_name: &'a str,
    n: i64,
    target_code: &'a str,
    rf_percent: f64,
    gamma: f64,
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

    let gamma = q.gamma.unwrap_or(3.0);

    // source 选择逻辑：
    // - 如果显式传了 source：只用该 source（保持确定性）。
    // - 否则：优先使用 crawl_source，其次用 crawl_source_fallbacks，再兜底内置 sources。
    let explicit_source = q.source.as_deref();
    let mut source_candidates: Vec<&'static str> = Vec::new();
    if let Some(raw) = explicit_source {
        let Some(s) = sources::normalize_source_name(raw) else {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("数据源 {raw} 不存在") })),
            )
                .into_response();
        };
        source_candidates.push(s);
    } else {
        let primary_raw = state
            .config()
            .get_string("crawl_source")
            .unwrap_or_else(|| sources::SOURCE_TIANTIAN.to_string());
        if let Some(s) = sources::normalize_source_name(&primary_raw) {
            source_candidates.push(s);
        }

        let fallbacks_raw = state
            .config()
            .get_string("crawl_source_fallbacks")
            .unwrap_or_default();
        for p in fallbacks_raw.split(',') {
            let s = p.trim();
            if s.is_empty() {
                continue;
            }
            if let Some(n) = sources::normalize_source_name(s) {
                source_candidates.push(n);
            }
        }

        for s in sources::BUILTIN_SOURCES {
            source_candidates.push(s);
        }

        // 去重但保持顺序
        let mut dedup: Vec<&'static str> = Vec::new();
        for s in source_candidates {
            if !dedup.contains(&s) {
                dedup.push(s);
            }
        }
        source_candidates = dedup;
    }

    let mut source_name: Option<&'static str> = None;
    let mut rows: Vec<sqlx::any::AnyRow> = Vec::new();
    for s in source_candidates {
        let rs = sqlx::query(
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
        .bind(s)
        .bind(n)
        .fetch_all(pool)
        .await;

        let rs = match rs {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    errors::internal_json(&state, e),
                )
                    .into_response();
            }
        };

        if rs.len() >= 2 {
            source_name = Some(s);
            rows = rs;
            break;
        }
    }

    let Some(source_name) = source_name else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "净值数据不足（请先同步该基金净值）" })),
        )
            .into_response();
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
    let short_term =
        analytics::short_term::compute_short_term_signals(&navs).map(|s| ShortTermOut {
            trend: ShortTermTrendOut {
                direction: s.trend.direction,
                strength_0_1: s.trend.strength_0_1,
                reasons: s.trend.reasons,
            },
            mean_reversion: ShortTermMeanReversionOut {
                bucket: s.mean_reversion.bucket.as_str().to_string(),
                score_0_1: s.mean_reversion.score_0_1,
                reasons: s.mean_reversion.reasons,
            },
            combined: ShortTermCombinedOut {
                bucket: s.combined.bucket.as_str().to_string(),
                action_hint: s.combined.action_hint,
                rationale: s.combined.rationale,
            },
        });

    // Prefer "关联板块" (fund_relate_theme). Fallback to fund_type when missing.
    let themes_rows = sqlx::query(
        r#"
        SELECT sec_code, sec_name
        FROM fund_relate_theme
        WHERE fund_code = $1
        ORDER BY ol2top DESC, corr_1y DESC, sec_code ASC
        "#,
    )
    .bind(code)
    .fetch_all(pool)
    .await;

    let themes_rows = match themes_rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut candidates: Vec<(ValueScoreOut, Option<CeOut>)> = Vec::new();
    let ctx = PeerComputeCtx {
        source_name,
        n,
        target_code: code,
        rf_percent,
        gamma,
    };

    if !themes_rows.is_empty() {
        for r in themes_rows {
            let sec_code: String = r.get("sec_code");
            let sec_name: String = r.get("sec_name");
            if sec_code.trim().is_empty() || sec_name.trim().is_empty() {
                continue;
            }

            let (vs, ce) =
                compute_value_score_and_ce_by_sector(pool, sec_code.trim(), sec_name.trim(), &ctx)
                    .await;

            if let Some(vs) = vs {
                candidates.push((vs, ce));
            }
        }
    } else {
        let fund_type_row = sqlx::query("SELECT fund_type FROM fund WHERE fund_code = $1 LIMIT 1")
            .bind(code)
            .fetch_optional(pool)
            .await;
        let fund_type = match fund_type_row {
            Ok(Some(r)) => r
                .try_get::<Option<String>, _>("fund_type")
                .ok()
                .flatten()
                .unwrap_or_default(),
            Ok(None) => "".to_string(),
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    errors::internal_json(&state, e),
                )
                    .into_response();
            }
        };

        if !fund_type.trim().is_empty() {
            let (vs, ce) = compute_value_score_and_ce_by_fund_type(pool, &fund_type, &ctx).await;
            if let Some(vs) = vs {
                candidates.push((vs, ce));
            }
        }
    }

    // Fallback：没有行业/类型时，仍给出“全市场”对比，保证性价比不空。
    if candidates.is_empty() {
        let (vs, ce) = compute_value_score_and_ce_overall(pool, &ctx).await;
        if let Some(vs) = vs {
            candidates.push((vs, ce));
        }
    }

    let scores: Vec<ValueScoreOut> = candidates.iter().map(|(vs, _)| vs.clone()).collect();
    let best_idx = pick_best_by_sample_threshold(&scores, 100);
    let best_value_score = best_idx.map(|i| scores[i].clone());
    let best_ce = best_idx.and_then(|i| candidates[i].1.clone());
    let value_scores_out = if scores.is_empty() {
        None
    } else {
        Some(scores)
    };

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
            value_score: best_value_score,
            value_scores: value_scores_out,
            ce: best_ce,
            short_term,
            computed_at: Utc::now().to_rfc3339_opts(SecondsFormat::AutoSi, false),
        }),
    )
        .into_response()
}

async fn compute_value_score_and_ce_by_fund_type(
    pool: &sqlx::AnyPool,
    fund_type: &str,
    ctx: &PeerComputeCtx<'_>,
) -> (Option<ValueScoreOut>, Option<CeOut>) {
    let rows = sqlx::query(
        r#"
        SELECT f.fund_code as fund_code
        FROM fund f
        JOIN fund_nav_history h ON h.fund_id = f.id
        WHERE f.fund_type = $1 AND h.source_name = $2
        GROUP BY f.fund_code
        ORDER BY f.fund_code ASC
        LIMIT 500
        "#,
    )
    .bind(fund_type)
    .bind(ctx.source_name)
    .fetch_all(pool)
    .await;

    let rows = match rows {
        Ok(v) => v,
        Err(_) => return (None, None),
    };

    if rows.len() < 3 {
        return (None, None);
    }

    let mut samples: Vec<analytics::value_score::SampleMetrics> = Vec::with_capacity(rows.len());
    let mut ces: Vec<(String, analytics::ce::CeResult)> = Vec::new();

    for r in rows {
        let code: String = r.get("fund_code");
        if code.trim().is_empty() {
            continue;
        }

        let nav_rows = sqlx::query(
            r#"
            SELECT CAST(h.unit_nav AS TEXT) as unit_nav
            FROM fund_nav_history h
            JOIN fund f ON f.id = h.fund_id
            WHERE f.fund_code = $1 AND h.source_name = $2
            ORDER BY h.nav_date DESC
            LIMIT $3
            "#,
        )
        .bind(code.trim())
        .bind(ctx.source_name)
        .bind(ctx.n)
        .fetch_all(pool)
        .await;

        let nav_rows = match nav_rows {
            Ok(v) => v,
            Err(_) => continue,
        };

        let mut navs: Vec<f64> = Vec::with_capacity(nav_rows.len());
        for rr in nav_rows.into_iter().rev() {
            let s: String = rr.get("unit_nav");
            if let Ok(v) = s.trim().parse::<f64>() {
                navs.push(v);
            }
        }
        if navs.len() < 2 {
            continue;
        }

        let m = match analytics::metrics::compute_metrics_from_navs(&navs, ctx.rf_percent) {
            Some(v) => v,
            None => continue,
        };

        let ann_return = compute_ann_return_from_navs(&navs);
        let mdd_mag = (-m.max_drawdown).max(0.0);
        let calmar = ann_return.and_then(|r| {
            if mdd_mag > 0.0 {
                Some(r / mdd_mag)
            } else {
                None
            }
        });

        samples.push(analytics::value_score::SampleMetrics {
            fund_code: code.trim().to_string(),
            ann_return,
            ann_vol: Some(m.ann_vol),
            max_drawdown: Some(mdd_mag),
            sharpe: m.sharpe,
            calmar,
        });

        if let Some(ce) = analytics::ce::compute_ce_from_navs(&navs, ctx.rf_percent, ctx.gamma) {
            ces.push((code.trim().to_string(), ce));
        }
    }

    let weights = analytics::value_score::ValueScoreWeights::default();
    let value_score =
        analytics::value_score::compute_value_score(&samples, ctx.target_code, &weights);
    let value_score_out = value_score.map(|vs| ValueScoreOut {
        peer_kind: "fund_type".to_string(),
        peer_name: fund_type.to_string(),
        peer_code: None,
        fund_type: fund_type.to_string(),
        score_0_100: vs.score_0_100,
        percentile_0_100: vs.percentile_0_100,
        sample_size: vs.sample_size as i64,
        components: vs
            .components
            .into_iter()
            .map(|c| ValueScoreComponentOut {
                name: c.name.to_string(),
                percentile_0_100: c.percentile_0_100,
                weight: c.weight,
                weighted: c.weighted,
            })
            .collect::<Vec<_>>(),
    });

    let ce_out = {
        let target = ces
            .iter()
            .find(|(c, _)| c == ctx.target_code)
            .map(|(_, x)| *x);
        target.map(|t| {
            let values = ces.iter().map(|(_, x)| x.ce).collect::<Vec<_>>();
            let p = percentile_high_better(&values, t.ce);
            CeOut {
                gamma: t.gamma,
                ce: t.ce,
                ann_excess: t.ann_excess,
                ann_var: t.ann_var,
                percentile_0_100: p,
            }
        })
    };

    (value_score_out, ce_out)
}

async fn compute_value_score_and_ce_overall(
    pool: &sqlx::AnyPool,
    ctx: &PeerComputeCtx<'_>,
) -> (Option<ValueScoreOut>, Option<CeOut>) {
    let rows = sqlx::query(
        r#"
        SELECT f.fund_code as fund_code
        FROM fund f
        JOIN fund_nav_history h ON h.fund_id = f.id
        WHERE h.source_name = $1
        GROUP BY f.fund_code
        ORDER BY f.fund_code ASC
        LIMIT 500
        "#,
    )
    .bind(ctx.source_name)
    .fetch_all(pool)
    .await;

    let rows = match rows {
        Ok(v) => v,
        Err(_) => return (None, None),
    };

    if rows.len() < 3 {
        return (None, None);
    }

    let mut samples: Vec<analytics::value_score::SampleMetrics> = Vec::with_capacity(rows.len());
    let mut ces: Vec<(String, analytics::ce::CeResult)> = Vec::new();

    for r in rows {
        let code: String = r.get("fund_code");
        let code = code.trim();
        if code.is_empty() {
            continue;
        }

        let nav_rows = sqlx::query(
            r#"
            SELECT CAST(h.unit_nav AS TEXT) as unit_nav
            FROM fund_nav_history h
            JOIN fund f ON f.id = h.fund_id
            WHERE f.fund_code = $1 AND h.source_name = $2
            ORDER BY h.nav_date DESC
            LIMIT $3
            "#,
        )
        .bind(code)
        .bind(ctx.source_name)
        .bind(ctx.n)
        .fetch_all(pool)
        .await;

        let nav_rows = match nav_rows {
            Ok(v) => v,
            Err(_) => continue,
        };

        let mut navs: Vec<f64> = Vec::with_capacity(nav_rows.len());
        for rr in nav_rows.into_iter().rev() {
            let s: String = rr.get("unit_nav");
            if let Ok(v) = s.trim().parse::<f64>() {
                if v > 0.0 {
                    navs.push(v);
                }
            }
        }
        if navs.len() < 2 {
            continue;
        }

        let m = match analytics::metrics::compute_metrics_from_navs(&navs, ctx.rf_percent) {
            Some(v) => v,
            None => continue,
        };

        let ann_return = compute_ann_return_from_navs(&navs);
        let mdd_mag = (-m.max_drawdown).max(0.0);
        let calmar = ann_return.and_then(|r| {
            if mdd_mag > 0.0 { Some(r / mdd_mag) } else { None }
        });

        samples.push(analytics::value_score::SampleMetrics {
            fund_code: code.to_string(),
            ann_return,
            ann_vol: Some(m.ann_vol),
            max_drawdown: Some(mdd_mag),
            sharpe: m.sharpe,
            calmar,
        });

        if let Some(ce) = analytics::ce::compute_ce_from_navs(&navs, ctx.rf_percent, ctx.gamma) {
            ces.push((code.to_string(), ce));
        }
    }

    let weights = analytics::value_score::ValueScoreWeights::default();
    let value_score = analytics::value_score::compute_value_score(&samples, ctx.target_code, &weights);

    let value_score_out = value_score.map(|vs| ValueScoreOut {
        peer_kind: "overall".to_string(),
        peer_name: "全市场".to_string(),
        peer_code: None,
        fund_type: "全市场".to_string(),
        score_0_100: vs.score_0_100,
        percentile_0_100: vs.percentile_0_100,
        sample_size: vs.sample_size as i64,
        components: vs
            .components
            .into_iter()
            .map(|c| ValueScoreComponentOut {
                name: c.name.to_string(),
                percentile_0_100: c.percentile_0_100,
                weight: c.weight,
                weighted: c.weighted,
            })
            .collect::<Vec<_>>(),
    });

    let ce_out = {
        let target = ces
            .iter()
            .find(|(c, _)| c == ctx.target_code)
            .map(|(_, x)| *x);
        target.map(|t| {
            let values = ces.iter().map(|(_, x)| x.ce).collect::<Vec<_>>();
            let p = percentile_high_better(&values, t.ce);
            CeOut {
                gamma: t.gamma,
                ce: t.ce,
                ann_excess: t.ann_excess,
                ann_var: t.ann_var,
                percentile_0_100: p,
            }
        })
    };

    (value_score_out, ce_out)
}

async fn compute_value_score_and_ce_by_sector(
    pool: &sqlx::AnyPool,
    sec_code: &str,
    sec_name: &str,
    ctx: &PeerComputeCtx<'_>,
) -> (Option<ValueScoreOut>, Option<CeOut>) {
    let rows = sqlx::query(
        r#"
        SELECT f.fund_code as fund_code
        FROM fund f
        JOIN fund_nav_history h ON h.fund_id = f.id
        JOIN fund_relate_theme t ON t.fund_code = f.fund_code
        WHERE t.sec_code = $1 AND h.source_name = $2
        GROUP BY f.fund_code
        ORDER BY f.fund_code ASC
        LIMIT 500
        "#,
    )
    .bind(sec_code)
    .bind(ctx.source_name)
    .fetch_all(pool)
    .await;

    let rows = match rows {
        Ok(v) => v,
        Err(_) => return (None, None),
    };

    if rows.len() < 3 {
        return (None, None);
    }

    let mut samples: Vec<analytics::value_score::SampleMetrics> = Vec::with_capacity(rows.len());
    let mut ces: Vec<(String, analytics::ce::CeResult)> = Vec::new();

    for r in rows {
        let code: String = r.get("fund_code");
        if code.trim().is_empty() {
            continue;
        }

        let nav_rows = sqlx::query(
            r#"
            SELECT CAST(h.unit_nav AS TEXT) as unit_nav
            FROM fund_nav_history h
            JOIN fund f ON f.id = h.fund_id
            WHERE f.fund_code = $1 AND h.source_name = $2
            ORDER BY h.nav_date DESC
            LIMIT $3
            "#,
        )
        .bind(code.trim())
        .bind(ctx.source_name)
        .bind(ctx.n)
        .fetch_all(pool)
        .await;

        let nav_rows = match nav_rows {
            Ok(v) => v,
            Err(_) => continue,
        };

        let mut navs: Vec<f64> = Vec::with_capacity(nav_rows.len());
        for rr in nav_rows.into_iter().rev() {
            let s: String = rr.get("unit_nav");
            if let Ok(v) = s.trim().parse::<f64>() {
                navs.push(v);
            }
        }
        if navs.len() < 2 {
            continue;
        }

        let m = match analytics::metrics::compute_metrics_from_navs(&navs, ctx.rf_percent) {
            Some(v) => v,
            None => continue,
        };

        let ann_return = compute_ann_return_from_navs(&navs);
        let mdd_mag = (-m.max_drawdown).max(0.0);
        let calmar = ann_return.and_then(|r| {
            if mdd_mag > 0.0 {
                Some(r / mdd_mag)
            } else {
                None
            }
        });

        samples.push(analytics::value_score::SampleMetrics {
            fund_code: code.trim().to_string(),
            ann_return,
            ann_vol: Some(m.ann_vol),
            max_drawdown: Some(mdd_mag),
            sharpe: m.sharpe,
            calmar,
        });

        if let Some(ce) = analytics::ce::compute_ce_from_navs(&navs, ctx.rf_percent, ctx.gamma) {
            ces.push((code.trim().to_string(), ce));
        }
    }

    let weights = analytics::value_score::ValueScoreWeights::default();
    let value_score =
        analytics::value_score::compute_value_score(&samples, ctx.target_code, &weights);
    let value_score_out = value_score.map(|vs| ValueScoreOut {
        peer_kind: "sector".to_string(),
        peer_name: sec_name.to_string(),
        peer_code: Some(sec_code.to_string()),
        fund_type: sec_name.to_string(),
        score_0_100: vs.score_0_100,
        percentile_0_100: vs.percentile_0_100,
        sample_size: vs.sample_size as i64,
        components: vs
            .components
            .into_iter()
            .map(|c| ValueScoreComponentOut {
                name: c.name.to_string(),
                percentile_0_100: c.percentile_0_100,
                weight: c.weight,
                weighted: c.weighted,
            })
            .collect::<Vec<_>>(),
    });

    let ce_out = {
        let target = ces
            .iter()
            .find(|(c, _)| c == ctx.target_code)
            .map(|(_, x)| *x);
        target.map(|t| {
            let values = ces.iter().map(|(_, x)| x.ce).collect::<Vec<_>>();
            let p = percentile_high_better(&values, t.ce);
            CeOut {
                gamma: t.gamma,
                ce: t.ce,
                ann_excess: t.ann_excess,
                ann_var: t.ann_var,
                percentile_0_100: p,
            }
        })
    };

    (value_score_out, ce_out)
}

fn compute_ann_return_from_navs(navs: &[f64]) -> Option<f64> {
    if navs.len() < 2 {
        return None;
    }
    let mut daily: Vec<f64> = Vec::with_capacity(navs.len().saturating_sub(1));
    for i in 1..navs.len() {
        let prev = navs[i - 1];
        let cur = navs[i];
        if prev <= 0.0 {
            continue;
        }
        daily.push(cur / prev - 1.0);
    }
    if daily.is_empty() {
        return None;
    }
    let n = daily.len() as f64;
    let mean = daily.iter().sum::<f64>() / n;
    Some(mean * 252.0)
}

fn percentile_high_better(values: &[f64], target: f64) -> f64 {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return 50.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len() as f64;
    let mut rank = 0usize;
    for (i, x) in v.iter().enumerate() {
        if *x <= target {
            rank = i;
        } else {
            break;
        }
    }
    (rank as f64) / (n - 1.0).max(1.0) * 100.0
}

fn pick_best_by_sample_threshold(scores: &[ValueScoreOut], min_sample_size: i64) -> Option<usize> {
    if scores.is_empty() {
        return None;
    }

    let eligible: Vec<usize> = scores
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            if s.sample_size >= min_sample_size {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    let pool: Box<dyn Iterator<Item = usize>> = if !eligible.is_empty() {
        Box::new(eligible.into_iter())
    } else {
        Box::new(0..scores.len())
    };

    let mut best_i: Option<usize> = None;
    let mut best_score: f64 = f64::NEG_INFINITY;
    for i in pool {
        let s = scores[i].score_0_100;
        let s = if s.is_finite() { s } else { f64::NEG_INFINITY };
        if best_i.is_none() || s > best_score {
            best_i = Some(i);
            best_score = s;
        }
    }
    best_i
}

#[cfg(test)]
mod tests {
    use super::{ValueScoreOut, pick_best_by_sample_threshold};

    fn vs(name: &str, score: f64, sample_size: i64) -> ValueScoreOut {
        ValueScoreOut {
            peer_kind: "sector".to_string(),
            peer_name: name.to_string(),
            peer_code: Some("BK000000".to_string()),
            fund_type: name.to_string(),
            score_0_100: score,
            percentile_0_100: 50.0,
            sample_size,
            components: Vec::new(),
        }
    }

    #[test]
    fn pick_best_prefers_eligible_when_present() {
        let a = vs("A", 99.0, 10);
        let b = vs("B", 60.0, 100);
        let c = vs("C", 80.0, 90);
        let idx = pick_best_by_sample_threshold(&[a, b, c], 100).unwrap();
        assert_eq!(idx, 1);
    }

    #[test]
    fn pick_best_falls_back_to_overall_when_no_eligible() {
        let a = vs("A", 10.0, 10);
        let b = vs("B", 60.0, 99);
        let c = vs("C", 80.0, 90);
        let idx = pick_best_by_sample_threshold(&[a, b, c], 100).unwrap();
        assert_eq!(idx, 2);
    }
}
