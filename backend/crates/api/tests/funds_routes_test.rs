use axum::{body::Body, http::Request};
use tower::ServiceExt;

use api::state::AppState;

#[tokio::test]
async fn funds_list_requires_database() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/funds/?page=1&page_size=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 500);
}

#[tokio::test]
async fn funds_retrieve_requires_database() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/funds/000001/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 500);
}
