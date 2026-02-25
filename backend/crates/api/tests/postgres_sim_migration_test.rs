#[test]
fn postgres_sim_migration_quotes_current_date_column() {
    let sql = include_str!("../../../migrations/postgres/20260221000003_create_sim_tables.sql");

    assert!(
        sql.contains("\"current_date\""),
        "Postgres migration should quote current_date column name to avoid SQL keyword conflicts"
    );

    assert!(
        !sql.contains("\n  current_date DATE"),
        "Postgres migration should not use unquoted current_date identifier"
    );
}
