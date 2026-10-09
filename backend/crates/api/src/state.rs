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
    /// 路由在入队后是否 spawn 后台 worker 立即执行任务。
    /// 生产环境为 true；集成测试应设为 false，避免与测试手动驱动的
    /// run_due_task_jobs 并发抢单连接池导致死锁。
    /// 注意：不能用 `cfg!(test)` 判断，因为集成测试链接的 lib 并非以 test cfg 编译。
    pub spawn_background_workers: bool,
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
                spawn_background_workers: true,
            }),
        }
    }

    /// Builder：设置是否允许路由 spawn 后台任务 worker（测试用）。
    pub fn with_spawn_background_workers(mut self, v: bool) -> Self {
        Arc::get_mut(&mut self.inner)
            .expect("AppState::with_spawn_background_workers on shared Arc")
            .spawn_background_workers = v;
        self
    }

    pub fn spawn_background_workers(&self) -> bool {
        self.inner.spawn_background_workers
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
