use api::config::ConfigStore;
use uuid::Uuid;

struct TempDirEnv {
    key: &'static str,
    path: std::path::PathBuf,
    old: Option<std::ffi::OsString>,
}

impl TempDirEnv {
    fn new() -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("fundval-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create temp dir");

        let key = "FUNDVAL_DATA_DIR";
        let old = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, &path);
        }

        Self { key, path, old }
    }
}

impl Drop for TempDirEnv {
    fn drop(&mut self) {
        match self.old.take() {
            Some(v) => unsafe {
                std::env::set_var(self.key, v);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn default_db_type_is_postgres_not_sqlite() {
    let _env = TempDirEnv::new();
    let cfg = ConfigStore::load();
    assert_eq!(cfg.get_string("db_type").as_deref(), Some("postgres"));
}

#[test]
fn default_crawl_config_is_present() {
    let _env = TempDirEnv::new();
    let cfg = ConfigStore::load();
    assert_eq!(cfg.get_bool("crawl_enabled", true), true);
    assert_eq!(cfg.get_string("crawl_source").as_deref(), Some("tiantian"));
    assert_eq!(cfg.get_i64("crawl_tick_interval_seconds", 0), 30);
    assert_eq!(cfg.get_i64("crawl_enqueue_max_jobs", 0), 200);
    assert_eq!(cfg.get_i64("crawl_daily_run_limit", 0), 3000);
    assert_eq!(cfg.get_i64("crawl_run_max_jobs", 0), 20);
    assert_eq!(cfg.get_i64("crawl_per_job_delay_ms", 0), 250);
    assert_eq!(cfg.get_i64("crawl_per_job_jitter_ms", 0), 200);
    assert_eq!(
        cfg.get_string("crawl_source_fallbacks").as_deref(),
        Some("danjuan,ths")
    );
}
