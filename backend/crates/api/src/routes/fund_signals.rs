use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;

use crate::ml;
use crate::routes::auth;
use crate::sources;
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct FundSignalsQuery {
    pub source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HorizonProbaOut {
    pub p_5t: Option<f64>,
    pub p_20t: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct PeerSignalsOut {
    pub peer_code: String,
    pub peer_name: String,
    pub position_percentile_0_100: Option<f64>,
    pub position_bucket: Option<String>,
    pub dip_buy: HorizonProbaOut,
    pub magic_rebound: HorizonProbaOut,
    pub model_sample_size_20t: Option<i64>,
    pub computed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FundSignalsOut {
    pub fund_code: String,
    pub source: String,
    pub as_of_date: Option<String>,
    pub best_peer_code: Option<String>,
    pub peers: Vec<PeerSignalsOut>,
    pub computed_at: String,
}

pub async fn retrieve(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(fund_code): axum::extract::Path<String>,
    Query(q): Query<FundSignalsQuery>,
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

    let source_raw = q.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let Some(source_name) = sources::normalize_source_name(source_raw) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("unknown source: {source_raw}") })),
        )
            .into_response();
    };

    let as_of_date = latest_nav_date(pool, code, source_name)
        .await
        .ok()
        .flatten();

    // 取关联板块列表（最多 6 个，避免一次请求过慢）
    let peer_rows = sqlx::query(
        r#"
        SELECT sec_code, sec_name
        FROM fund_relate_theme
        WHERE fund_code = $1
        GROUP BY sec_code, sec_name
        ORDER BY sec_code ASC
        LIMIT 6
        "#,
    )
    .bind(code)
    .fetch_all(pool)
    .await;

    let peer_rows = peer_rows.unwrap_or_default();

    if peer_rows.is_empty() {
        // fallback: fund_type 还在，但“同类=关联板块”优先；这里先返回空 peers。
        return (
            StatusCode::OK,
            Json(FundSignalsOut {
                fund_code: code.to_string(),
                source: source_name.to_string(),
                as_of_date,
                best_peer_code: None,
                peers: Vec::new(),
                computed_at: Utc::now().to_rfc3339_opts(SecondsFormat::AutoSi, false),
            }),
        )
            .into_response();
    }

    // 逐板块尝试补齐当日快照（缺失时会触发训练/计算兜底）
    for r in &peer_rows {
        let peer_code: String = r.get("sec_code");
        let _ =
            ml::compute::compute_and_store_fund_snapshot(pool, code, &peer_code, source_name).await;
    }

    let mut peers: Vec<PeerSignalsOut> = Vec::new();
    for r in peer_rows {
        let peer_code: String = r.get("sec_code");
        let peer_name: String = r.get("sec_name");

        let snap = if let Some(ref d) = as_of_date {
            sqlx::query(
                r#"
                SELECT
                  position_percentile_0_100,
                  position_bucket,
                  dip_buy_proba_5t,
                  dip_buy_proba_20t,
                  magic_rebound_proba_5t,
                  magic_rebound_proba_20t,
                  CAST(computed_at AS TEXT) as computed_at
                FROM fund_signal_snapshot
                WHERE fund_code = $1 AND peer_code = $2 AND as_of_date = $3
                "#,
            )
            .bind(code)
            .bind(&peer_code)
            .bind(d)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
        } else {
            None
        };

        let (position_percentile_0_100, position_bucket, dip_buy, magic_rebound, computed_at) =
            if let Some(s) = snap {
                (
                    s.try_get::<Option<f64>, _>("position_percentile_0_100")
                        .ok()
                        .flatten(),
                    s.try_get::<Option<String>, _>("position_bucket")
                        .ok()
                        .flatten(),
                    HorizonProbaOut {
                        p_5t: s
                            .try_get::<Option<f64>, _>("dip_buy_proba_5t")
                            .ok()
                            .flatten(),
                        p_20t: s
                            .try_get::<Option<f64>, _>("dip_buy_proba_20t")
                            .ok()
                            .flatten(),
                    },
                    HorizonProbaOut {
                        p_5t: s
                            .try_get::<Option<f64>, _>("magic_rebound_proba_5t")
                            .ok()
                            .flatten(),
                        p_20t: s
                            .try_get::<Option<f64>, _>("magic_rebound_proba_20t")
                            .ok()
                            .flatten(),
                    },
                    s.try_get::<Option<String>, _>("computed_at").ok().flatten(),
                )
            } else {
                (
                    None,
                    None,
                    HorizonProbaOut {
                        p_5t: None,
                        p_20t: None,
                    },
                    HorizonProbaOut {
                        p_5t: None,
                        p_20t: None,
                    },
                    None,
                )
            };

        let model_sample_size_20t = ml_sector_sample_size(pool, &peer_code).await.ok().flatten();

        peers.push(PeerSignalsOut {
            peer_code,
            peer_name,
            position_percentile_0_100,
            position_bucket,
            dip_buy,
            magic_rebound,
            model_sample_size_20t,
            computed_at,
        });
    }

    let best_peer_code = pick_best_peer(&peers);

    (
        StatusCode::OK,
        Json(FundSignalsOut {
            fund_code: code.to_string(),
            source: source_name.to_string(),
            as_of_date,
            best_peer_code,
            peers,
            computed_at: Utc::now().to_rfc3339_opts(SecondsFormat::AutoSi, false),
        }),
    )
        .into_response()
}

async fn latest_nav_date(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    source_name: &str,
) -> Result<Option<String>, String> {
    let row = sqlx::query(
        r#"
        SELECT CAST(h.nav_date AS TEXT) as nav_date
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE f.fund_code = $1 AND h.source_name = $2
        ORDER BY h.nav_date DESC
        LIMIT 1
        "#,
    )
    .bind(fund_code)
    .bind(source_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|r| r.get::<String, _>("nav_date")))
}

async fn ml_sector_sample_size(
    pool: &sqlx::AnyPool,
    peer_code: &str,
) -> Result<Option<i64>, String> {
    let row = sqlx::query(
        r#"
        SELECT metrics_json
        FROM ml_sector_model
        WHERE peer_code = $1 AND task = 'dip_buy' AND horizon_days = 20
        "#,
    )
    .bind(peer_code)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some(row) = row else {
        return Ok(None);
    };

    let raw: String = row.get("metrics_json");
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(v.get("sample_size").and_then(|x| x.as_i64()))
}

fn pick_best_peer(peers: &[PeerSignalsOut]) -> Option<String> {
    if peers.is_empty() {
        return None;
    }

    let mut best: Option<(i32, f64, String)> = None;
    for p in peers {
        let sample_ok = p.model_sample_size_20t.unwrap_or(0) >= 100;
        let tier = if sample_ok { 2 } else { 1 };
        let score = p.dip_buy.p_20t.or(p.dip_buy.p_5t).unwrap_or(0.0);
        let key = (tier, score, p.peer_code.clone());
        if best
            .as_ref()
            .is_none_or(|b| key.0 > b.0 || (key.0 == b.0 && key.1 > b.1))
        {
            best = Some(key);
        }
    }
    best.map(|(_, _, code)| code)
}
