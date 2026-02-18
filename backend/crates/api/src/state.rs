use std::sync::Arc;

use serde::Serialize;
use sqlx::PgPool;

use crate::config::ConfigStore;
use crate::jwt::JwtService;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<InnerState>,
}

struct InnerState {
    pub pool: Option<PgPool>,
    pub config: ConfigStore,
    pub jwt: JwtService,
}

impl AppState {
    pub fn new(pool: Option<PgPool>, config: ConfigStore, jwt: JwtService) -> Self {
        Self {
            inner: Arc::new(InnerState {
                pool,
                config,
                jwt,
            }),
        }
    }

    pub fn pool(&self) -> Option<&PgPool> {
        self.inner.pool.as_ref()
    }
    pub fn config(&self) -> &ConfigStore {
        &self.inner.config
    }

    pub fn jwt(&self) -> &JwtService {
        &self.inner.jwt
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub database: &'static str,
    pub system_initialized: bool,
}
