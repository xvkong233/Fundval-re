use std::sync::Arc;

use serde::Serialize;
use sqlx::AnyPool;
use tokio::sync::Mutex;

use crate::config::ConfigStore;
use crate::jwt::JwtService;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<InnerState>,
}

struct InnerState {
    pub pool: Option<AnyPool>,
    pub config: ConfigStore,
    pub jwt: JwtService,
    pub sniffer_lock: Mutex<()>,
}

impl AppState {
    pub fn new(pool: Option<AnyPool>, config: ConfigStore, jwt: JwtService) -> Self {
        Self {
            inner: Arc::new(InnerState {
                pool,
                config,
                jwt,
                sniffer_lock: Mutex::new(()),
            }),
        }
    }

    pub fn pool(&self) -> Option<&AnyPool> {
        self.inner.pool.as_ref()
    }
    pub fn config(&self) -> &ConfigStore {
        &self.inner.config
    }

    pub fn jwt(&self) -> &JwtService {
        &self.inner.jwt
    }

    pub fn sniffer_lock(&self) -> &Mutex<()> {
        &self.inner.sniffer_lock
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub database: &'static str,
    pub system_initialized: bool,
}
