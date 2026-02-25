#[test]
fn snapshot_score_query_casts_as_of_date_for_postgres() {
    let sql = api::sim::engine::snapshot_score_select_sql(true);
    assert!(
        sql.contains("as_of_date = ($2)::date"),
        "Postgres should cast $2 to date to avoid operator does not exist: date = text"
    );
}

#[test]
fn snapshot_score_query_does_not_use_postgres_cast_for_sqlite() {
    let sql = api::sim::engine::snapshot_score_select_sql(false);
    assert!(
        !sql.contains("::date"),
        "SQLite/Any flavor should not contain Postgres-only casts"
    );
}

