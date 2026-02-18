use std::net::SocketAddr;

use api::{app, state::AppState};
use axum::http::HeaderValue;
use sqlx::Error as SqlxError;
use sqlx::postgres::PgPoolOptions;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::EnvFilter;
use sqlx::migrate::Migrator;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

fn is_missing_database_error(err: &SqlxError) -> bool {
    match err {
        SqlxError::Database(db_err) => db_err.code().as_deref() == Some("3D000"),
        _ => false,
    }
}

fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('\"', "\"\""))
}

fn parse_database_name(database_url: &str) -> Option<String> {
    let base = database_url.split('?').next().unwrap_or(database_url);
    let (_, db) = base.rsplit_once('/')?;
    let db = db.trim();
    if db.is_empty() {
        None
    } else {
        Some(db.to_string())
    }
}

fn build_admin_database_url(database_url: &str, admin_db: &str) -> Option<String> {
    let (base, query) = match database_url.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (database_url, None),
    };
    let (prefix, _) = base.rsplit_once('/')?;
    let mut out = format!("{prefix}/{admin_db}");
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    Some(out)
}

async fn ensure_database_exists(database_url: &str) -> Result<(), SqlxError> {
    let Some(db_name) = parse_database_name(database_url) else {
        return Ok(());
    };
    let Some(admin_url) = build_admin_database_url(database_url, "postgres") else {
        return Ok(());
    };

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await?;

    let exists = sqlx::query("SELECT 1 FROM pg_database WHERE datname = $1")
        .bind(&db_name)
        .fetch_optional(&admin_pool)
        .await?;

    if exists.is_none() {
        let create_sql = format!("CREATE DATABASE {}", quote_ident(&db_name));
        let _ = sqlx::query(&create_sql).execute(&admin_pool).await?;
    }

    Ok(())
}

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
    let pool = match database_url.as_deref() {
        None => None,
        Some(url) => match PgPoolOptions::new().max_connections(5).connect(url).await {
            Ok(pool) => Some(pool),
            Err(e) => {
                if is_missing_database_error(&e) {
                    tracing::warn!(error = %e, "database does not exist, trying to create it");
                    match ensure_database_exists(url).await {
                        Ok(()) => match PgPoolOptions::new().max_connections(5).connect(url).await {
                            Ok(pool) => Some(pool),
                            Err(e2) => {
                                tracing::warn!(error = %e2, "failed to connect database after creation");
                                None
                            }
                        },
                        Err(e2) => {
                            tracing::warn!(error = %e2, "failed to create database");
                            None
                        }
                    }
                } else {
                    tracing::warn!(error = %e, "failed to connect database");
                    None
                }
            }
        },
    };

    // 初始化配置（文件 + env 覆盖）
    let config = api::config::ConfigStore::load();

    let secret = std::env::var("SECRET_KEY").unwrap_or_else(|_| "django-insecure-dev-only".to_string());
    let jwt = api::jwt::JwtService::from_secret(&secret);

    if !config.system_initialized() {
        if let Some(key) = config.get_or_generate_bootstrap_key() {
            tracing::info!(bootstrap_key = %key, "BOOTSTRAP KEY");
        }
    }

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
    tracing::info!(%addr, "backend listening");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind listener");
    axum::serve(listener, app).await.expect("serve");
}
