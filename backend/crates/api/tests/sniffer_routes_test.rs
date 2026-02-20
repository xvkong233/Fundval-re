use axum::{body::Body, http::Request};
use tower::ServiceExt;

use api::state::AppState;

#[tokio::test]
async fn sniffer_endpoints_require_auth() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt);
    let app = api::service(state);

    for uri in ["/api/sniffer/status/", "/api/sniffer/items/"] {
        let res = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 401, "uri={uri}");
    }

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/sniffer/sync/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}
