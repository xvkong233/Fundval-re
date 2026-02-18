use axum::{body::Body, http::Request};
use tower::ServiceExt;

use api::state::AppState;

#[tokio::test]
async fn sources_list_returns_builtin_canonical_names() {
    let config = api::config::ConfigStore::load();
    config.set_system_initialized(false);
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt);
    let app = api::app(state);

    let res = app
        .oneshot(Request::builder().uri("/api/sources/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let names = json
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.get("name").and_then(|x| x.as_str()).map(|s| s.to_string()))
        .collect::<Vec<_>>();

    assert!(names.contains(&"tiantian".to_string()));
    assert!(names.contains(&"danjuan".to_string()));
    assert!(names.contains(&"ths".to_string()));
}

#[tokio::test]
async fn sources_health_can_be_disabled_for_tests() {
    let config = api::config::ConfigStore::load();
    config.set_system_initialized(false);
    config.set_bool("sources_health_probe", false);
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt);
    let app = api::app(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/sources/health/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = json.as_array().unwrap();
    assert!(arr.len() >= 3);

    let by_name = arr
        .iter()
        .filter_map(|v| {
            Some((
                v.get("name")?.as_str()?.to_string(),
                v.get("error").and_then(|e| e.as_str()).unwrap_or("").to_string(),
            ))
        })
        .collect::<std::collections::HashMap<_, _>>();

    for name in ["tiantian", "danjuan", "ths"] {
        let err = by_name.get(name).expect("missing source in health");
        assert!(err.contains("禁用") || err.contains("disabled"));
    }
}

