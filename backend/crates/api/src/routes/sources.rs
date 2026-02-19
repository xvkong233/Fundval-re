use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;
use tokio::{sync::Semaphore, task::JoinSet};
use uuid::Uuid;

use crate::eastmoney;
use crate::accuracy;
use crate::sources;
use crate::state::AppState;
use sqlx::Row;

fn builtin_sources() -> Vec<&'static str> {
    sources::BUILTIN_SOURCES.to_vec()
}

#[derive(Debug, Serialize)]
pub struct SourceItem {
    pub name: String,
}

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    // 目标：返回“系统已知的数据源列表”。
    // - builtin：当前实现支持的数据源（eastmoney）
    // - db：如果数据库已连接，合并 estimate_accuracy 中出现过的 source_name（便于展示历史/扩展数据源）
    let mut names: Vec<String> = builtin_sources().into_iter().map(|s| s.to_string()).collect();

    if let Some(pool) = state.pool() {
        // 这张表在 migrations 中创建；若尚未 migrate 则忽略错误，回退 builtin。
        if let Ok(rows) = sqlx::query("SELECT DISTINCT source_name FROM estimate_accuracy")
            .fetch_all(pool)
            .await
        {
            for row in rows {
                let name = row.get::<String, _>("source_name");
                let name = name.trim();
                if name.is_empty() {
                    continue;
                }
                if let Some(canonical) = sources::normalize_source_name(name) {
                    names.push(canonical.to_string());
                } else {
                    names.push(name.to_string());
                }
            }
        }
    }

    names.sort();
    names.dedup();

    (
        StatusCode::OK,
        Json(names.into_iter().map(|name| SourceItem { name }).collect::<Vec<_>>()),
    )
}

#[derive(Debug, Serialize)]
pub struct SourceHealthItem {
    pub name: String,
    pub ok: bool,
    pub latency_ms: Option<u128>,
    pub error: Option<String>,
}

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    // 健康度用于“运维/可观测性”页面：主要衡量数据源（上游）是否可访问、响应是否可解析。
    // 注意：这里不依赖数据库；仅做上游连通性探测。
    // 这里复用 list 的“合并逻辑”，确保能展示出库里出现过的 source_name。
    let mut names: Vec<String> = builtin_sources().into_iter().map(|s| s.to_string()).collect();
    if let Some(pool) = state.pool() {
        if let Ok(rows) = sqlx::query("SELECT DISTINCT source_name FROM estimate_accuracy")
            .fetch_all(pool)
            .await
        {
            for row in rows {
                let name = row.get::<String, _>("source_name");
                let name = name.trim();
                if name.is_empty() {
                    continue;
                }
                if let Some(canonical) = sources::normalize_source_name(name) {
                    names.push(canonical.to_string());
                } else {
                    names.push(name.to_string());
                }
            }
        }
    }
    names.sort();
    names.dedup();

    let mut result: Vec<SourceHealthItem> = Vec::with_capacity(names.len());

    if !state.config().get_bool("sources_health_probe", true) {
        for name in names {
            result.push(SourceHealthItem {
                name,
                ok: false,
                latency_ms: None,
                error: Some("健康探测已禁用".to_string()),
            });
        }
        return (StatusCode::OK, Json(result));
    }

    for name in names {
        if name == "tiantian" {
            let start = Instant::now();
            let check = async {
                let client = eastmoney::build_client()?;
                let fund_code = "161725";

                // 真实环境下某些时间段 dwjz/jzrq 可能为空，但 gsz/gztime 仍可用；这里任一可用即判定健康。
                if let Ok(Some(_)) = eastmoney::fetch_estimate(&client, fund_code).await {
                    return Ok::<(), String>(());
                }
                match eastmoney::fetch_realtime_nav(&client, fund_code).await? {
                    Some(_) => Ok::<(), String>(()),
                    None => Err("上游返回为空或解析失败".to_string()),
                }
            }
            .await;

            match check {
                Ok(()) => result.push(SourceHealthItem {
                    name,
                    ok: true,
                    latency_ms: Some(start.elapsed().as_millis()),
                    error: None,
                }),
                Err(e) => result.push(SourceHealthItem {
                    name,
                    ok: false,
                    latency_ms: Some(start.elapsed().as_millis()),
                    error: Some(e),
                }),
            }
        } else if name == "danjuan" {
            let start = Instant::now();
            let check = async {
                let client = eastmoney::build_client()?;
                let fund_code = "161725";
                match sources::danjuan::fetch_latest_row(&client, fund_code).await? {
                    Some(_) => Ok::<(), String>(()),
                    None => Err("上游返回为空或解析失败".to_string()),
                }
            }
            .await;

            match check {
                Ok(()) => result.push(SourceHealthItem {
                    name,
                    ok: true,
                    latency_ms: Some(start.elapsed().as_millis()),
                    error: None,
                }),
                Err(e) => result.push(SourceHealthItem {
                    name,
                    ok: false,
                    latency_ms: Some(start.elapsed().as_millis()),
                    error: Some(e),
                }),
            }
        } else if name == "ths" {
            let start = Instant::now();
            let check = async {
                let client = eastmoney::build_client()?;
                let fund_code = "161725";
                match sources::ths::fetch_realtime_nav(&client, fund_code).await? {
                    Some(_) => Ok::<(), String>(()),
                    None => Err("上游返回为空或解析失败".to_string()),
                }
            }
            .await;

            match check {
                Ok(()) => result.push(SourceHealthItem {
                    name,
                    ok: true,
                    latency_ms: Some(start.elapsed().as_millis()),
                    error: None,
                }),
                Err(e) => result.push(SourceHealthItem {
                    name,
                    ok: false,
                    latency_ms: Some(start.elapsed().as_millis()),
                    error: Some(e),
                }),
            }
        } else if name == "tushare" {
            let start = Instant::now();
            let check = async {
                let token = state.config().get_string("tushare_token").unwrap_or_default();
                if token.trim().is_empty() {
                    return Err("tushare token 未配置（请在“设置”页面填写）".to_string());
                }
                let client = eastmoney::build_client()?;
                let fund_code = "161725";
                match sources::tushare::fetch_realtime_nav(&client, &token, fund_code).await? {
                    Some(_) => Ok::<(), String>(()),
                    None => Err("上游返回为空或解析失败".to_string()),
                }
            }
            .await;

            match check {
                Ok(()) => result.push(SourceHealthItem {
                    name,
                    ok: true,
                    latency_ms: Some(start.elapsed().as_millis()),
                    error: None,
                }),
                Err(e) => result.push(SourceHealthItem {
                    name,
                    ok: false,
                    latency_ms: Some(start.elapsed().as_millis()),
                    error: Some(e),
                }),
            }
        } else {
            result.push(SourceHealthItem {
                name,
                ok: false,
                latency_ms: None,
                error: Some("未实现该数据源的健康探测".to_string()),
            });
        }
    }

    (StatusCode::OK, Json(result))
}

#[derive(Debug, Deserialize)]
pub struct AccuracyQuery {
    pub days: Option<i64>,
}

pub async fn accuracy(
    State(state): State<AppState>,
    Path(source_name): Path<String>,
    Query(q): Query<AccuracyQuery>,
) -> impl IntoResponse {
    let pool = match state.pool() {
        Some(pool) => pool,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "数据库未连接" })),
            );
        }
    };

    let days = q.days.unwrap_or(100).max(0);
    let limit = days;

    let rows: Vec<(Decimal,)> = match sqlx::query_as(
        r#"
        SELECT error_rate
        FROM estimate_accuracy
        WHERE source_name = $1 AND error_rate IS NOT NULL
        ORDER BY estimate_date DESC
        LIMIT $2
        "#,
    )
    .bind(&source_name)
    .bind(limit)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(error = %e, "sources.accuracy db query failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "服务器内部错误" })),
            );
        }
    };

    let record_count = rows.len() as i64;
    if record_count == 0 {
        return (
            StatusCode::OK,
            Json(json!({ "avg_error_rate": 0, "record_count": 0 })),
        );
    }

    let mut total = Decimal::ZERO;
    for (error_rate,) in rows {
        total += error_rate;
    }
    let avg = total / Decimal::from(record_count);

    // 对齐 golden：这里以 number 返回。
    (
        StatusCode::OK,
        Json(json!({ "avg_error_rate": avg.to_f64().unwrap_or(0.0), "record_count": record_count })),
    )
}

#[derive(Debug, Deserialize)]
pub struct CalculateAccuracyRequest {
    pub date: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CalculateAccuracyResponse {
    pub source: String,
    pub date: String,
    pub total: i64,
    pub success: i64,
    pub failed: i64,
}

pub async fn calculate_accuracy(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(source_name): Path<String>,
    Json(body): Json<CalculateAccuracyRequest>,
) -> axum::response::Response {
    let pool = match state.pool() {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "数据库未连接" })),
            )
                .into_response();
        }
    };

    // Django IsAdminUser：要求 is_staff
    let user_id = match crate::routes::auth::authenticate(&state, &headers) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let user_id_i64 = match user_id.parse::<i64>() {
        Ok(v) => v,
        Err(_) => return crate::routes::auth::invalid_token_response(),
    };

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

    let Some(source) = sources::normalize_source_name(&source_name) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("数据源 {source_name} 不存在") })),
        )
            .into_response();
    };

    let target_date = match body.date.as_deref() {
        None | Some("") => chrono::Utc::now().date_naive() - chrono::Duration::days(1),
        Some(v) => match chrono::NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "无效日期格式" })),
                )
                    .into_response();
            }
        },
    };

    let rows = match sqlx::query(
        r#"
        SELECT ea.id, ea.fund_id, ea.estimate_nav, f.fund_code
        FROM estimate_accuracy ea
        JOIN fund f ON f.id = ea.fund_id
        WHERE ea.source_name = $1 AND ea.estimate_date = $2 AND ea.actual_nav IS NULL
        "#,
    )
    .bind(source)
    .bind(target_date)
    .fetch_all(pool)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "sources.calculate_accuracy db query failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "服务器内部错误" })),
            )
                .into_response();
        }
    };

    let total = rows.len() as i64;
    if total == 0 {
        return (
            StatusCode::OK,
            Json(CalculateAccuracyResponse {
                source: source.to_string(),
                date: target_date.to_string(),
                total: 0,
                success: 0,
                failed: 0,
            }),
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
    let tushare_token = state.config().get_string("tushare_token").unwrap_or_default();

    #[derive(Clone)]
    struct WorkItem {
        id: Uuid,
        fund_code: String,
        estimate_nav: Decimal,
    }

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(WorkItem {
            id: row.get("id"),
            fund_code: row.get("fund_code"),
            estimate_nav: row.get("estimate_nav"),
        });
    }

    let sem = std::sync::Arc::new(Semaphore::new(5));
    let mut set: JoinSet<Result<(), String>> = JoinSet::new();
    for item in items {
        let sem = sem.clone();
        let pool = pool.clone();
        let client = client.clone();
        let tushare_token = tushare_token.clone();
        set.spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore");

            let actual_nav: Decimal = match source {
                sources::SOURCE_TIANTIAN => match eastmoney::fetch_realtime_nav(&client, &item.fund_code).await {
                    Ok(Some(v)) => {
                        if v.nav_date != target_date {
                            return Err(format!(
                                "实际净值日期不匹配: got {} expect {}",
                                v.nav_date, target_date
                            ));
                        }
                        v.nav
                    }
                    Ok(None) => return Err("上游返回为空或解析失败".to_string()),
                    Err(e) => return Err(e),
                },
                sources::SOURCE_DANJUAN => {
                    let rows = sources::danjuan::fetch_nav_history(&client, &item.fund_code, Some(target_date), Some(target_date)).await?;
                    let Some(r) = rows.into_iter().find(|r| r.nav_date == target_date) else {
                        return Err("未找到该日期的净值".to_string());
                    };
                    r.unit_nav
                }
                sources::SOURCE_THS => {
                    let rows = sources::ths::fetch_nav_series(&client, &item.fund_code).await?;
                    let Some(r) = rows.into_iter().find(|r| r.nav_date == target_date) else {
                        return Err("未找到该日期的净值".to_string());
                    };
                    r.unit_nav
                }
                sources::SOURCE_TUSHARE => {
                    if tushare_token.trim().is_empty() {
                        return Err("tushare token 未配置（请在“设置”页面填写）".to_string());
                    }
                    let rows = sources::tushare::fetch_nav_history(&client, &tushare_token, &item.fund_code, Some(target_date), Some(target_date)).await?;
                    let Some(r) = rows.into_iter().find(|r| r.nav_date == target_date) else {
                        return Err("未找到该日期的净值".to_string());
                    };
                    r.unit_nav
                }
                _ => return Err(format!("数据源 {source} 不存在")),
            };

            let Some(error_rate) = accuracy::compute_error_rate(item.estimate_nav, actual_nav) else {
                return Err("actual_nav 无效".to_string());
            };

            sqlx::query(
                r#"
                UPDATE estimate_accuracy
                SET actual_nav = $2,
                    error_rate = $3
                WHERE id = $1
                "#,
            )
            .bind(item.id)
            .bind(actual_nav)
            .bind(error_rate)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;

            Ok(())
        });
    }

    let mut success = 0i64;
    let mut failed = 0i64;
    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(())) => success += 1,
            _ => failed += 1,
        }
    }

    (
        StatusCode::OK,
        Json(CalculateAccuracyResponse {
            source: source.to_string(),
            date: target_date.to_string(),
            total,
            success,
            failed,
        }),
    )
        .into_response()
}
