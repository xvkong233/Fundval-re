use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SourceItem {
    pub name: &'static str,
}

pub async fn list(_state: axum::extract::State<AppState>) -> impl IntoResponse {
    // 对齐 Python SourceRegistry 当前注册的数据源（当前仅 eastmoney）
    let sources = vec![SourceItem { name: "eastmoney" }];
    (StatusCode::OK, Json(sources))
}

