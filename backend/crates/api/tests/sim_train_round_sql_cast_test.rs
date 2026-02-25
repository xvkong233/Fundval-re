#[test]
fn sim_train_round_upsert_sql_casts_run_id_for_postgres() {
    let sql = api::sim::engine::sim_train_round_upsert_sql(true);
    assert!(
        sql.contains("($1)::uuid") || sql.contains("$1::uuid"),
        "Postgres should cast $1 to uuid to avoid column run_id is uuid but expression is text"
    );
}

#[test]
fn sim_train_round_upsert_sql_does_not_use_postgres_cast_for_sqlite() {
    let sql = api::sim::engine::sim_train_round_upsert_sql(false);
    assert!(
        !sql.contains("::uuid"),
        "SQLite/Any flavor should not contain Postgres-only casts"
    );
}

