use axum::{body::Body, http::Request};
use tower::ServiceExt;

use api::state::AppState;

#[tokio::test]
async fn normalize_path_trims_multiple_trailing_slashes() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/health//")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
}
