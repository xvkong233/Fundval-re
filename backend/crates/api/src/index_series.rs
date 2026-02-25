use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::Row;
use uuid::Uuid;

use crate::db::DatabaseKind;
use crate::eastmoney;

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub async fn load_index_close_series(
    pool: &sqlx::AnyPool,
    index_code: &str,
    source_name: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<Vec<(NaiveDate, Decimal)>, String> {
    let mut out: Vec<(NaiveDate, Decimal)> = Vec::new();
    let start_s = fmt_date(start_date);
    let end_s = fmt_date(end_date);

    let rows = sqlx::query(
        r#"
        SELECT
          CAST(trade_date AS TEXT) as trade_date,
          CAST(close AS TEXT) as close
        FROM index_daily_price
        WHERE index_code = $1 AND source_name = $2
          AND CAST(trade_date AS TEXT) >= $3 AND CAST(trade_date AS TEXT) <= $4
        ORDER BY trade_date ASC
        "#,
    )
    .bind(index_code.trim())
    .bind(source_name.trim())
    .bind(&start_s)
    .bind(&end_s)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    for r in rows {
        let d: String = r.get("trade_date");
        let c: String = r.get("close");
        let Ok(dd) = NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d") else { continue };
        let Ok(cc) = c.trim().parse::<Decimal>() else { continue };
        out.push((dd, cc));
    }
    Ok(out)
}

async fn upsert_index_close_series(
    pool: &sqlx::AnyPool,
    db_kind: DatabaseKind,
    index_code: &str,
    source_name: &str,
    list: Vec<eastmoney::IndexKlineRow>,
) {
    if list.is_empty() {
        return;
    }

    let sql_pg = r#"
        INSERT INTO index_daily_price (
          id, index_code, source_name, trade_date, close, created_at, updated_at
        )
        VALUES (($1)::uuid,$2,$3,($4)::date,($5)::numeric,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        ON CONFLICT (index_code, source_name, trade_date) DO UPDATE SET
          close = excluded.close,
          updated_at = CURRENT_TIMESTAMP
    "#;
    let sql_any = r#"
        INSERT INTO index_daily_price (
          id, index_code, source_name, trade_date, close, created_at, updated_at
        )
        VALUES ($1,$2,$3,$4,$5,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        ON CONFLICT (index_code, source_name, trade_date) DO UPDATE SET
          close = excluded.close,
          updated_at = CURRENT_TIMESTAMP
    "#;

    for it in list {
        let id = Uuid::new_v4().to_string();
        let trade_date = fmt_date(it.trade_date);
        let close = it.close.to_string();

        let r = if db_kind == DatabaseKind::Postgres {
            sqlx::query(sql_pg)
                .bind(&id)
                .bind(index_code.trim())
                .bind(source_name.trim())
                .bind(&trade_date)
                .bind(&close)
                .execute(pool)
                .await
        } else {
            sqlx::query(sql_any)
                .bind(&id)
                .bind(index_code.trim())
                .bind(source_name.trim())
                .bind(&trade_date)
                .bind(&close)
                .execute(pool)
                .await
        };

        if r.is_err() && db_kind == DatabaseKind::Postgres {
            let _ = sqlx::query(sql_any)
                .bind(&id)
                .bind(index_code.trim())
                .bind(source_name.trim())
                .bind(&trade_date)
                .bind(&close)
                .execute(pool)
                .await;
        }
    }
}

pub async fn load_or_fetch_index_close_series(
    pool: &sqlx::AnyPool,
    client: &reqwest::Client,
    db_kind: DatabaseKind,
    index_code: &str,
    source_name: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    min_points: usize,
) -> Result<Vec<(NaiveDate, Decimal)>, String> {
    let mut out = load_index_close_series(pool, index_code, source_name, start_date, end_date).await?;
    if out.len() >= min_points {
        return Ok(out);
    }

    if source_name.trim() != "eastmoney" {
        return Ok(out);
    }

    if let Ok(list) =
        eastmoney::fetch_index_kline_daily(client, index_code.trim(), start_date, end_date).await
    {
        if !list.is_empty() {
            upsert_index_close_series(pool, db_kind, index_code, source_name, list).await;
            out = load_index_close_series(pool, index_code, source_name, start_date, end_date)
                .await
                .unwrap_or_default();
        }
    }

    Ok(out)
}

