use std::sync::Arc;

use serde::Serialize;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<InnerState>,
}

struct InnerState {
    pub pool: Option<PgPool>,
    pub system_initialized: bool,
}

impl AppState {
    pub fn new(pool: Option<PgPool>, system_initialized: bool) -> Self {
        Self {
            inner: Arc::new(InnerState {
                pool,
                system_initialized,
            }),
        }
    }

    pub fn pool(&self) -> Option<&PgPool> {
        self.inner.pool.as_ref()
    }

    pub fn system_initialized(&self) -> bool {
        self.inner.system_initialized
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub database: &'static str,
    pub system_initialized: bool,
}

