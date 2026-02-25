use uuid::Uuid;

#[test]
fn ensure_sqlite_db_file_creates_missing_file() {
    let mut dir = std::env::temp_dir();
    dir.push(format!("fundval-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let db_path = dir.join("data").join("fundval.sqlite");
    assert!(!db_path.exists());

    let url = format!("sqlite:{}", db_path.to_string_lossy());
    api::db::ensure_sqlite_db_file(&url).expect("ensure sqlite db file");

    assert!(db_path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensure_sqlite_db_file_ignores_memory() {
    api::db::ensure_sqlite_db_file("sqlite::memory:").expect("memory url ok");
    api::db::ensure_sqlite_db_file("sqlite://:memory:").expect("memory url ok");
}
