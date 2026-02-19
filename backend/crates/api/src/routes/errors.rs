use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use std::fmt::Display;

use crate::state::AppState;

pub fn internal_message(state: &AppState, err: impl Display) -> String {
    tracing::error!(error = %err, "internal error");
    if state.config().get_bool("debug", false) {
        err.to_string()
    } else {
        "服务器内部错误".to_string()
    }
}

pub fn masked_message(state: &AppState, public_message: &'static str, err: impl Display) -> String {
    tracing::error!(error = %err, "internal error");
    if state.config().get_bool("debug", false) {
        format!("{public_message}: {err}")
    } else {
        public_message.to_string()
    }
}

pub fn internal_json(state: &AppState, err: impl Display) -> Json<serde_json::Value> {
    Json(json!({ "error": internal_message(state, err) }))
}

pub fn masked_json(
    state: &AppState,
    public_message: &'static str,
    err: impl Display,
) -> Json<serde_json::Value> {
    Json(json!({ "error": masked_message(state, public_message, err) }))
}

pub fn internal_response(state: &AppState, err: impl Display) -> axum::response::Response {
    (StatusCode::INTERNAL_SERVER_ERROR, internal_json(state, err)).into_response()
}
