use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RunRow {
    pub id: String,
    pub user_id: i64,
    pub mode: String,
    pub name: String,
    pub source_name: String,
    pub fund_codes: Vec<String>,
    pub strategy: String,
    pub strategy_params_json: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub current_date: Option<NaiveDate>,
    pub calendar: Vec<NaiveDate>,
    pub initial_cash: Decimal,
    pub cash_available: Decimal,
    pub cash_frozen: Decimal,
    pub buy_fee_rate: f64,
    pub sell_fee_rate: f64,
    pub settlement_days: i64,
    pub status: String,
}

fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| e.to_string())
}

fn parse_decimal(s: &str) -> Decimal {
    s.trim().parse::<Decimal>().unwrap_or(Decimal::ZERO)
}

pub async fn load_run(pool: &sqlx::AnyPool, run_id: &str) -> Result<Option<RunRow>, String> {
    let row = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          user_id,
          mode,
          name,
          source_name,
          fund_codes_json,
          strategy,
          strategy_params_json,
          CAST(start_date AS TEXT) as start_date,
          CAST(end_date AS TEXT) as end_date,
          CAST("current_date" AS TEXT) as current_date,
          calendar_json,
          CAST(initial_cash AS TEXT) as initial_cash,
          CAST(cash_available AS TEXT) as cash_available,
          CAST(cash_frozen AS TEXT) as cash_frozen,
          buy_fee_rate,
          sell_fee_rate,
          settlement_days,
          status
        FROM sim_run
        WHERE CAST(id AS TEXT) = $1
        "#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some(row) = row else {
        return Ok(None);
    };

    let fund_codes_json: String = row.get("fund_codes_json");
    let calendar_json: String = row.get("calendar_json");

    let fund_codes: Vec<String> =
        serde_json::from_str(&fund_codes_json).map_err(|e| e.to_string())?;
    let calendar_strs: Vec<String> =
        serde_json::from_str(&calendar_json).map_err(|e| e.to_string())?;
    let mut calendar: Vec<NaiveDate> = Vec::with_capacity(calendar_strs.len());
    for s in calendar_strs {
        calendar.push(parse_date(&s)?);
    }

    let start_date = parse_date(&row.get::<String, _>("start_date"))?;
    let end_date = parse_date(&row.get::<String, _>("end_date"))?;

    let current_date = row
        .try_get::<Option<String>, _>("current_date")
        .ok()
        .flatten()
        .map(|s| parse_date(&s))
        .transpose()?;

    Ok(Some(RunRow {
        id: row.get::<String, _>("id"),
        user_id: row.get::<i64, _>("user_id"),
        mode: row.get::<String, _>("mode"),
        name: row.get::<String, _>("name"),
        source_name: row.get::<String, _>("source_name"),
        fund_codes,
        strategy: row.get::<String, _>("strategy"),
        strategy_params_json: row.get::<String, _>("strategy_params_json"),
        start_date,
        end_date,
        current_date,
        calendar,
        initial_cash: parse_decimal(&row.get::<String, _>("initial_cash")),
        cash_available: parse_decimal(&row.get::<String, _>("cash_available")),
        cash_frozen: parse_decimal(&row.get::<String, _>("cash_frozen")),
        buy_fee_rate: row.get::<f64, _>("buy_fee_rate"),
        sell_fee_rate: row.get::<f64, _>("sell_fee_rate"),
        settlement_days: row.get::<i64, _>("settlement_days"),
        status: row.get::<String, _>("status"),
    }))
}

#[allow(clippy::too_many_arguments)]
pub async fn create_run(
    pool: &sqlx::AnyPool,
    user_id: i64,
    mode: &str,
    name: &str,
    source_name: &str,
    fund_codes: &[String],
    strategy: &str,
    strategy_params_json: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    calendar: &[NaiveDate],
    initial_cash: Decimal,
    buy_fee_rate: f64,
    sell_fee_rate: f64,
    settlement_days: i64,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let is_postgres =
        crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let fund_codes_json = serde_json::to_string(fund_codes).map_err(|e| e.to_string())?;
    let calendar_json = serde_json::to_string(
        &calendar
            .iter()
            .map(|d| d.format("%Y-%m-%d").to_string())
            .collect::<Vec<_>>(),
    )
    .map_err(|e| e.to_string())?;

    let sql_pg = r#"
        INSERT INTO sim_run (
          id, user_id, mode, name, source_name,
          fund_codes_json, strategy, strategy_params_json,
          start_date, end_date, "current_date", calendar_json,
          initial_cash, cash_available, cash_frozen,
          buy_fee_rate, sell_fee_rate, settlement_days,
          status, created_at, updated_at
        )
        VALUES (
          ($1)::uuid,$2,$3,$4,$5,$6,$7,$8,
          ($9)::date,($10)::date,($11)::date,$12,
          ($13)::numeric,($14)::numeric,($15)::numeric,
          $16,$17,$18,
          'created',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP
        )
    "#;

    let sql_any = r#"
        INSERT INTO sim_run (
          id, user_id, mode, name, source_name,
          fund_codes_json, strategy, strategy_params_json,
          start_date, end_date, "current_date", calendar_json,
          initial_cash, cash_available, cash_frozen,
          buy_fee_rate, sell_fee_rate, settlement_days,
          status, created_at, updated_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,'created',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
    "#;

    let sql = if is_postgres { sql_pg } else { sql_any };
    sqlx::query(sql)
        .bind(&id)
        .bind(user_id)
        .bind(mode)
        .bind(name)
        .bind(source_name)
        .bind(&fund_codes_json)
        .bind(strategy)
        .bind(strategy_params_json)
        .bind(start_date.format("%Y-%m-%d").to_string())
        .bind(end_date.format("%Y-%m-%d").to_string())
        .bind(start_date.format("%Y-%m-%d").to_string())
        .bind(&calendar_json)
        .bind(initial_cash.to_string())
        .bind(initial_cash.to_string())
        .bind(Decimal::ZERO.to_string())
        .bind(buy_fee_rate)
        .bind(sell_fee_rate)
        .bind(settlement_days)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(id)
}

pub async fn build_calendar(
    pool: &sqlx::AnyPool,
    fund_codes: &[String],
    source_name: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    extra_days: i64,
) -> Result<Vec<NaiveDate>, String> {
    if fund_codes.is_empty() {
        return Err("fund_codes is empty".to_string());
    }

    let is_postgres =
        crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let end_plus = end_date
        .checked_add_signed(chrono::Duration::days(extra_days.max(0)))
        .unwrap_or(end_date);

    let mut sql = String::from(
        r#"
        SELECT DISTINCT CAST(h.nav_date AS TEXT) as nav_date
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE h.source_name = $1
        "#,
    );
    if is_postgres {
        sql.push_str(" AND h.nav_date >= ($2)::date AND h.nav_date <= ($3)::date");
    } else {
        sql.push_str(" AND h.nav_date >= $2 AND h.nav_date <= $3");
    }
    sql.push_str(" AND f.fund_code IN (");
    for (i, _) in fund_codes.iter().enumerate() {
        if i > 0 {
            sql.push(',');
        }
        sql.push_str(&format!("${}", i + 4));
    }
    sql.push_str(") ORDER BY nav_date ASC");

    let mut q = sqlx::query(&sql);
    q = q.bind(source_name);
    q = q.bind(start_date.format("%Y-%m-%d").to_string());
    q = q.bind(end_plus.format("%Y-%m-%d").to_string());
    for code in fund_codes {
        q = q.bind(code);
    }

    let rows = q.fetch_all(pool).await.map_err(|e| e.to_string())?;

    let mut out: Vec<NaiveDate> = Vec::with_capacity(rows.len());
    for r in rows {
        let d: String = r.get("nav_date");
        out.push(parse_date(&d)?);
    }

    Ok(out)
}

pub async fn build_calendar_for_source(
    pool: &sqlx::AnyPool,
    source_name: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    extra_days: i64,
) -> Result<Vec<NaiveDate>, String> {
    let is_postgres = crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let end_plus = end_date
        .checked_add_signed(chrono::Duration::days(extra_days.max(0)))
        .unwrap_or(end_date);

    let mut sql = String::from(
        r#"
        SELECT DISTINCT CAST(h.nav_date AS TEXT) as nav_date
        FROM fund_nav_history h
        WHERE h.source_name = $1
        "#,
    );
    if is_postgres {
        sql.push_str(" AND h.nav_date >= ($2)::date AND h.nav_date <= ($3)::date");
    } else {
        sql.push_str(" AND h.nav_date >= $2 AND h.nav_date <= $3");
    }
    sql.push_str(" ORDER BY nav_date ASC");

    let rows = sqlx::query(&sql)
        .bind(source_name)
        .bind(start_date.format("%Y-%m-%d").to_string())
        .bind(end_plus.format("%Y-%m-%d").to_string())
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut out: Vec<NaiveDate> = Vec::with_capacity(rows.len());
    for r in rows {
        let s: String = r.get("nav_date");
        out.push(parse_date(&s)?);
    }
    Ok(out)
}

pub async fn next_nav_date(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    source_name: &str,
    after_date: NaiveDate,
) -> Result<Option<NaiveDate>, String> {
    let sql_pg = r#"
        SELECT CAST(h.nav_date AS TEXT) as nav_date
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE f.fund_code = $1 AND h.source_name = $2 AND h.nav_date > ($3)::date
        ORDER BY h.nav_date ASC
        LIMIT 1
    "#;
    let sql_any = r#"
        SELECT CAST(h.nav_date AS TEXT) as nav_date
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE f.fund_code = $1 AND h.source_name = $2 AND h.nav_date > $3
        ORDER BY h.nav_date ASC
        LIMIT 1
    "#;

    let row = sqlx::query(sql_pg)
        .bind(fund_code)
        .bind(source_name)
        .bind(after_date.format("%Y-%m-%d").to_string())
        .fetch_optional(pool)
        .await;

    let row = match row {
        Ok(v) => v,
        Err(_) => sqlx::query(sql_any)
            .bind(fund_code)
            .bind(source_name)
            .bind(after_date.format("%Y-%m-%d").to_string())
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?,
    };

    Ok(row.map(|r| {
        let s: String = r.get("nav_date");
        parse_date(&s).unwrap_or(after_date)
    }))
}

pub async fn nav_on_or_before(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    source_name: &str,
    date: NaiveDate,
) -> Result<Option<Decimal>, String> {
    let sql_pg = r#"
        SELECT CAST(h.unit_nav AS TEXT) as unit_nav
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE f.fund_code = $1 AND h.source_name = $2 AND h.nav_date <= ($3)::date
        ORDER BY h.nav_date DESC
        LIMIT 1
    "#;
    let sql_any = r#"
        SELECT CAST(h.unit_nav AS TEXT) as unit_nav
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE f.fund_code = $1 AND h.source_name = $2 AND h.nav_date <= $3
        ORDER BY h.nav_date DESC
        LIMIT 1
    "#;

    let row = sqlx::query(sql_pg)
        .bind(fund_code)
        .bind(source_name)
        .bind(date.format("%Y-%m-%d").to_string())
        .fetch_optional(pool)
        .await;

    let row = match row {
        Ok(v) => v,
        Err(_) => sqlx::query(sql_any)
            .bind(fund_code)
            .bind(source_name)
            .bind(date.format("%Y-%m-%d").to_string())
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?,
    };

    Ok(row.map(|r| parse_decimal(&r.get::<String, _>("unit_nav"))))
}
