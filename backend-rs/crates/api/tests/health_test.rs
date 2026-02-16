use axum::{body::Body, http::Request};
use tower::ServiceExt;

use api::state::AppState;

#[tokio::test]
async fn health_returns_ok_shape() {
    let config = api::config::ConfigStore::load();
    config.set_system_initialized(false);
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt);
    let app = api::app(state);

    let res = app
        .oneshot(Request::builder().uri("/api/health/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert!(json.get("database").is_some());
    assert_eq!(json["system_initialized"], false);
}
