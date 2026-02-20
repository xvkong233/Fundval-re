use std::net::SocketAddr;

use api::{app, state::AppState};
use axum::http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderValue, Method};
use sqlx::Error as SqlxError;
use sqlx::any::AnyPoolOptions;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

static MIGRATOR_POSTGRES: Migrator = sqlx::migrate!("../../migrations/postgres");
static MIGRATOR_SQLITE: Migrator = sqlx::migrate!("../../migrations/sqlite");

fn env_truthy(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes" | "y" | "on"))
}

fn is_default_insecure_secret(secret: &str) -> bool {
    secret.trim() == "django-insecure-dev-only"
}

fn is_placeholder_secret(secret: &str) -> bool {
    let s = secret.trim();
    if s.is_empty() {
        return true;
    }
    if s.len() < 32 {
        return true;
    }
    let lower = s.to_ascii_lowercase();
    lower.contains("change_me") || lower.contains("dev_only") || lower.contains("insecure")
}

fn validate_secret_key(secret: &str, debug: bool) {
    if debug {
        if is_default_insecure_secret(secret) || is_placeholder_secret(secret) {
            tracing::warn!("DEBUG=true 且 SECRET_KEY 看起来不安全；仅建议本地开发使用");
        }
        return;
    }

    if is_default_insecure_secret(secret) {
        tracing::error!(
            "SECRET_KEY 未配置（仍为默认值 django-insecure-dev-only）。请设置环境变量 SECRET_KEY 后再启动。"
        );
        std::process::exit(1);
    }

    // 生产默认不强制中断，避免破坏 quickstart；但提供开关以便部署时硬性要求。
    if env_truthy("REQUIRE_SECURE_SECRET") && is_placeholder_secret(secret) {
        tracing::error!(
            "REQUIRE_SECURE_SECRET=true 且 SECRET_KEY 看起来是占位符/过短。请使用高熵随机字符串（建议 >= 32 字符）。"
        );
        std::process::exit(1);
    }

    if is_placeholder_secret(secret) {
        tracing::warn!(
            "SECRET_KEY 看起来是占位符或长度过短；如用于生产部署，建议更换为高熵随机字符串（建议 >= 32 字符）。可设置 REQUIRE_SECURE_SECRET=true 强制校验。"
        );
    }
}

fn build_cors_layer(debug: bool) -> CorsLayer {
    let raw = std::env::var("CORS_ALLOW_ORIGINS").unwrap_or_default();
    let raw = raw.trim();

    // 默认策略：
    // - 生产：不主动开启跨域（依赖 Next.js /api 反代即可）
    // - 开发：如果未配置则放开，便于直接从浏览器请求后端
    let allow_origin = if raw.is_empty() {
        if debug {
            tracing::warn!(
                "未设置 CORS_ALLOW_ORIGINS 且 DEBUG=true：将使用宽松 CORS（仅建议本地开发）"
            );
            Any.into()
        } else {
            AllowOrigin::predicate(|_, _| false)
        }
    } else if raw == "*" {
        tracing::warn!("CORS_ALLOW_ORIGINS='*'：将允许任意来源跨域访问（不建议生产环境）");
        Any.into()
    } else {
        let origins: Vec<HeaderValue> = raw
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .filter_map(|s| match HeaderValue::from_str(s) {
                Ok(v) => Some(v),
                Err(_) => {
                    tracing::warn!(origin = s, "忽略无效的 CORS origin");
                    None
                }
            })
            .collect();

        if origins.is_empty() {
            AllowOrigin::predicate(|_, _| false)
        } else {
            AllowOrigin::list(origins)
        }
    };

    CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE, ACCEPT])
}

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

fn maybe_ensure_sqlite_parent_dir(database_url: &str) {
    // 兼容：sqlite:data.db / sqlite://data.db / sqlite:///abs/path
    // 对内存数据库与 query-only URL 直接跳过。
    let url = database_url.trim();
    if !url.starts_with("sqlite:") {
        return;
    }
    if url.starts_with("sqlite::memory:") || url.starts_with("sqlite://:memory:") {
        return;
    }

    let rest = url
        .trim_start_matches("sqlite:")
        .trim_start_matches('/')
        .split('?')
        .next()
        .unwrap_or("")
        .trim();
    if rest.is_empty() {
        return;
    }

    // 这里不尝试解析 URL 编码；仅用于“目录不存在导致无法创建 sqlite 文件”的常见场景。
    let path = std::path::PathBuf::from(rest.replace('/', std::path::MAIN_SEPARATOR_STR));
    let _ = api::db::ensure_parent_dir(&path);
}

async fn connect_any_pool(database_url: &str) -> Result<sqlx::AnyPool, SqlxError> {
    AnyPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
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

    sqlx::any::install_default_drivers();

    let (database_url, db_kind) = api::db::resolve_database_url();

    if db_kind == api::db::DatabaseKind::Sqlite {
        maybe_ensure_sqlite_parent_dir(&database_url);
    }

    let pool = match connect_any_pool(&database_url).await {
        Ok(pool) => Some(pool),
        Err(e) => {
            if db_kind == api::db::DatabaseKind::Postgres && is_missing_database_error(&e) {
                tracing::warn!(error = %e, "database does not exist, trying to create it");
                match ensure_database_exists(&database_url).await {
                    Ok(()) => match connect_any_pool(&database_url).await {
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
    };

    // 初始化配置（文件 + env 覆盖）
    let config = api::config::ConfigStore::load();

    let secret =
        std::env::var("SECRET_KEY").unwrap_or_else(|_| "django-insecure-dev-only".to_string());
    validate_secret_key(&secret, config.get_bool("debug", false));
    let jwt = api::jwt::JwtService::from_secret(&secret);

    if !config.system_initialized()
        && let Some(key) = config.get_or_generate_bootstrap_key()
    {
        tracing::info!(bootstrap_key = %key, "BOOTSTRAP KEY");
    }

    if let Some(ref pool) = pool {
        let migrator = match db_kind {
            api::db::DatabaseKind::Postgres => &MIGRATOR_POSTGRES,
            api::db::DatabaseKind::Sqlite => &MIGRATOR_SQLITE,
        };
        if let Err(e) = migrator.run(pool).await {
            tracing::warn!(error=%e, "failed to run migrations");
        }
    }

    let state = AppState::new(pool, config, jwt);

    if state.pool().is_some() {
        tokio::spawn(api::crawl::worker::background_task(state.clone()));
    }

    let cors = build_cors_layer(state.config().get_bool("debug", false));

    let app = app(state).layer(TraceLayer::new_for_http()).layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "backend listening");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind listener");
    axum::serve(listener, app).await.expect("serve");
}
