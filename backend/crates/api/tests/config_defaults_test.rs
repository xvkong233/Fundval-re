use api::config::ConfigStore;

#[test]
fn default_db_type_is_postgres_not_sqlite() {
    let cfg = ConfigStore::load();
    assert_eq!(cfg.get_string("db_type").as_deref(), Some("postgres"));
}
