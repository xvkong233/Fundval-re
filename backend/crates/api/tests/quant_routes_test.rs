use axum::{body::Body, http::Request};
use tower::ServiceExt;

use api::state::AppState;

fn authed_req(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn quant_health_requires_auth() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(Request::builder().uri("/api/quant/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn quant_health_returns_502_when_service_unreachable() {
    let config = api::config::ConfigStore::load();
    config.set_string("quant_service_url", Some("http://127.0.0.1:1".to_string()));
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let token = jwt.issue_access_token("user-1");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app.oneshot(authed_req("/api/quant/health", &token)).await.unwrap();
    assert_eq!(res.status(), 502);
}

#[tokio::test]
async fn quant_xalpha_qdiipredict_requires_auth() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/quant/xalpha/qdiipredict")
                .method("POST")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"last_value":1.0,"legs":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn quant_xalpha_backtest_requires_auth() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/quant/xalpha/backtest")
                .method("POST")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"strategy":"scheduled","series":{},"params":{}}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn quant_fund_strategies_compare_requires_auth() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/quant/fund-strategies/compare")
                .method("POST")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"strategies":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn quant_pytrader_strategies_requires_auth() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(Request::builder().uri("/api/quant/pytrader/strategies").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn quant_pytrader_backtest_requires_auth() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/quant/pytrader/backtest")
                .method("POST")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"strategy":"macd_cross","series":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn indexes_daily_requires_auth() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/indexes/daily?index_code=1.000300")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}
