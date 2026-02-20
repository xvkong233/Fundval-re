use api::config::ConfigStore;

#[test]
fn default_db_type_is_postgres_not_sqlite() {
    let cfg = ConfigStore::load();
    assert_eq!(cfg.get_string("db_type").as_deref(), Some("postgres"));
}

#[test]
fn default_crawl_config_is_present() {
    let cfg = ConfigStore::load();
    assert_eq!(cfg.get_bool("crawl_enabled", true), true);
    assert_eq!(cfg.get_string("crawl_source").as_deref(), Some("tiantian"));
    assert_eq!(cfg.get_i64("crawl_tick_interval_seconds", 0), 30);
    assert_eq!(cfg.get_i64("crawl_enqueue_max_jobs", 0), 200);
    assert_eq!(cfg.get_i64("crawl_run_max_jobs", 0), 20);
    assert_eq!(cfg.get_i64("crawl_per_job_delay_ms", 0), 250);
}
