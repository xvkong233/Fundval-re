use std::net::SocketAddr;

use api::{app, state::AppState};
use axum::http::HeaderValue;
use sqlx::postgres::PgPoolOptions;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::EnvFilter;
use sqlx::migrate::Migrator;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

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

    // 初始化配置（文件 + env 覆盖）
    let config = api::config::ConfigStore::load();

    let secret = std::env::var("SECRET_KEY").unwrap_or_else(|_| "django-insecure-dev-only".to_string());
    let jwt = api::jwt::JwtService::from_secret(&secret);

    if let Some(ref pool) = pool
        && let Err(e) = MIGRATOR.run(pool).await
    {
        tracing::warn!(error=%e, "failed to run migrations");
    }

    let state = AppState::new(pool, config, jwt);

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
