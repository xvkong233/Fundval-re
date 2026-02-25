use std::sync::Arc;

use serde::Serialize;
use sqlx::AnyPool;
use tokio::sync::{Mutex, Notify};

use crate::config::ConfigStore;
use crate::db::DatabaseKind;
use crate::jwt::JwtService;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<InnerState>,
}

struct InnerState {
    pub pool: Option<AnyPool>,
    pub config: ConfigStore,
    pub jwt: JwtService,
    pub db_kind: DatabaseKind,
    pub sniffer_lock: Mutex<()>,
    pub crawl_lock: Mutex<()>,
    pub crawl_notify: Notify,
}

impl AppState {
    pub fn new(pool: Option<AnyPool>, config: ConfigStore, jwt: JwtService, db_kind: DatabaseKind) -> Self {
        Self {
            inner: Arc::new(InnerState {
                pool,
                config,
                jwt,
                db_kind,
                sniffer_lock: Mutex::new(()),
                crawl_lock: Mutex::new(()),
                crawl_notify: Notify::new(),
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

    pub fn db_kind(&self) -> DatabaseKind {
        self.inner.db_kind
    }

    pub fn sniffer_lock(&self) -> &Mutex<()> {
        &self.inner.sniffer_lock
    }

    pub fn crawl_lock(&self) -> &Mutex<()> {
        &self.inner.crawl_lock
    }

    pub fn crawl_notify(&self) -> &Notify {
        &self.inner.crawl_notify
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub database: &'static str,
    pub system_initialized: bool,
}
