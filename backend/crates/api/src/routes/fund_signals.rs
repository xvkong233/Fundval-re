use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;

use crate::ml;
use crate::routes::auth;
use crate::routes::errors;
use crate::sources;
use crate::state::AppState;
use crate::tasks;

#[derive(Debug, Deserialize, Default)]
pub struct FundSignalsQuery {
    pub source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HorizonProbaOut {
    pub p_5t: Option<f64>,
    pub p_20t: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct BatchFundSignalsBody {
    pub fund_codes: Vec<String>,
    pub source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FundSignalsLiteOut {
    pub fund_code: String,
    pub source: String,
    pub as_of_date: Option<String>,
    pub best_peer: Option<PeerSignalsOut>,
    pub computed_at: String,
}

#[derive(Debug, Serialize)]
pub struct EnqueueSignalsBatchOut {
    pub task_id: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchSignalsPageQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct BatchSignalsPageOut {
    pub task_id: String,
    pub status: String,
    pub error: Option<String>,
    pub total: i64,
    pub done: i64,
    pub page: i64,
    pub page_size: i64,
    pub items: Vec<FundSignalsLiteOut>,
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

    let explicit_source = q.source.as_deref();
    let mut candidates: Vec<&'static str> = Vec::new();
    if let Some(raw) = explicit_source {
        let Some(s) = sources::normalize_source_name(raw) else {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("unknown source: {raw}") })),
            )
                .into_response();
        };
        candidates.push(s);
    } else {
        let primary_raw = state
            .config()
            .get_string("crawl_source")
            .unwrap_or_else(|| sources::SOURCE_TIANTIAN.to_string());
        if let Some(s) = sources::normalize_source_name(&primary_raw) {
            candidates.push(s);
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
                candidates.push(n);
            }
        }

        for s in sources::BUILTIN_SOURCES {
            candidates.push(s);
        }

        let mut dedup: Vec<&'static str> = Vec::new();
        for s in candidates {
            if !dedup.contains(&s) {
                dedup.push(s);
            }
        }
        candidates = dedup;
    }

    let mut source_name: Option<&'static str> = None;
    let mut as_of_date: Option<String> = None;
    for s in candidates {
        let d = latest_nav_date(pool, code, s).await.ok().flatten();
        if d.is_some() {
            source_name = Some(s);
            as_of_date = d;
            break;
        }
    }

    let Some(source_name) = source_name else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "该基金暂无净值数据（请先同步净值）" })),
        )
            .into_response();
    };

    // 取关联板块列表（最多 5 个 + 全市场兜底，共 6 个，避免一次请求过慢）
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

    // 始终包含“全市场”，确保 ML 能基于全量基金数据训练/推断（同时也作为关联板块缺失时的兜底）。
    let mut peers_list: Vec<(String, String)> = Vec::new();
    peers_list.push((ml::train::PEER_CODE_ALL.to_string(), "全市场".to_string()));

    for r in peer_rows {
        let peer_code: String = r.get("sec_code");
        let peer_name: String = r.get("sec_name");
        if peer_code.trim().is_empty() || peer_name.trim().is_empty() {
            continue;
        }
        if peer_code.trim() == ml::train::PEER_CODE_ALL {
            continue;
        }
        peers_list.push((peer_code, peer_name));
        // 预留 1 个给全市场，所以最多再加 5 个关联板块
        if peers_list.len() >= 6 {
            break;
        }
    }

    // 逐板块尝试补齐当日快照（缺失时会触发训练/计算兜底）
    for (peer_code, _) in &peers_list {
        let _ =
            ml::compute::compute_and_store_fund_snapshot(pool, code, peer_code, source_name).await;
    }

    let mut peers: Vec<PeerSignalsOut> = Vec::new();
    for (peer_code, peer_name) in peers_list {
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
                WHERE fund_code = $1 AND peer_code = $2 AND CAST(as_of_date AS TEXT) = $3
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

pub async fn batch(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BatchFundSignalsBody>,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let created_by = user_id.parse::<i64>().ok();

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

    enqueue_signals_batch_job(&state, pool, created_by, body).await
}

pub async fn batch_async(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<BatchFundSignalsBody>,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let created_by = user_id.parse::<i64>().ok();

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

    enqueue_signals_batch_job(&state, pool, created_by, body).await
}

async fn enqueue_signals_batch_job(
    state: &AppState,
    pool: &sqlx::AnyPool,
    created_by: Option<i64>,
    body: BatchFundSignalsBody,
) -> axum::response::Response {
    let source_raw = body.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let Some(source_name) = sources::normalize_source_name(source_raw) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("unknown source: {source_raw}") })),
        )
            .into_response();
    };

    let mut fund_codes: Vec<String> = Vec::new();
    for c in body.fund_codes {
        let s = c.trim();
        if s.is_empty() {
            continue;
        }
        if !fund_codes.contains(&s.to_string()) {
            fund_codes.push(s.to_string());
        }
    }
    if fund_codes.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "fund_codes 不能为空" })),
        )
            .into_response();
    }

    // 异步任务可承载更大批量；仍保守设置一个上限，避免恶意/误操作压垮 worker。
    fund_codes.truncate(2000);

    let payload = json!({
      "fund_codes": fund_codes,
      "source": source_name,
    });

    let task_id = match tasks::enqueue_task_job(pool, "signals_batch", &payload, 100, created_by).await {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(state, e),
            )
                .into_response();
        }
    };

    (StatusCode::ACCEPTED, Json(EnqueueSignalsBatchOut { task_id })).into_response()
}

pub async fn batch_async_page(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(task_id): axum::extract::Path<String>,
    axum::extract::Query(q): axum::extract::Query<BatchSignalsPageQuery>,
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

    let task_id = task_id.trim();
    if task_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "缺少 task_id" })),
        )
            .into_response();
    }

    let job = match tasks::get_task_job(pool, task_id).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Json(json!({ "error": "task not found" }))).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    if job.task_type != "signals_batch" {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "task_type mismatch" })),
        )
            .into_response();
    }

    let mut total = 0_i64;
    if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&job.payload_json) {
        if let Some(arr) = payload.get("fund_codes").and_then(|v| v.as_array()) {
            let mut dedup: Vec<String> = Vec::new();
            for v in arr {
                let s = v.as_str().unwrap_or("").trim();
                if s.is_empty() {
                    continue;
                }
                if !dedup.contains(&s.to_string()) {
                    dedup.push(s.to_string());
                }
            }
            total = dedup.len() as i64;
        }
    }

    let done_row = sqlx::query(
        r#"
        SELECT COUNT(1) as n
        FROM fund_signals_batch_item
        WHERE CAST(task_id AS TEXT) = $1
        "#,
    )
    .bind(task_id)
    .fetch_one(pool)
    .await;

    let done = match done_row {
        Ok(r) => r.get::<i64, _>("n"),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let page = q.page.unwrap_or(1).clamp(1, 100000);
    let page_size = q.page_size.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) * page_size;

    // NOTE: Any + SQLite 在 LIMIT/OFFSET 占位符上偶发兼容性差异；这里内联以确保稳定。
    let sql = format!(
        r#"
        SELECT
          fund_code,
          source,
          CAST(as_of_date AS TEXT) as as_of_date,
          CAST(best_peer_json AS TEXT) as best_peer_json,
          CAST(computed_at AS TEXT) as computed_at
        FROM fund_signals_batch_item
        WHERE CAST(task_id AS TEXT) = $1
        ORDER BY fund_code ASC
        LIMIT {page_size} OFFSET {offset}
        "#,
    );

    let rows = sqlx::query(&sql).bind(task_id).fetch_all(pool).await;
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

    let mut items: Vec<FundSignalsLiteOut> = Vec::with_capacity(rows.len());
    for r in rows {
        let best_peer_json: String = r.get("best_peer_json");
        let best_peer = serde_json::from_str::<PeerSignalsOut>(&best_peer_json).ok();
        items.push(FundSignalsLiteOut {
            fund_code: r.get("fund_code"),
            source: r.get("source"),
            as_of_date: r.try_get::<Option<String>, _>("as_of_date").ok().flatten(),
            best_peer,
            computed_at: r.get("computed_at"),
        });
    }

    (
        StatusCode::OK,
        Json(BatchSignalsPageOut {
            task_id: job.id,
            status: job.status,
            error: job.error,
            total,
            done,
            page,
            page_size,
            items,
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
