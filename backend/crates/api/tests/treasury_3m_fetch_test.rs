use axum::{Json, Router, routing::get};
use serde_json::json;

#[tokio::test]
async fn fetch_chinabond_3m_reads_json_and_parses() {
    let app = Router::new().route(
        "/cbweb-czb-web/czb/czbChartIndex",
        get(|| async {
            Json(json!({
              "worktime": "2026-02-14",
              "seriesData": [[0.25, 1.3428], [1.0, 1.3145]]
            }))
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let url = format!("http://{addr}/cbweb-czb-web/czb/czbChartIndex");
    let client = reqwest::Client::new();
    let got = api::rates::treasury_3m::fetch_chinabond_3m(&client, &url)
        .await
        .expect("fetch ok");

    assert_eq!(got.rate_date, "2026-02-14");
    assert!((got.rate_percent - 1.3428).abs() < 1e-9);
}

