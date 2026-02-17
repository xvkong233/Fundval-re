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

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SourceItem {
    pub name: &'static str,
}

pub async fn list(_state: State<AppState>) -> impl IntoResponse {
    // 对齐 Python SourceRegistry 当前注册的数据源（目前仅 eastmoney）
    let sources = vec![SourceItem { name: "eastmoney" }];
    (StatusCode::OK, Json(sources))
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
    let limit = days as i64;

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
