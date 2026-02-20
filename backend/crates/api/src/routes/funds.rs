use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use chrono::{DateTime, Datelike, Duration, NaiveDate, SecondsFormat, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;
use tokio::{sync::Semaphore, task::JoinSet};
use uuid::Uuid;

use crate::eastmoney;
use crate::routes::auth;
use crate::routes::errors;
use crate::sources;
use crate::state::AppState;

async fn upsert_basic_fund(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    fund_name: &str,
    fund_type: Option<&str>,
) -> Result<(), String> {
    let code = fund_code.trim();
    let name = fund_name.trim();
    if code.is_empty() || name.is_empty() {
        return Err("invalid fund_code/fund_name".to_string());
    }

    sqlx::query(
        r#"
        INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
        VALUES (CAST($1 AS uuid), $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (fund_code) DO UPDATE
          SET fund_name = EXCLUDED.fund_name,
              fund_type = COALESCE(EXCLUDED.fund_type, fund.fund_type),
              updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(code)
    .bind(name)
    .bind(fund_type)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

async fn ensure_fund_exists(
    pool: &sqlx::AnyPool,
    client: &reqwest::Client,
    fund_code: &str,
) -> Result<bool, String> {
    let code = fund_code.trim();
    if code.is_empty() {
        return Ok(false);
    }

    let exists = sqlx::query("SELECT 1 FROM fund WHERE fund_code = $1")
        .bind(code)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .is_some();
    if exists {
        return Ok(true);
    }

    // 优先用天天基金估值接口拿到基金名称（更轻量），失败再回退 fund list。
    if let Ok(Some(est)) = eastmoney::fetch_estimate(client, code).await {
        let _ = upsert_basic_fund(pool, code, &est.fund_name, None).await;
        return Ok(true);
    }

    let list = eastmoney::fetch_fund_list(client).await?;
    if let Some(item) = list.into_iter().find(|it| it.fund_code.trim() == code) {
        let _ = upsert_basic_fund(pool, code, &item.fund_name, Some(&item.fund_type)).await;
        return Ok(true);
    }

    Ok(false)
}

#[derive(Debug, Deserialize)]
pub struct FundListQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub search: Option<String>,
    pub fund_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FundItem {
    pub id: String,
    pub fund_code: String,
    pub fund_name: String,
    pub fund_type: Option<String>,
    pub latest_nav: Option<String>,
    pub latest_nav_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct FundListResponse {
    pub count: i64,
    pub results: Vec<FundItem>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(q): Query<FundListQuery>,
) -> axum::response::Response {
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

    let page_size = q.page_size.unwrap_or(20).clamp(1, 200);
    let page = q.page.unwrap_or(1).max(1);
    let offset = (page - 1) * page_size;

    let mut where_sql = String::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(search) = q.search.as_ref().and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    }) {
        where_sql.push_str(
            " WHERE (LOWER(fund_code) LIKE LOWER($1) OR LOWER(fund_name) LIKE LOWER($1))",
        );
        binds.push(format!("%{search}%"));
    }

    if let Some(ft) = q.fund_type.as_ref().and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    }) {
        let idx = binds.len() + 1;
        if where_sql.is_empty() {
            where_sql.push_str(&format!(" WHERE fund_type = ${idx}"));
        } else {
            where_sql.push_str(&format!(" AND fund_type = ${idx}"));
        }
        binds.push(ft);
    }

    // count
    let count_sql = format!("SELECT COUNT(*) as cnt FROM fund{where_sql}");
    let mut count_q = sqlx::query(&count_sql);
    for b in &binds {
        count_q = count_q.bind(b);
    }
    let count_row = match count_q.fetch_one(pool).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };
    let count: i64 = count_row.get::<i64, _>("cnt");

    // page data
    let list_sql = format!(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          fund_code,
          fund_name,
          fund_type,
          CAST(latest_nav AS TEXT) as latest_nav,
          CAST(latest_nav_date AS TEXT) as latest_nav_date,
          CAST(created_at AS TEXT) as created_at,
          CAST(updated_at AS TEXT) as updated_at
        FROM fund
        {where_sql}
        ORDER BY fund_code ASC
        LIMIT {page_size} OFFSET {offset}
        "#
    );

    let mut list_q = sqlx::query(&list_sql);
    for b in &binds {
        list_q = list_q.bind(b);
    }

    let rows = match list_q.fetch_all(pool).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    // 开箱即用：当 DB 中搜索不到任何基金时，回退到上游基金列表做匹配（无需先手动 sync）。
    let search_keyword = q
        .search
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if count == 0 && rows.is_empty() {
        let Some(keyword) = search_keyword else {
            return (
                StatusCode::OK,
                Json(FundListResponse {
                    count: 0,
                    results: vec![],
                }),
            )
                .into_response();
        };
        let keyword_lc = keyword.to_lowercase();

        let client = match eastmoney::build_client() {
            Ok(c) => c,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("创建 HTTP 客户端失败: {e}") })),
                )
                    .into_response();
            }
        };

        let list = match eastmoney::fetch_fund_list(&client).await {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e })),
                )
                    .into_response();
            }
        };

        let mut matched: Vec<eastmoney::FundListItem> = Vec::new();
        for it in list {
            let code = it.fund_code.trim();
            let name = it.fund_name.trim();
            if code.is_empty() || name.is_empty() {
                continue;
            }
            if code.contains(&keyword)
                || name.contains(&keyword)
                || code.to_lowercase().contains(&keyword_lc)
                || name.to_lowercase().contains(&keyword_lc)
            {
                matched.push(it);
            }
        }

        let remote_count = matched.len() as i64;
        let slice = matched
            .into_iter()
            .skip(offset as usize)
            .take(page_size as usize)
            .collect::<Vec<_>>();

        // 写入 DB 作为缓存（仅插入本页命中的结果）
        for it in &slice {
            let _ =
                upsert_basic_fund(pool, &it.fund_code, &it.fund_name, Some(&it.fund_type)).await;
        }

        // 从 DB 回读（补齐 id/时间字段）
        let codes: Vec<String> = slice
            .iter()
            .map(|it| it.fund_code.trim().to_string())
            .collect();
        let mut by_code: std::collections::HashMap<String, FundItem> =
            std::collections::HashMap::new();
        if !codes.is_empty() {
            let mut sql = String::from(
                r#"
                SELECT
                  CAST(id AS TEXT) as id,
                  fund_code,
                  fund_name,
                  fund_type,
                  CAST(latest_nav AS TEXT) as latest_nav,
                  CAST(latest_nav_date AS TEXT) as latest_nav_date,
                  CAST(created_at AS TEXT) as created_at,
                  CAST(updated_at AS TEXT) as updated_at
                FROM fund
                WHERE fund_code IN (
                "#,
            );
            for (i, _) in codes.iter().enumerate() {
                if i > 0 {
                    sql.push_str(", ");
                }
                sql.push_str(&format!("${}", i + 1));
            }
            sql.push_str(")\n");

            let mut q = sqlx::query(&sql);
            for code in &codes {
                q = q.bind(code);
            }

            if let Ok(db_rows) = q.fetch_all(pool).await {
                for row in db_rows {
                    let code = row.get::<String, _>("fund_code");
                    by_code.insert(
                        code.clone(),
                        FundItem {
                            id: row.get::<String, _>("id"),
                            fund_code: code,
                            fund_name: row.get::<String, _>("fund_name"),
                            fund_type: row.get::<Option<String>, _>("fund_type"),
                            latest_nav: row.get::<Option<String>, _>("latest_nav"),
                            latest_nav_date: row.get::<Option<String>, _>("latest_nav_date"),
                            created_at: crate::dbfmt::datetime_to_rfc3339(
                                &row.get::<String, _>("created_at"),
                            ),
                            updated_at: crate::dbfmt::datetime_to_rfc3339(
                                &row.get::<String, _>("updated_at"),
                            ),
                        },
                    );
                }
            }
        }

        let mut results: Vec<FundItem> = Vec::with_capacity(codes.len());
        for code in codes {
            if let Some(item) = by_code.remove(&code) {
                results.push(item);
            }
        }

        return (
            StatusCode::OK,
            Json(FundListResponse {
                count: remote_count,
                results,
            }),
        )
            .into_response();
    }

    let results = rows
        .into_iter()
        .map(|row| FundItem {
            id: row.get::<String, _>("id"),
            fund_code: row.get::<String, _>("fund_code"),
            fund_name: row.get::<String, _>("fund_name"),
            fund_type: row.get::<Option<String>, _>("fund_type"),
            latest_nav: row.get::<Option<String>, _>("latest_nav"),
            latest_nav_date: row.get::<Option<String>, _>("latest_nav_date"),
            created_at: crate::dbfmt::datetime_to_rfc3339(&row.get::<String, _>("created_at")),
            updated_at: crate::dbfmt::datetime_to_rfc3339(&row.get::<String, _>("updated_at")),
        })
        .collect::<Vec<_>>();

    (StatusCode::OK, Json(FundListResponse { count, results })).into_response()
}

pub async fn retrieve(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(fund_code): axum::extract::Path<String>,
) -> axum::response::Response {
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

    let row = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          fund_code,
          fund_name,
          fund_type,
          CAST(latest_nav AS TEXT) as latest_nav,
          CAST(latest_nav_date AS TEXT) as latest_nav_date,
          CAST(created_at AS TEXT) as created_at,
          CAST(updated_at AS TEXT) as updated_at
        FROM fund
        WHERE fund_code = $1
        "#,
    )
    .bind(&fund_code)
    .fetch_optional(pool)
    .await;

    let row = match row {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let row = match row {
        Some(v) => Some(v),
        None => {
            // 开箱即用：fund 表里没有时，尝试从上游补齐后再查一次。
            let client = match eastmoney::build_client() {
                Ok(c) => c,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("创建 HTTP 客户端失败: {e}") })),
                    )
                        .into_response();
                }
            };

            let _ = ensure_fund_exists(pool, &client, &fund_code).await;
            sqlx::query(
                r#"
                SELECT
                  CAST(id AS TEXT) as id,
                  fund_code,
                  fund_name,
                  fund_type,
                  CAST(latest_nav AS TEXT) as latest_nav,
                  CAST(latest_nav_date AS TEXT) as latest_nav_date,
                  CAST(created_at AS TEXT) as created_at,
                  CAST(updated_at AS TEXT) as updated_at
                FROM fund
                WHERE fund_code = $1
                "#,
            )
            .bind(&fund_code)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
        }
    };

    let Some(row) = row else {
        // 对齐 DRF 默认 404 响应格式
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    };

    let item = FundItem {
        id: row.get::<String, _>("id"),
        fund_code: row.get::<String, _>("fund_code"),
        fund_name: row.get::<String, _>("fund_name"),
        fund_type: row.get::<Option<String>, _>("fund_type"),
        latest_nav: row.get::<Option<String>, _>("latest_nav"),
        latest_nav_date: row.get::<Option<String>, _>("latest_nav_date"),
        created_at: crate::dbfmt::datetime_to_rfc3339(&row.get::<String, _>("created_at")),
        updated_at: crate::dbfmt::datetime_to_rfc3339(&row.get::<String, _>("updated_at")),
    };

    (StatusCode::OK, Json(item)).into_response()
}

pub async fn estimate(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(fund_code): axum::extract::Path<String>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> axum::response::Response {
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

    let row =
        sqlx::query("SELECT CAST(id AS TEXT) as id, fund_name FROM fund WHERE fund_code = $1")
            .bind(&fund_code)
            .fetch_optional(pool)
            .await;

    let row = match row {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let row = match row {
        Some(v) => Some(v),
        None => {
            // 开箱即用：fund 表里没有时，尝试从上游补齐后再继续估值。
            let client = match eastmoney::build_client() {
                Ok(c) => c,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("创建 HTTP 客户端失败: {e}") })),
                    )
                        .into_response();
                }
            };
            let _ = ensure_fund_exists(pool, &client, &fund_code).await;
            sqlx::query("SELECT CAST(id AS TEXT) as id, fund_name FROM fund WHERE fund_code = $1")
                .bind(&fund_code)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten()
        }
    };

    let Some(row) = row else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    };
    let fund_id: String = row.get("id");
    let fund_name: String = row.get("fund_name");

    let source_name_raw = q
        .get("source")
        .map(|s| s.as_str())
        .unwrap_or(sources::SOURCE_TIANTIAN);
    let Some(source_name) = sources::normalize_source_name(source_name_raw) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("数据源 {source_name_raw} 不存在") })),
        )
            .into_response();
    };

    let client = match eastmoney::build_client() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("创建 HTTP 客户端失败: {e}") })),
            )
                .into_response();
        }
    };

    match source_name {
        sources::SOURCE_TIANTIAN => match eastmoney::fetch_estimate(&client, &fund_code).await {
            Ok(Some(data)) => {
                let estimate_date = data.estimate_time.date();
                let estimate_nav = data.estimate_nav;
                let _ = upsert_estimate_accuracy(
                    pool,
                    sources::SOURCE_TIANTIAN,
                    &fund_id,
                    estimate_date.to_string(),
                    estimate_nav,
                )
                .await;

                (
                    StatusCode::OK,
                    Json(json!({
                      "fund_code": data.fund_code,
                      "fund_name": fund_name,
                      "estimate_nav": data.estimate_nav.to_string(),
                      "estimate_growth": data.estimate_growth.to_string(),
                      "estimate_time": data.estimate_time.format("%Y-%m-%dT%H:%M:%S").to_string()
                    })),
                )
                    .into_response()
            }
            Ok(None) => (StatusCode::OK, Json(serde_json::Value::Null)).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e })),
            )
                .into_response(),
        },
        sources::SOURCE_DANJUAN => {
            match sources::danjuan::fetch_latest_row(&client, &fund_code).await {
                Ok(Some(row)) => {
                    let estimate_date = row.nav_date;
                    let estimate_nav = row.unit_nav;
                    let _ = upsert_estimate_accuracy(
                        pool,
                        sources::SOURCE_DANJUAN,
                        &fund_id,
                        estimate_date.to_string(),
                        estimate_nav,
                    )
                    .await;

                    (
                        StatusCode::OK,
                        Json(json!({
                          "fund_code": fund_code.as_str(),
                          "fund_name": fund_name,
                          "estimate_nav": row.unit_nav.to_string(),
                          "estimate_growth": row.daily_growth.unwrap_or(Decimal::ZERO).to_string(),
                          "estimate_time": format!("{}T15:00:00", row.nav_date)
                        })),
                    )
                        .into_response()
                }
                Ok(None) => (StatusCode::OK, Json(serde_json::Value::Null)).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e })),
                )
                    .into_response(),
            }
        }
        sources::SOURCE_THS => {
            let series = match sources::ths::fetch_nav_series(&client, &fund_code).await {
                Ok(v) => v,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": e })),
                    )
                        .into_response();
                }
            };
            if series.is_empty() {
                return (StatusCode::OK, Json(serde_json::Value::Null)).into_response();
            }

            // latest & previous
            let mut latest = &series[0];
            for r in &series[1..] {
                if r.nav_date > latest.nav_date {
                    latest = r;
                }
            }
            let mut prev: Option<&eastmoney::NavRow> = None;
            for r in &series {
                if r.nav_date < latest.nav_date {
                    prev = match prev {
                        None => Some(r),
                        Some(p) => {
                            if r.nav_date > p.nav_date {
                                Some(r)
                            } else {
                                Some(p)
                            }
                        }
                    };
                }
            }
            let growth = prev
                .and_then(|p| {
                    if p.unit_nav > Decimal::ZERO {
                        Some(((latest.unit_nav - p.unit_nav) / p.unit_nav) * Decimal::from(100))
                    } else {
                        None
                    }
                })
                .unwrap_or(Decimal::ZERO);

            let estimate_date = latest.nav_date;
            let estimate_nav = latest.unit_nav;
            let _ = upsert_estimate_accuracy(
                pool,
                sources::SOURCE_THS,
                &fund_id,
                estimate_date.to_string(),
                estimate_nav,
            )
            .await;

            (
                StatusCode::OK,
                Json(json!({
                  "fund_code": fund_code.as_str(),
                  "fund_name": fund_name,
                  "estimate_nav": latest.unit_nav.to_string(),
                  "estimate_growth": growth.round_dp(4).to_string(),
                  "estimate_time": format!("{}T15:00:00", latest.nav_date)
                })),
            )
                .into_response()
        }
        _ => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("数据源 {source_name} 不存在") })),
        )
            .into_response(),
    }
}

pub async fn accuracy(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(fund_code): axum::extract::Path<String>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> axum::response::Response {
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

    let row = sqlx::query("SELECT CAST(id AS TEXT) as id FROM fund WHERE fund_code = $1")
        .bind(&fund_code)
        .fetch_optional(pool)
        .await;

    let row = match row {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    if row.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    }

    let fund_id: String = row.unwrap().get("id");
    let days: i64 = q
        .get("days")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(100)
        .max(0);

    let rows = match sqlx::query(
        r#"
        SELECT
          source_name,
          CAST(estimate_date AS TEXT) as estimate_date,
          CAST(error_rate AS REAL) as error_rate
        FROM estimate_accuracy
        WHERE CAST(fund_id AS TEXT) = $1 AND error_rate IS NOT NULL
        ORDER BY estimate_date DESC
        LIMIT $2
        "#,
    )
    .bind(fund_id)
    .bind(days)
    .fetch_all(pool)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    #[derive(Default)]
    struct Accum {
        total: f64,
        count: i64,
        records: Vec<serde_json::Value>,
    }

    let mut by_source: std::collections::BTreeMap<String, Accum> =
        std::collections::BTreeMap::new();
    for row in rows {
        let source_name: String = row.get("source_name");
        let estimate_date: String = row.get("estimate_date");
        let error_rate: f64 = row.get("error_rate");

        let entry = by_source.entry(source_name).or_default();
        entry.total += error_rate;
        entry.count += 1;
        entry.records.push(json!({
          "date": estimate_date,
          "error_rate": error_rate
        }));
    }

    let mut out: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for (source_name, acc) in by_source {
        let avg = if acc.count > 0 {
            acc.total / (acc.count as f64)
        } else {
            0.0
        };
        out.insert(
            source_name,
            json!({
              "avg_error_rate": avg,
              "record_count": acc.count,
              "records": acc.records
            }),
        );
    }

    (StatusCode::OK, Json(serde_json::Value::Object(out))).into_response()
}

#[derive(Debug, Deserialize)]
pub struct BatchEstimateRequest {
    pub fund_codes: Option<Vec<String>>,
    pub source: Option<String>,
}

pub async fn batch_estimate(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<BatchEstimateRequest>,
) -> axum::response::Response {
    let fund_codes = body.fund_codes.unwrap_or_default();
    if fund_codes.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "缺少 fund_codes 参数" })),
        )
            .into_response();
    }

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

    let source_name_raw = body.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let Some(source_name) = sources::normalize_source_name(source_name_raw) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("数据源 {source_name_raw} 不存在") })),
        )
            .into_response();
    };
    let cache_enabled = source_name == sources::SOURCE_TIANTIAN;

    #[derive(Clone)]
    struct FundDbRow {
        id: String,
        fund_name: String,
        estimate_nav: Option<String>,
        estimate_growth: Option<String>,
        estimate_time: Option<String>,
        latest_nav: Option<String>,
        latest_nav_date: Option<String>,
    }

    let mut sql = String::from(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          fund_code,
          fund_name,
          CAST(estimate_nav AS TEXT) as estimate_nav,
          CAST(estimate_growth AS TEXT) as estimate_growth,
          CAST(estimate_time AS TEXT) as estimate_time,
          CAST(latest_nav AS TEXT) as latest_nav,
          CAST(latest_nav_date AS TEXT) as latest_nav_date
        FROM fund
        WHERE fund_code IN (
        "#,
    );
    for (i, _) in fund_codes.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("${}", i + 1));
    }
    sql.push_str(")\n");

    let mut q = sqlx::query(&sql);
    for code in &fund_codes {
        q = q.bind(code);
    }

    let rows = match q.fetch_all(pool).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut db_map: std::collections::HashMap<String, FundDbRow> = std::collections::HashMap::new();
    for row in rows {
        let code: String = row.get("fund_code");
        db_map.insert(
            code,
            FundDbRow {
                id: row.get("id"),
                fund_name: row.get("fund_name"),
                estimate_nav: row.get("estimate_nav"),
                estimate_growth: row.get("estimate_growth"),
                estimate_time: row.get("estimate_time"),
                latest_nav: row.get("latest_nav"),
                latest_nav_date: row.get("latest_nav_date"),
            },
        );
    }

    let ttl_minutes = state.config().get_i64("estimate_cache_ttl", 5).max(0);
    let ttl = Duration::minutes(ttl_minutes);
    let now = Utc::now();

    let mut results: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    let mut need_fetch: Vec<(String, FundDbRow)> = Vec::new();

    for code in &fund_codes {
        let Some(row) = db_map.get(code).cloned() else {
            results.insert(code.clone(), json!({ "error": "基金不存在" }));
            continue;
        };

        let cache_valid = cache_enabled
            && match (
                &row.estimate_nav,
                row.estimate_time
                    .as_deref()
                    .and_then(crate::dbfmt::parse_datetime_utc),
            ) {
                (Some(_), Some(ts)) => now - ts < ttl,
                _ => false,
            };

        if cache_valid {
            results.insert(
                code.clone(),
                json!({
                  "fund_code": code,
                  "fund_name": row.fund_name,
                  "estimate_nav": row.estimate_nav,
                  "estimate_growth": row.estimate_growth,
                  "estimate_time": row.estimate_time.as_deref().map(crate::dbfmt::datetime_to_rfc3339),
                  "latest_nav": row.latest_nav,
                  "latest_nav_date": row.latest_nav_date,
                  "from_cache": true
                }),
            );
        } else {
            need_fetch.push((code.clone(), row));
        }
    }

    if !need_fetch.is_empty() {
        let client = match eastmoney::build_client() {
            Ok(c) => c,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("创建 HTTP 客户端失败: {e}") })),
                )
                    .into_response();
            }
        };

        let sem = Arc::new(Semaphore::new(5));
        let mut set: JoinSet<(String, serde_json::Value)> = JoinSet::new();

        for (code, row) in need_fetch {
            let client = client.clone();
            let sem = sem.clone();
            let pool = pool.clone();
            set.spawn(async move {
                let _permit = sem.acquire_owned().await.expect("semaphore");
                match source_name {
                    sources::SOURCE_TIANTIAN => match eastmoney::fetch_estimate(&client, &code).await {
                        Ok(Some(data)) => {
                            let now = Utc::now();
                            let _ = sqlx::query(
                                r#"
                                UPDATE fund
                                SET estimate_nav = CAST($2 AS NUMERIC),
                                    estimate_growth = CAST($3 AS NUMERIC),
                                    estimate_time = CAST($4 AS TIMESTAMPTZ),
                                    updated_at = CURRENT_TIMESTAMP
                            WHERE CAST(id AS TEXT) = $1
                                "#,
                            )
                            .bind(&row.id)
                            .bind(data.estimate_nav.to_string())
                            .bind(data.estimate_growth.to_string())
                            .bind(format_dt(now))
                            .execute(&pool)
                            .await;

                            let _ = upsert_estimate_accuracy(
                                &pool,
                                sources::SOURCE_TIANTIAN,
                                &row.id,
                                data.estimate_time.date().to_string(),
                                data.estimate_nav,
                            )
                            .await;

                            (
                                code.clone(),
                                json!({
                                  "fund_code": code,
                                  "fund_name": row.fund_name,
                                  "estimate_nav": data.estimate_nav.to_string(),
                                  "estimate_growth": data.estimate_growth.to_string(),
                                  "estimate_time": format_dt(now),
                                  "latest_nav": row.latest_nav,
                                  "latest_nav_date": row.latest_nav_date,
                                  "from_cache": false
                                }),
                            )
                        }
                        Ok(None) => (
                            code.clone(),
                            json!({
                              "fund_code": code,
                              "error": "获取估值失败: 返回空"
                            }),
                        ),
                        Err(e) => (
                            code.clone(),
                            json!({
                              "fund_code": code,
                              "error": format!("获取估值失败: {e}")
                            }),
                        ),
                    },
                    sources::SOURCE_DANJUAN => match sources::danjuan::fetch_latest_row(&client, &code).await {
                        Ok(Some(latest)) => {
                            let _ = upsert_estimate_accuracy(
                                &pool,
                                sources::SOURCE_DANJUAN,
                                &row.id,
                                latest.nav_date.to_string(),
                                latest.unit_nav,
                            )
                            .await;

                            (
                                code.clone(),
                                json!({
                                  "fund_code": code,
                                  "fund_name": row.fund_name,
                                  "estimate_nav": latest.unit_nav.to_string(),
                                  "estimate_growth": latest.daily_growth.unwrap_or(Decimal::ZERO).to_string(),
                                  "estimate_time": format!("{}T15:00:00", latest.nav_date),
                                  "latest_nav": row.latest_nav,
                                  "latest_nav_date": row.latest_nav_date,
                                  "from_cache": false
                                }),
                            )
                        }
                        Ok(None) => (
                            code.clone(),
                            json!({
                              "fund_code": code,
                              "error": "获取估值失败: 返回空"
                            }),
                        ),
                        Err(e) => (
                            code.clone(),
                            json!({
                              "fund_code": code,
                              "error": format!("获取估值失败: {e}")
                            }),
                        ),
                    },
                    sources::SOURCE_THS => {
                        let series = match sources::ths::fetch_nav_series(&client, &code).await {
                            Ok(v) => v,
                            Err(e) => {
                                return (
                                    code.clone(),
                                    json!({
                                      "fund_code": code,
                                      "error": format!("获取估值失败: {e}")
                                    }),
                                );
                            }
                        };
                        if series.is_empty() {
                            return (
                                code.clone(),
                                json!({
                                  "fund_code": code,
                                  "error": "获取估值失败: 返回空"
                                }),
                            );
                        }

                        let mut latest = &series[0];
                        for r in &series[1..] {
                            if r.nav_date > latest.nav_date {
                                latest = r;
                            }
                        }
                        let mut prev: Option<&eastmoney::NavRow> = None;
                        for r in &series {
                            if r.nav_date < latest.nav_date {
                                prev = match prev {
                                    None => Some(r),
                                    Some(p) => {
                                        if r.nav_date > p.nav_date {
                                            Some(r)
                                        } else {
                                            Some(p)
                                        }
                                    }
                                };
                            }
                        }
                        let growth = prev
                            .and_then(|p| {
                                if p.unit_nav > Decimal::ZERO {
                                    Some(((latest.unit_nav - p.unit_nav) / p.unit_nav) * Decimal::from(100))
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(Decimal::ZERO)
                            .round_dp(4);

                        let _ = upsert_estimate_accuracy(
                            &pool,
                            sources::SOURCE_THS,
                            &row.id,
                            latest.nav_date.to_string(),
                            latest.unit_nav,
                        )
                        .await;

                        (
                            code.clone(),
                            json!({
                              "fund_code": code,
                              "fund_name": row.fund_name,
                              "estimate_nav": latest.unit_nav.to_string(),
                              "estimate_growth": growth.to_string(),
                              "estimate_time": format!("{}T15:00:00", latest.nav_date),
                              "latest_nav": row.latest_nav,
                              "latest_nav_date": row.latest_nav_date,
                              "from_cache": false
                            }),
                        )
                    }
                    _ => (
                        code.clone(),
                        json!({
                          "fund_code": code,
                          "error": format!("数据源 {source_name} 不存在")
                        }),
                    ),
                }
            });
        }

        while let Some(res) = set.join_next().await {
            if let Ok((code, value)) = res {
                results.insert(code, value);
            }
        }
    }

    (StatusCode::OK, Json(serde_json::Value::Object(results))).into_response()
}

#[derive(Debug, Deserialize)]
pub struct BatchUpdateNavRequest {
    pub fund_codes: Option<Vec<String>>,
    pub source: Option<String>,
}

pub async fn batch_update_nav(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<BatchUpdateNavRequest>,
) -> axum::response::Response {
    let fund_codes = body.fund_codes.unwrap_or_default();
    if fund_codes.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "缺少 fund_codes 参数" })),
        )
            .into_response();
    }

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

    let source_name_raw = body.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let Some(source_name) = sources::normalize_source_name(source_name_raw) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("数据源 {source_name_raw} 不存在") })),
        )
            .into_response();
    };

    let mut sql = String::from("SELECT fund_code FROM fund WHERE fund_code IN (");
    for (i, _) in fund_codes.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("${}", i + 1));
    }
    sql.push(')');
    let mut q = sqlx::query(&sql);
    for code in &fund_codes {
        q = q.bind(code);
    }

    let rows = match q.fetch_all(pool).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let existing_codes = rows
        .into_iter()
        .map(|r| r.get::<String, _>("fund_code"))
        .collect::<Vec<_>>();
    if existing_codes.is_empty() {
        // Python 行为：只处理 DB 中存在的 fund；空库/全不存在时返回空对象
        return (StatusCode::OK, Json(json!({}))).into_response();
    }

    let client = match eastmoney::build_client() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("创建 HTTP 客户端失败: {e}") })),
            )
                .into_response();
        }
    };
    let tushare_token = state
        .config()
        .get_string("tushare_token")
        .unwrap_or_default();

    let sem = Arc::new(Semaphore::new(5));
    let mut set: JoinSet<(String, serde_json::Value)> = JoinSet::new();
    for code in existing_codes {
        let client = client.clone();
        let sem = sem.clone();
        let pool = pool.clone();
        let tushare_token = tushare_token.clone();
        set.spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore");
            let fetched = match source_name {
                sources::SOURCE_TIANTIAN => eastmoney::fetch_realtime_nav(&client, &code).await,
                sources::SOURCE_DANJUAN => {
                    match sources::danjuan::fetch_latest_row(&client, &code).await {
                        Ok(Some(row)) => Ok(Some(eastmoney::RealtimeNavData {
                            fund_code: code.clone(),
                            nav: row.unit_nav,
                            nav_date: row.nav_date,
                        })),
                        Ok(None) => Ok(None),
                        Err(e) => Err(e),
                    }
                }
                sources::SOURCE_THS => sources::ths::fetch_realtime_nav(&client, &code).await,
                sources::SOURCE_TUSHARE => {
                    if tushare_token.trim().is_empty() {
                        Err("tushare token 未配置（请在“设置”页面填写）".to_string())
                    } else {
                        sources::tushare::fetch_realtime_nav(&client, &tushare_token, &code).await
                    }
                }
                _ => Err(format!("数据源 {source_name} 不存在")),
            };

            match fetched {
                Ok(Some(data)) => {
                    let _ = sqlx::query(
                        r#"
                        UPDATE fund
                        SET latest_nav = CAST($2 AS NUMERIC),
                            latest_nav_date = CAST($3 AS DATE),
                            updated_at = CURRENT_TIMESTAMP
                        WHERE fund_code = $1
                        "#,
                    )
                    .bind(&code)
                    .bind(data.nav.to_string())
                    .bind(data.nav_date.to_string())
                    .execute(&pool)
                    .await;

                    (
                        code.clone(),
                        json!({
                          "fund_code": code,
                          "latest_nav": data.nav.to_string(),
                          "latest_nav_date": data.nav_date.to_string()
                        }),
                    )
                }
                Ok(None) => (
                    code.clone(),
                    json!({
                      "fund_code": code,
                      "error": "获取净值失败: 返回空"
                    }),
                ),
                Err(e) => (
                    code.clone(),
                    json!({
                      "fund_code": code,
                      "error": format!("获取净值失败: {e}")
                    }),
                ),
            }
        });
    }

    let mut results: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    while let Some(res) = set.join_next().await {
        if let Ok((code, value)) = res {
            results.insert(code, value);
        }
    }

    (StatusCode::OK, Json(serde_json::Value::Object(results))).into_response()
}

#[derive(Debug, Deserialize)]
pub struct QueryNavRequest {
    pub fund_code: String,
    pub operation_date: String,
    pub before_15: bool,
    pub source: Option<String>,
}

pub async fn query_nav(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<QueryNavRequest>,
) -> axum::response::Response {
    let operation_date = match NaiveDate::parse_from_str(body.operation_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "operation_date": ["无效日期格式"] })),
            )
                .into_response();
        }
    };

    let today = Utc::now().date_naive();
    if operation_date > today {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "operation_date": ["操作日期不能是未来"] })),
        )
            .into_response();
    }

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

    // 仅覆盖空库/不存在 fund 的契约：与 Django get_object_or_404 对齐
    let row = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          fund_name,
          CAST(latest_nav AS TEXT) as latest_nav,
          CAST(latest_nav_date AS TEXT) as latest_nav_date
        FROM fund
        WHERE fund_code = $1
        "#,
    )
    .bind(body.fund_code.trim())
    .fetch_optional(pool)
    .await;

    let row = match row {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    if row.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    }

    let row = row.unwrap();
    let fund_id: String = row.get("id");
    let fund_name: String = row.get("fund_name");
    let latest_nav: Option<String> = row.get("latest_nav");
    let latest_nav_date: Option<String> = row.get("latest_nav_date");

    let source_name_raw = body.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let Some(source_name) = sources::normalize_source_name(source_name_raw) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("数据源 {source_name_raw} 不存在") })),
        )
            .into_response();
    };

    let query_date = if body.before_15 {
        // 15:00 前操作：查询 T-1 的最近交易日
        let d = operation_date.pred_opt().unwrap_or(operation_date);
        get_last_trading_day(d)
    } else {
        // 15:00 后操作：查询 T 的最近交易日
        get_last_trading_day(operation_date)
    };

    let history_row = sqlx::query(
        r#"
        SELECT unit_nav::text as unit_nav, nav_date::text as nav_date
        FROM fund_nav_history
        WHERE source_name = $1
          AND CAST(fund_id AS TEXT) = $2
          AND nav_date = CAST($3 AS DATE)
        LIMIT 1
        "#,
    )
    .bind(source_name)
    .bind(&fund_id)
    .bind(query_date.to_string())
    .fetch_optional(pool)
    .await;

    match history_row {
        Ok(Some(r)) => {
            return (
                StatusCode::OK,
                Json(json!({
                  "fund_code": body.fund_code.trim(),
                  "fund_name": fund_name,
                  "nav": r.get::<String, _>("unit_nav"),
                  "nav_date": r.get::<String, _>("nav_date"),
                  "source": "history"
                })),
            )
                .into_response();
        }
        Ok(None) => {}
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    }

    // 尝试同步单日净值（对齐 Python：缺失时同步后再查）
    let tushare_token = state
        .config()
        .get_string("tushare_token")
        .unwrap_or_default();
    let _ = sync_nav_history_for_date(
        pool,
        source_name,
        &fund_id,
        body.fund_code.trim(),
        query_date,
        &tushare_token,
    )
    .await;
    let history_row = sqlx::query(
        r#"
        SELECT unit_nav::text as unit_nav, nav_date::text as nav_date
        FROM fund_nav_history
        WHERE source_name = $1
          AND CAST(fund_id AS TEXT) = $2
          AND nav_date = CAST($3 AS DATE)
        LIMIT 1
        "#,
    )
    .bind(source_name)
    .bind(&fund_id)
    .bind(query_date.to_string())
    .fetch_optional(pool)
    .await;

    match history_row {
        Ok(Some(r)) => {
            return (
                StatusCode::OK,
                Json(json!({
                  "fund_code": body.fund_code.trim(),
                  "fund_name": fund_name,
                  "nav": r.get::<String, _>("unit_nav"),
                  "nav_date": r.get::<String, _>("nav_date"),
                  "source": "synced"
                })),
            )
                .into_response();
        }
        Ok(None) => {}
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    }

    // fallback: Fund.latest_nav
    if let Some(nav) = latest_nav {
        return (
            StatusCode::OK,
            Json(json!({
              "fund_code": body.fund_code.trim(),
              "fund_name": fund_name,
              "nav": nav,
              "nav_date": latest_nav_date,
              "source": "latest"
            })),
        )
            .into_response();
    }

    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "净值数据未找到" })),
    )
        .into_response()
}

pub async fn sync(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let user_id_i64 = match user_id.parse::<i64>() {
        Ok(v) => v,
        Err(_) => return auth::invalid_token_response(),
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

    // Django IsAdminUser：要求 is_staff（不要求 is_superuser）
    let is_admin = match sqlx::query("SELECT is_staff FROM auth_user WHERE id = $1")
        .bind(user_id_i64)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(row)) => row.get::<bool, _>("is_staff"),
        _ => false,
    };

    if !is_admin {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "detail": "You do not have permission to perform this action." })),
        )
            .into_response();
    }

    let client = match eastmoney::build_client() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("创建 HTTP 客户端失败: {e}") })),
            )
                .into_response();
        }
    };

    let funds = match eastmoney::fetch_fund_list(&client).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e })),
            )
                .into_response();
        }
    };

    let mut created: i64 = 0;
    let mut updated: i64 = 0;

    for f in &funds {
        let existed = match sqlx::query("SELECT 1 FROM fund WHERE fund_code = $1")
            .bind(&f.fund_code)
            .fetch_optional(pool)
            .await
        {
            Ok(v) => v.is_some(),
            Err(e) => {
                tracing::error!(error = %e, "funds.sync exists check failed");
                continue;
            }
        };

        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
            VALUES (CAST($1 AS uuid), $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT (fund_code) DO UPDATE
              SET fund_name = EXCLUDED.fund_name,
                  fund_type = EXCLUDED.fund_type,
                  updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&f.fund_code)
        .bind(&f.fund_name)
        .bind(&f.fund_type)
        .execute(pool)
        .await
        {
            tracing::error!(error = %e, "funds.sync upsert failed");
            continue;
        }

        if existed {
            updated += 1;
        } else {
            created += 1;
        }
    }

    (
        StatusCode::OK,
        Json(json!({
          "created": created,
          "updated": updated,
          "total": funds.len()
        })),
    )
        .into_response()
}

fn is_trading_day(d: NaiveDate) -> bool {
    // 近似实现：周一至周五视为交易日。
    // Python 版本使用 chinese_calendar 会考虑法定节假日与调休；若需要 100% 对齐，可在此处替换实现。
    let wd = d.weekday().num_days_from_monday();
    wd < 5
}

fn get_last_trading_day(mut d: NaiveDate) -> NaiveDate {
    let original = d;
    for _ in 0..30 {
        if is_trading_day(d) {
            return d;
        }
        d = d.pred_opt().unwrap_or(d);
    }
    original
}

async fn sync_nav_history_for_date(
    pool: &sqlx::AnyPool,
    source_name: &str,
    fund_id: &str,
    fund_code: &str,
    nav_date: NaiveDate,
    tushare_token: &str,
) -> Result<i64, String> {
    let client = eastmoney::build_client()?;
    let data = match source_name {
        sources::SOURCE_TIANTIAN => {
            eastmoney::fetch_nav_history(&client, fund_code, Some(nav_date), Some(nav_date)).await?
        }
        sources::SOURCE_DANJUAN => {
            sources::danjuan::fetch_nav_history(&client, fund_code, Some(nav_date), Some(nav_date))
                .await?
        }
        sources::SOURCE_THS => {
            let all = sources::ths::fetch_nav_series(&client, fund_code).await?;
            all.into_iter()
                .filter(|r| r.nav_date == nav_date)
                .collect::<Vec<_>>()
        }
        sources::SOURCE_TUSHARE => {
            if tushare_token.trim().is_empty() {
                return Err("tushare token 未配置（请在“设置”页面填写）".to_string());
            }
            sources::tushare::fetch_nav_history(
                &client,
                tushare_token,
                fund_code,
                Some(nav_date),
                Some(nav_date),
            )
            .await?
        }
        _ => return Err(format!("数据源 {source_name} 不存在")),
    };
    if data.is_empty() {
        return Ok(0);
    }

    let mut inserted_count: i64 = 0;
    for item in data {
        let exists = sqlx::query(
            r#"
            SELECT 1
            FROM fund_nav_history
            WHERE source_name = $1
              AND CAST(fund_id AS TEXT) = $2
              AND nav_date = CAST($3 AS DATE)
            "#,
        )
        .bind(source_name)
        .bind(fund_id)
        .bind(item.nav_date.to_string())
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .is_some();

        sqlx::query(
            r#"
            INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, accumulated_nav, daily_growth, created_at, updated_at)
            VALUES (
              CAST($1 AS uuid),
              $2,
              CAST($3 AS uuid),
              CAST($4 AS DATE),
              CAST($5 AS NUMERIC),
              CAST($6 AS NUMERIC),
              CAST($7 AS NUMERIC),
              CURRENT_TIMESTAMP,
              CURRENT_TIMESTAMP
            )
            ON CONFLICT (source_name, fund_id, nav_date) DO UPDATE
              SET unit_nav = CAST(EXCLUDED.unit_nav AS NUMERIC),
                  accumulated_nav = CAST(EXCLUDED.accumulated_nav AS NUMERIC),
                  daily_growth = CAST(EXCLUDED.daily_growth AS NUMERIC),
                  updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(source_name)
        .bind(fund_id)
        .bind(item.nav_date.to_string())
        .bind(item.unit_nav.to_string())
        .bind(item.accumulated_nav.map(|v| v.to_string()))
        .bind(item.daily_growth.map(|v| v.to_string()))
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        if !exists {
            inserted_count += 1;
        }
    }

    Ok(inserted_count)
}

fn format_dt(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::AutoSi, false)
}

async fn upsert_estimate_accuracy(
    pool: &sqlx::AnyPool,
    source_name: &str,
    fund_id: &str,
    estimate_date: String,
    estimate_nav: Decimal,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO estimate_accuracy (id, source_name, fund_id, estimate_date, estimate_nav, created_at)
        VALUES (CAST($1 AS uuid), $2, CAST($3 AS uuid), CAST($4 AS DATE), CAST($5 AS NUMERIC), CURRENT_TIMESTAMP)
        ON CONFLICT (source_name, fund_id, estimate_date) DO UPDATE
          SET estimate_nav = CAST(EXCLUDED.estimate_nav AS NUMERIC)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(source_name)
    .bind(fund_id)
    .bind(estimate_date)
    .bind(estimate_nav.to_string())
    .execute(pool)
    .await
    .map(|_| ())
}
