use std::net::SocketAddr;

use api::{app, state::AppState};
use axum::http::HeaderValue;
use sqlx::postgres::PgPoolOptions;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8001);

    let database_url = std::env::var("DATABASE_URL").ok();
    let pool = match database_url {
        None => None,
        Some(url) => PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .ok(),
    };

    let system_initialized = std::env::var("SYSTEM_INITIALIZED")
        .ok()
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(false);

    let state = AppState::new(pool, system_initialized);

    let cors = CorsLayer::new()
        .allow_origin(HeaderValue::from_static("*"))
        .allow_headers(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any);

    let app = app(state).layer(TraceLayer::new_for_http()).layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "backend-rs listening");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind listener");
    axum::serve(listener, app).await.expect("serve");
}
