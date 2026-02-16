use std::sync::Arc;

use serde::Serialize;
use sqlx::PgPool;

use crate::config::ConfigStore;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<InnerState>,
}

struct InnerState {
    pub pool: Option<PgPool>,
    pub config: ConfigStore,
}

impl AppState {
    pub fn new(pool: Option<PgPool>, config: ConfigStore) -> Self {
        Self {
            inner: Arc::new(InnerState {
                pool,
                config,
            }),
        }
    }

    pub fn pool(&self) -> Option<&PgPool> {
        self.inner.pool.as_ref()
    }
    pub fn config(&self) -> &ConfigStore {
        &self.inner.config
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub database: &'static str,
    pub system_initialized: bool,
}
