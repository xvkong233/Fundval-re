use axum::{body::Body, http::Request};
use tower::ServiceExt;

use api::state::AppState;

#[tokio::test]
async fn calculate_accuracy_requires_database() {
    let config = api::config::ConfigStore::load();
    config.set_system_initialized(false);
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/tiantian/accuracy/calculate/")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"date":"2026-02-17"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 503);
}
