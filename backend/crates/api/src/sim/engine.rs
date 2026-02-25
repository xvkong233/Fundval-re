use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::ml;
use rand::SeedableRng;

use super::db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub side: Side,
    pub fund_code: String,
    pub amount: Option<String>,
    pub shares: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PositionView {
    pub fund_code: String,
    pub shares_available: String,
    pub shares_frozen: String,
    pub nav: Option<String>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Observation {
    pub date: String,
    pub cash_available: String,
    pub cash_frozen: String,
    pub cash_receivable: String,
    pub total_equity: String,
    pub positions: Vec<PositionView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StepResult {
    pub date: String,
    pub reward: f64,
    pub done: bool,
    pub observation: Observation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTopkSnapshotParams {
    pub top_k: usize,
    /// 每隔多少个交易日调仓一次（>=1）。
    pub rebalance_every: i64,
    /// 线性打分权重：[pos, dip5, dip20, magic5, magic20]
    pub weights: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTopkTsTimingParams {
    pub top_k: usize,
    pub rebalance_every: i64,
    pub weights: Option<Vec<f64>>,

    /// 参考指数（如 1.000001 上证、1.000300 沪深300、1.000905 中证500）
    pub refer_index_code: String,

    /// MACD 临界点（0..100）。None 表示禁用该方向择时。
    pub sell_macd_point: Option<f64>,
    pub buy_macd_point: Option<f64>,

    /// 止盈：上证指数阈值
    pub sh_composite_index: f64,
    /// 止盈：仓位阈值（0..100，指“权益中持仓占比”）
    pub fund_position: f64,
    /// 止盈：是否要求权益新高
    pub sell_at_top: bool,
    /// 止盈：卖出数值（sell_unit=amount/fundPercent）
    pub sell_num: f64,
    pub sell_unit: String,
    /// 止盈：累计收益率阈值（0..100，按总权益计算）
    pub profit_rate: f64,

    /// 补仓：买入金额（<=100 表示剩余现金百分比，否则表示固定金额）
    pub buy_amount_percent: f64,

    /// quant-service base url
    pub quant_service_url: String,
}

fn normalize_weights(raw: Option<Vec<f64>>) -> [f64; 5] {
    let mut w = [0.0_f64; 5];
    if let Some(v) = raw {
        for (i, x) in v.into_iter().take(5).enumerate() {
            if x.is_finite() {
                w[i] = x;
            }
        }
    }
    // 默认更偏向 20T 反弹
    if w.iter().all(|x| x.abs() < 1e-12) {
        w[4] = 1.0;
    }
    w
}

fn parse_decimal(s: &str) -> Decimal {
    s.trim().parse::<Decimal>().unwrap_or(Decimal::ZERO)
}

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn fmt_dec(d: Decimal) -> String {
    d.round_dp(8).normalize().to_string()
}

fn add_trading_days(calendar: &[NaiveDate], date: NaiveDate, days: i64) -> Option<NaiveDate> {
    let idx = calendar.iter().position(|d| *d == date)?;
    let next = idx as i64 + days;
    if next < 0 {
        return None;
    }
    calendar.get(next as usize).copied()
}

async fn load_positions(
    pool: &sqlx::AnyPool,
    run_id: &str,
) -> Result<Vec<(String, Decimal, Decimal, Decimal)>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
          fund_code,
          CAST(shares_available AS TEXT) as shares_available,
          CAST(shares_frozen AS TEXT) as shares_frozen,
          CAST(avg_cost AS TEXT) as avg_cost
        FROM sim_position
        WHERE CAST(run_id AS TEXT) = $1
        ORDER BY fund_code ASC
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut out: Vec<(String, Decimal, Decimal, Decimal)> = Vec::with_capacity(rows.len());
    for r in rows {
        let code: String = r.get("fund_code");
        let avail = parse_decimal(&r.get::<String, _>("shares_available"));
        let frozen = parse_decimal(&r.get::<String, _>("shares_frozen"));
        let avg_cost = parse_decimal(&r.get::<String, _>("avg_cost"));
        out.push((code, avail, frozen, avg_cost));
    }
    Ok(out)
}

async fn upsert_position(
    pool: &sqlx::AnyPool,
    run_id: &str,
    fund_code: &str,
    shares_available: Decimal,
    shares_frozen: Decimal,
    avg_cost: Decimal,
) -> Result<(), String> {
    let is_postgres =
        crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let sql = if is_postgres {
        r#"
            INSERT INTO sim_position (run_id, fund_code, shares_available, shares_frozen, avg_cost, updated_at)
            VALUES (($1)::uuid,$2,($3)::numeric,($4)::numeric,($5)::numeric,CURRENT_TIMESTAMP)
            ON CONFLICT (run_id, fund_code) DO UPDATE SET
              shares_available = excluded.shares_available,
              shares_frozen = excluded.shares_frozen,
              avg_cost = excluded.avg_cost,
              updated_at = CURRENT_TIMESTAMP
        "#
    } else {
        r#"
            INSERT INTO sim_position (run_id, fund_code, shares_available, shares_frozen, avg_cost, updated_at)
            VALUES ($1,$2,CAST($3 AS TEXT),CAST($4 AS TEXT),CAST($5 AS TEXT),CURRENT_TIMESTAMP)
            ON CONFLICT (run_id, fund_code) DO UPDATE SET
              shares_available = excluded.shares_available,
              shares_frozen = excluded.shares_frozen,
              avg_cost = excluded.avg_cost,
              updated_at = CURRENT_TIMESTAMP
        "#
    };

    sqlx::query(sql)
        .bind(run_id)
        .bind(fund_code)
        .bind(shares_available.to_string())
        .bind(shares_frozen.to_string())
        .bind(avg_cost.to_string())
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn sum_receivable(pool: &sqlx::AnyPool, run_id: &str) -> Result<Decimal, String> {
    let rows = sqlx::query(
        r#"
        SELECT CAST(amount AS TEXT) as amount
        FROM sim_cash_receivable
        WHERE CAST(run_id AS TEXT) = $1
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut total = Decimal::ZERO;
    for r in rows {
        total += parse_decimal(&r.get::<String, _>("amount"));
    }
    Ok(total)
}

async fn settle_receivable_for_date(
    pool: &sqlx::AnyPool,
    run_id: &str,
    date: NaiveDate,
) -> Result<Decimal, String> {
    let date_s = fmt_date(date);
    let rows = sqlx::query(
        r#"
        SELECT CAST(id AS TEXT) as id, CAST(amount AS TEXT) as amount
        FROM sim_cash_receivable
        WHERE CAST(run_id AS TEXT) = $1 AND CAST(settle_date AS TEXT) = $2
        "#,
    )
    .bind(run_id)
    .bind(&date_s)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut total = Decimal::ZERO;
    let mut ids: Vec<String> = Vec::with_capacity(rows.len());
    for r in rows {
        ids.push(r.get::<String, _>("id"));
        total += parse_decimal(&r.get::<String, _>("amount"));
    }

    for id in ids {
        let _ = sqlx::query(
            "DELETE FROM sim_cash_receivable WHERE CAST(run_id AS TEXT) = $1 AND CAST(id AS TEXT) = $2",
        )
        .bind(run_id)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(total)
}

async fn update_run_cash_and_date(
    pool: &sqlx::AnyPool,
    run_id: &str,
    cash_available: Decimal,
    cash_frozen: Decimal,
    current_date: NaiveDate,
) -> Result<(), String> {
    let is_postgres =
        crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let sql = if is_postgres {
        r#"
            UPDATE sim_run
            SET cash_available = ($2)::numeric,
                cash_frozen = ($3)::numeric,
                "current_date" = ($4)::date,
                updated_at = CURRENT_TIMESTAMP
            WHERE CAST(id AS TEXT) = $1
        "#
    } else {
        r#"
            UPDATE sim_run
            SET cash_available = CAST($2 AS TEXT),
                cash_frozen = CAST($3 AS TEXT),
                "current_date" = $4,
                updated_at = CURRENT_TIMESTAMP
            WHERE CAST(id AS TEXT) = $1
        "#
    };

    sqlx::query(sql)
    .bind(run_id)
    .bind(cash_available.to_string())
    .bind(cash_frozen.to_string())
    .bind(fmt_date(current_date))
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn upsert_daily_equity(
    pool: &sqlx::AnyPool,
    run_id: &str,
    date: NaiveDate,
    total_equity: f64,
    cash_available: f64,
    cash_frozen: f64,
    cash_receivable: f64,
    positions_value: f64,
) -> Result<(), String> {
    let is_postgres =
        crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let sql = if is_postgres {
        r#"
            INSERT INTO sim_daily_equity (
              run_id, date, total_equity,
              cash_available, cash_frozen, cash_receivable, positions_value,
              created_at
            )
            VALUES (($1)::uuid,($2)::date,$3,$4,$5,$6,$7,CURRENT_TIMESTAMP)
            ON CONFLICT (run_id, date) DO UPDATE SET
              total_equity = excluded.total_equity,
              cash_available = excluded.cash_available,
              cash_frozen = excluded.cash_frozen,
              cash_receivable = excluded.cash_receivable,
              positions_value = excluded.positions_value
        "#
    } else {
        r#"
            INSERT INTO sim_daily_equity (
              run_id, date, total_equity,
              cash_available, cash_frozen, cash_receivable, positions_value,
              created_at
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,CURRENT_TIMESTAMP)
            ON CONFLICT (run_id, date) DO UPDATE SET
              total_equity = excluded.total_equity,
              cash_available = excluded.cash_available,
              cash_frozen = excluded.cash_frozen,
              cash_receivable = excluded.cash_receivable,
              positions_value = excluded.positions_value
        "#
    };

    sqlx::query(sql)
        .bind(run_id)
        .bind(fmt_date(date))
        .bind(total_equity)
        .bind(cash_available)
        .bind(cash_frozen)
        .bind(cash_receivable)
        .bind(positions_value)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_order(
    pool: &sqlx::AnyPool,
    run_id: &str,
    trade_date: NaiveDate,
    exec_date: NaiveDate,
    side: Side,
    fund_code: &str,
    amount: Option<Decimal>,
    shares: Option<Decimal>,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let is_postgres =
        crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let sql = if is_postgres {
        r#"
            INSERT INTO sim_order (
              id, run_id, trade_date, exec_date, side, fund_code,
              amount, shares,
              status, created_at, updated_at
            )
            VALUES (
              ($1)::uuid,($2)::uuid,($3)::date,($4)::date,$5,$6,
              ($7)::numeric,($8)::numeric,
              'pending',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP
            )
        "#
    } else {
        r#"
            INSERT INTO sim_order (
              id, run_id, trade_date, exec_date, side, fund_code,
              amount, shares,
              status, created_at, updated_at
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'pending',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        "#
    };

    sqlx::query(sql)
        .bind(&id)
        .bind(run_id)
        .bind(fmt_date(trade_date))
        .bind(fmt_date(exec_date))
        .bind(match side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        })
        .bind(fund_code)
        .bind(amount.map(|v| v.to_string()))
        .bind(shares.map(|v| v.to_string()))
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(id)
}

async fn execute_orders_for_date(
    pool: &sqlx::AnyPool,
    run: &db::RunRow,
    date: NaiveDate,
) -> Result<(), String> {
    let date_s = fmt_date(date);
    let is_postgres =
        crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let rows = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          side,
          fund_code,
          CAST(amount AS TEXT) as amount,
          CAST(shares AS TEXT) as shares
        FROM sim_order
        WHERE CAST(run_id AS TEXT) = $1 AND CAST(exec_date AS TEXT) = $2 AND status = 'pending'
        ORDER BY created_at ASC
        "#,
    )
    .bind(&run.id)
    .bind(&date_s)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        return Ok(());
    }

    // Re-load cash for consistent updates inside a tx-like scope.
    let mut cash_frozen = run.cash_frozen;

    for r in rows {
        let order_id: String = r.get("id");
        let side_raw: String = r.get("side");
        let fund_code: String = r.get("fund_code");

        let nav = db::nav_on_or_before(pool, &fund_code, &run.source_name, date)
            .await?
            .ok_or_else(|| format!("missing nav for {fund_code} at {date_s}"))?;
        if nav <= Decimal::ZERO {
            continue;
        }

        if side_raw == "BUY" {
            let amount = parse_decimal(&r.get::<String, _>("amount"));
            if amount <= Decimal::ZERO {
                continue;
            }
            let fee = Decimal::from_f64(run.buy_fee_rate).unwrap_or(Decimal::ZERO) * amount;
            let fee = fee.max(Decimal::ZERO);
            let net = (amount - fee).max(Decimal::ZERO);
            let shares_bought = if net > Decimal::ZERO {
                net / nav
            } else {
                Decimal::ZERO
            };

            cash_frozen -= amount;
            if cash_frozen < Decimal::ZERO {
                cash_frozen = Decimal::ZERO;
            }

            // Update position with average cost basis (include fee in cost).
            let positions = load_positions(pool, &run.id).await?;
            let existing = positions
                .iter()
                .find(|(c, _, _, _)| c.as_str() == fund_code.as_str())
                .cloned();
            let (_, avail, frozen, avg_cost) = existing.unwrap_or((
                fund_code.clone(),
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
            ));
            let total_shares_before = avail + frozen;
            let total_cost_before = avg_cost * total_shares_before;
            let total_shares_after = total_shares_before + shares_bought;
            let avg_cost_after = if total_shares_after > Decimal::ZERO {
                (total_cost_before + amount) / total_shares_after
            } else {
                Decimal::ZERO
            };
            upsert_position(
                pool,
                &run.id,
                &fund_code,
                avail + shares_bought,
                frozen,
                avg_cost_after,
            )
            .await?;

            // Trade record
            let trade_id = Uuid::new_v4().to_string();
            let sql = if is_postgres {
                r#"
                    INSERT INTO sim_trade (
                      id, run_id, order_id,
                      exec_date, side, fund_code, nav, shares,
                      gross_amount, fee, net_amount, settle_date, created_at
                    )
                    VALUES (
                      ($1)::uuid,($2)::uuid,($3)::uuid,($4)::date,'BUY',$5,
                      ($6)::numeric,($7)::numeric,
                      ($8)::numeric,($9)::numeric,($10)::numeric,
                      NULL,
                      CURRENT_TIMESTAMP
                    )
                "#
            } else {
                r#"
                    INSERT INTO sim_trade (
                      id, run_id, order_id,
                      exec_date, side, fund_code, nav, shares,
                      gross_amount, fee, net_amount, settle_date, created_at
                    )
                    VALUES ($1,$2,$3,$4,'BUY',$5,$6,$7,$8,$9,$10,NULL,CURRENT_TIMESTAMP)
                "#
            };

            sqlx::query(sql)
                .bind(&trade_id)
                .bind(&run.id)
                .bind(&order_id)
                .bind(&date_s)
                .bind(&fund_code)
                .bind(nav.to_string())
                .bind(shares_bought.to_string())
                .bind(amount.to_string())
                .bind(fee.to_string())
                .bind(net.to_string())
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;

            let sql = if is_postgres {
                r#"
                    UPDATE sim_order
                    SET status='executed',
                        exec_nav=($2)::numeric,
                        fee=($3)::numeric,
                        executed_shares=($4)::numeric,
                        cash_delta=($5)::numeric,
                        updated_at=CURRENT_TIMESTAMP
                    WHERE CAST(run_id AS TEXT)=$1 AND CAST(id AS TEXT)=$6
                "#
            } else {
                r#"
                    UPDATE sim_order
                    SET status='executed',
                        exec_nav=CAST($2 AS TEXT),
                        fee=CAST($3 AS TEXT),
                        executed_shares=CAST($4 AS TEXT),
                        cash_delta=CAST($5 AS TEXT),
                        updated_at=CURRENT_TIMESTAMP
                    WHERE CAST(run_id AS TEXT)=$1 AND CAST(id AS TEXT)=$6
                "#
            };

            sqlx::query(sql)
                .bind(&run.id)
                .bind(nav.to_string())
                .bind(fee.to_string())
                .bind(shares_bought.to_string())
                .bind((-amount).to_string())
                .bind(order_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        } else if side_raw == "SELL" {
            let shares = parse_decimal(&r.get::<String, _>("shares"));
            if shares <= Decimal::ZERO {
                continue;
            }

            // Reduce frozen shares; cost basis uses average cost and remains constant.
            let positions = load_positions(pool, &run.id).await?;
            let existing = positions
                .iter()
                .find(|(c, _, _, _)| c.as_str() == fund_code.as_str())
                .cloned()
                .ok_or_else(|| "position not found".to_string())?;
            let (_, avail, frozen, avg_cost) = existing;
            let frozen_after = (frozen - shares).max(Decimal::ZERO);
            let total_shares_after = (avail + frozen_after).max(Decimal::ZERO);
            let avg_cost_after = if total_shares_after > Decimal::ZERO {
                avg_cost
            } else {
                Decimal::ZERO
            };
            upsert_position(
                pool,
                &run.id,
                &fund_code,
                avail,
                frozen_after,
                avg_cost_after,
            )
            .await?;

            let gross = shares * nav;
            let fee = Decimal::from_f64(run.sell_fee_rate).unwrap_or(Decimal::ZERO) * gross;
            let fee = fee.max(Decimal::ZERO);
            let net = (gross - fee).max(Decimal::ZERO);

            // Create receivable
            let settle_date = add_trading_days(&run.calendar, date, run.settlement_days)
                .ok_or_else(|| "settle_date out of calendar".to_string())?;
            let receivable_id = Uuid::new_v4().to_string();
            let sql = if is_postgres {
                r#"
                    INSERT INTO sim_cash_receivable (id, run_id, settle_date, amount, created_at)
                    VALUES (($1)::uuid,($2)::uuid,($3)::date,($4)::numeric,CURRENT_TIMESTAMP)
                "#
            } else {
                r#"
                    INSERT INTO sim_cash_receivable (id, run_id, settle_date, amount, created_at)
                    VALUES ($1,$2,$3,CAST($4 AS TEXT),CURRENT_TIMESTAMP)
                "#
            };
            sqlx::query(sql)
                .bind(&receivable_id)
                .bind(&run.id)
                .bind(fmt_date(settle_date))
                .bind(net.to_string())
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;

            // Trade record
            let trade_id = Uuid::new_v4().to_string();
            let sql = if is_postgres {
                r#"
                    INSERT INTO sim_trade (
                      id, run_id, order_id,
                      exec_date, side, fund_code, nav, shares,
                      gross_amount, fee, net_amount, settle_date, created_at
                    )
                    VALUES (
                      ($1)::uuid,($2)::uuid,($3)::uuid,($4)::date,'SELL',$5,
                      ($6)::numeric,($7)::numeric,
                      ($8)::numeric,($9)::numeric,($10)::numeric,
                      ($11)::date,
                      CURRENT_TIMESTAMP
                    )
                "#
            } else {
                r#"
                    INSERT INTO sim_trade (
                      id, run_id, order_id,
                      exec_date, side, fund_code, nav, shares,
                      gross_amount, fee, net_amount, settle_date, created_at
                    )
                    VALUES ($1,$2,$3,$4,'SELL',$5,$6,$7,$8,$9,$10,$11,CURRENT_TIMESTAMP)
                "#
            };

            sqlx::query(sql)
                .bind(&trade_id)
                .bind(&run.id)
                .bind(&order_id)
                .bind(&date_s)
                .bind(&fund_code)
                .bind(nav.to_string())
                .bind(shares.to_string())
                .bind(gross.to_string())
                .bind(fee.to_string())
                .bind(net.to_string())
                .bind(fmt_date(settle_date))
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;

            let sql = if is_postgres {
                r#"
                    UPDATE sim_order
                    SET status='executed',
                        exec_nav=($2)::numeric,
                        fee=($3)::numeric,
                        executed_shares=($4)::numeric,
                        cash_delta=($5)::numeric,
                        settle_date=($6)::date,
                        updated_at=CURRENT_TIMESTAMP
                    WHERE CAST(run_id AS TEXT)=$1 AND CAST(id AS TEXT)=$7
                "#
            } else {
                r#"
                    UPDATE sim_order
                    SET status='executed',
                        exec_nav=CAST($2 AS TEXT),
                        fee=CAST($3 AS TEXT),
                        executed_shares=CAST($4 AS TEXT),
                        cash_delta=CAST($5 AS TEXT),
                        settle_date=$6,
                        updated_at=CURRENT_TIMESTAMP
                    WHERE CAST(run_id AS TEXT)=$1 AND CAST(id AS TEXT)=$7
                "#
            };

            sqlx::query(sql)
                .bind(&run.id)
                .bind(nav.to_string())
                .bind(fee.to_string())
                .bind(shares.to_string())
                .bind(net.to_string())
                .bind(fmt_date(settle_date))
                .bind(order_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    // Persist updated cash (only buy changes frozen; settlement handled elsewhere)
    update_run_cash_and_date(pool, &run.id, run.cash_available, cash_frozen, date).await?;
    Ok(())
}

async fn compute_observation(
    pool: &sqlx::AnyPool,
    run: &db::RunRow,
    date: NaiveDate,
) -> Result<Observation, String> {
    let positions = load_positions(pool, &run.id).await?;
    let receivable = sum_receivable(pool, &run.id).await?;

    let mut views: Vec<PositionView> = Vec::new();
    let mut positions_value = Decimal::ZERO;
    for (code, avail, frozen, _) in positions {
        let total_shares = avail + frozen;
        let nav = db::nav_on_or_before(pool, &code, &run.source_name, date).await?;
        let value = nav.map(|n| n * total_shares);
        if let Some(v) = value {
            positions_value += v;
        }
        views.push(PositionView {
            fund_code: code,
            shares_available: fmt_dec(avail),
            shares_frozen: fmt_dec(frozen),
            nav: nav.map(fmt_dec),
            value: value.map(fmt_dec),
        });
    }

    let total_equity = run.cash_available + run.cash_frozen + receivable + positions_value;

    let positions_value_f = positions_value.to_f64().unwrap_or(0.0);
    let obs = Observation {
        date: fmt_date(date),
        cash_available: fmt_dec(run.cash_available),
        cash_frozen: fmt_dec(run.cash_frozen),
        cash_receivable: fmt_dec(receivable),
        total_equity: fmt_dec(total_equity),
        positions: views,
    };

    upsert_daily_equity(
        pool,
        &run.id,
        date,
        total_equity.to_f64().unwrap_or(0.0),
        run.cash_available.to_f64().unwrap_or(0.0),
        run.cash_frozen.to_f64().unwrap_or(0.0),
        receivable.to_f64().unwrap_or(0.0),
        positions_value_f,
    )
    .await?;

    Ok(obs)
}

#[allow(clippy::too_many_arguments)]
pub async fn env_create(
    pool: &sqlx::AnyPool,
    user_id: i64,
    name: &str,
    source_name: &str,
    fund_codes: &[String],
    start_date: NaiveDate,
    end_date: NaiveDate,
    initial_cash: Decimal,
    buy_fee_rate: f64,
    sell_fee_rate: f64,
    settlement_days: i64,
) -> Result<(String, Observation), String> {
    let calendar = db::build_calendar(
        pool,
        fund_codes,
        source_name,
        start_date,
        end_date,
        settlement_days + 5,
    )
    .await?;
    if calendar.is_empty() {
        return Err("empty trading calendar (no nav history in range)".to_string());
    }
    let run_id = db::create_run(
        pool,
        user_id,
        "env",
        name,
        source_name,
        fund_codes,
        "env_manual",
        "{}",
        start_date,
        end_date,
        &calendar,
        initial_cash,
        buy_fee_rate,
        sell_fee_rate,
        settlement_days,
    )
    .await?;

    let run = db::load_run(pool, &run_id).await?.ok_or("run missing")?;
    let date = run.current_date.unwrap_or(start_date);
    let obs = compute_observation(pool, &run, date).await?;
    Ok((run_id, obs))
}

pub async fn env_step(
    pool: &sqlx::AnyPool,
    run_id: &str,
    actions: &[Action],
) -> Result<StepResult, String> {
    let mut run = db::load_run(pool, run_id)
        .await?
        .ok_or_else(|| "run not found".to_string())?;
    if run.mode != "env" {
        return Err("run is not env mode".to_string());
    }

    let cur = run.current_date.unwrap_or(run.start_date);
    let prev_obs = compute_observation(pool, &run, cur).await?;
    let prev_equity = parse_decimal(&prev_obs.total_equity);

    // Apply actions at current date: create orders and freeze cash/shares.
    for a in actions {
        let code = a.fund_code.trim();
        if code.is_empty() {
            continue;
        }

        let exec_date = db::next_nav_date(pool, code, &run.source_name, cur)
            .await?
            .ok_or_else(|| format!("no next nav date for {code} after {}", fmt_date(cur)))?;

        match a.side {
            Side::Buy => {
                let amount_s = a.amount.as_deref().unwrap_or("0");
                let amount = parse_decimal(amount_s);
                if amount <= Decimal::ZERO {
                    continue;
                }
                if run.cash_available < amount {
                    return Err("insufficient cash".to_string());
                }
                run.cash_available -= amount;
                run.cash_frozen += amount;
                create_order(
                    pool,
                    &run.id,
                    cur,
                    exec_date,
                    Side::Buy,
                    code,
                    Some(amount),
                    None,
                )
                .await?;
            }
            Side::Sell => {
                let shares_s = a.shares.as_deref().unwrap_or("0");
                let shares = parse_decimal(shares_s);
                if shares <= Decimal::ZERO {
                    continue;
                }
                let positions = load_positions(pool, &run.id).await?;
                let (avail, frozen, avg_cost) = positions
                    .iter()
                    .find(|(c, _, _, _)| c.as_str() == code)
                    .map(|(_, a, f, cost)| (*a, *f, *cost))
                    .unwrap_or((Decimal::ZERO, Decimal::ZERO, Decimal::ZERO));
                if avail < shares {
                    return Err("insufficient shares".to_string());
                }
                upsert_position(
                    pool,
                    &run.id,
                    code,
                    avail - shares,
                    frozen + shares,
                    avg_cost,
                )
                .await?;
                create_order(
                    pool,
                    &run.id,
                    cur,
                    exec_date,
                    Side::Sell,
                    code,
                    None,
                    Some(shares),
                )
                .await?;
            }
        }
    }

    // Advance to next trading day in calendar
    let next =
        add_trading_days(&run.calendar, cur, 1).ok_or_else(|| "no next trading day".to_string())?;

    // Persist cash + date
    update_run_cash_and_date(pool, &run.id, run.cash_available, run.cash_frozen, next).await?;

    // Reload run and process executions/settlements at next date
    run = db::load_run(pool, run_id).await?.ok_or("run missing")?;

    execute_orders_for_date(pool, &run, next).await?;

    // Settle receivable cash for next date
    let settled = settle_receivable_for_date(pool, &run.id, next).await?;
    if settled > Decimal::ZERO {
        run.cash_available += settled;
        update_run_cash_and_date(pool, &run.id, run.cash_available, run.cash_frozen, next).await?;
        run = db::load_run(pool, run_id).await?.ok_or("run missing")?;
    }

    let obs = compute_observation(pool, &run, next).await?;
    let equity = parse_decimal(&obs.total_equity);
    let reward = if prev_equity > Decimal::ZERO {
        ((equity - prev_equity) / prev_equity)
            .to_f64()
            .unwrap_or(0.0)
    } else {
        0.0
    };

    let done = next >= run.end_date;
    Ok(StepResult {
        date: obs.date.clone(),
        reward,
        done,
        observation: obs,
    })
}

pub async fn env_observation(pool: &sqlx::AnyPool, run_id: &str) -> Result<Observation, String> {
    let run = db::load_run(pool, run_id)
        .await?
        .ok_or_else(|| "run not found".to_string())?;
    if run.mode != "env" {
        return Err("run is not env mode".to_string());
    }
    let date = run.current_date.unwrap_or(run.start_date);
    compute_observation(pool, &run, date).await
}

#[allow(clippy::too_many_arguments)]
pub async fn backtest_create_buy_and_hold_equal(
    pool: &sqlx::AnyPool,
    user_id: i64,
    name: &str,
    source_name: &str,
    fund_codes: &[String],
    start_date: NaiveDate,
    end_date: NaiveDate,
    initial_cash: Decimal,
    buy_fee_rate: f64,
    sell_fee_rate: f64,
    settlement_days: i64,
) -> Result<String, String> {
    let calendar = db::build_calendar(
        pool,
        fund_codes,
        source_name,
        start_date,
        end_date,
        settlement_days + 5,
    )
    .await?;
    if calendar.is_empty() {
        return Err("empty trading calendar (no nav history in range)".to_string());
    }
    db::create_run(
        pool,
        user_id,
        "backtest",
        name,
        source_name,
        fund_codes,
        "buy_and_hold_equal",
        "{}",
        start_date,
        end_date,
        &calendar,
        initial_cash,
        buy_fee_rate,
        sell_fee_rate,
        settlement_days,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn backtest_create_auto_topk_snapshot(
    pool: &sqlx::AnyPool,
    user_id: i64,
    name: &str,
    source_name: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    initial_cash: Decimal,
    buy_fee_rate: f64,
    sell_fee_rate: f64,
    settlement_days: i64,
    params: AutoTopkSnapshotParams,
) -> Result<String, String> {
    // auto 策略不需要预先给出 universe fund_codes；日历直接基于该 source 全量净值。
    let calendar =
        db::build_calendar_for_source(pool, source_name, start_date, end_date, settlement_days + 5)
            .await?;
    if calendar.is_empty() {
        return Err("empty trading calendar (no nav history in range)".to_string());
    }

    let params_json = serde_json::to_string(&params).map_err(|e| e.to_string())?;
    db::create_run(
        pool,
        user_id,
        "backtest",
        name,
        source_name,
        &[],
        "auto_topk_snapshot",
        &params_json,
        start_date,
        end_date,
        &calendar,
        initial_cash,
        buy_fee_rate,
        sell_fee_rate,
        settlement_days,
    )
    .await
}

pub async fn backtest_run(pool: &sqlx::AnyPool, run_id: &str) -> Result<(), String> {
    let run = db::load_run(pool, run_id)
        .await?
        .ok_or_else(|| "run not found".to_string())?;
    if run.mode != "backtest" {
        return Err("run is not backtest mode".to_string());
    }

    match run.strategy.as_str() {
        "buy_and_hold_equal" => backtest_run_buy_and_hold_equal(pool, run_id).await,
        "auto_topk_snapshot" => backtest_run_auto_topk_snapshot(pool, &run).await,
        "auto_topk_ts_timing" => backtest_run_auto_topk_ts_timing(pool, &run).await,
        other => Err(format!("unknown backtest strategy: {other}")),
    }
}

async fn pick_topk_by_snapshot_score(
    pool: &sqlx::AnyPool,
    date: NaiveDate,
    top_k: usize,
    w: [f64; 5],
) -> Result<Vec<String>, String> {
    let top_k = top_k.clamp(1, 200);

    let is_postgres = crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;

    let rows = sqlx::query(snapshot_score_select_sql(is_postgres))
    .bind(ml::train::PEER_CODE_ALL)
    .bind(fmt_date(date))
    .bind(w[0])
    .bind(w[1])
    .bind(w[2])
    .bind(w[3])
    .bind(w[4])
    .bind(top_k as i64)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut out: Vec<String> = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(r.get::<String, _>("fund_code"));
    }
    Ok(out)
}

/// 用于 `auto_topk_snapshot`：按 fund_signal_snapshot 线性打分选 Top-K。
///
/// 注意：Postgres 下 `as_of_date` 是 DATE，如果用 text 参数会触发
/// “operator does not exist: date = text”，因此需要 `($2)::date` 显式 cast。
pub fn snapshot_score_select_sql(is_postgres: bool) -> &'static str {
    if is_postgres {
        r#"
        SELECT
          fund_code
        FROM fund_signal_snapshot
        WHERE peer_code = $1
          AND as_of_date = ($2)::date
        ORDER BY (
          COALESCE(position_percentile_0_100, 0.0) * $3
          + COALESCE(dip_buy_proba_5t, 0.0) * $4
          + COALESCE(dip_buy_proba_20t, 0.0) * $5
          + COALESCE(magic_rebound_proba_5t, 0.0) * $6
          + COALESCE(magic_rebound_proba_20t, 0.0) * $7
        ) DESC,
        fund_code ASC
        LIMIT $8
        "#
    } else {
        r#"
        SELECT
          fund_code
        FROM fund_signal_snapshot
        WHERE peer_code = $1
          AND CAST(as_of_date AS TEXT) = $2
        ORDER BY (
          COALESCE(position_percentile_0_100, 0.0) * $3
          + COALESCE(dip_buy_proba_5t, 0.0) * $4
          + COALESCE(dip_buy_proba_20t, 0.0) * $5
          + COALESCE(magic_rebound_proba_5t, 0.0) * $6
          + COALESCE(magic_rebound_proba_20t, 0.0) * $7
        ) DESC,
        fund_code ASC
        LIMIT $8
        "#
    }
}

/// 用于 `sim_train_round` 的 upsert。
///
/// 注意：Postgres 下 `run_id` 是 UUID，如果用 text 参数会触发
/// “column run_id is of type uuid but expression is of type text”，因此需要显式 cast。
pub fn sim_train_round_upsert_sql(_is_postgres: bool) -> &'static str {
    if _is_postgres {
        r#"
        INSERT INTO sim_train_round (
          run_id, round, best_total_return, best_final_equity, best_weights_json, created_at
        )
        VALUES (($1)::uuid,$2,$3,$4,$5,CURRENT_TIMESTAMP)
        ON CONFLICT (run_id, round) DO UPDATE SET
          best_total_return = excluded.best_total_return,
          best_final_equity = excluded.best_final_equity,
          best_weights_json = excluded.best_weights_json
        "#
    } else {
        r#"
        INSERT INTO sim_train_round (
          run_id, round, best_total_return, best_final_equity, best_weights_json, created_at
        )
        VALUES ($1,$2,$3,$4,$5,CURRENT_TIMESTAMP)
        ON CONFLICT (run_id, round) DO UPDATE SET
          best_total_return = excluded.best_total_return,
          best_final_equity = excluded.best_final_equity,
          best_weights_json = excluded.best_weights_json
        "#
    }
}

async fn simulate_auto_topk_snapshot_final_equity(
    pool: &sqlx::AnyPool,
    source_name: &str,
    calendar: &[NaiveDate],
    start_date: NaiveDate,
    end_date: NaiveDate,
    initial_cash: Decimal,
    buy_fee_rate: f64,
    sell_fee_rate: f64,
    params: &AutoTopkSnapshotParams,
    w: [f64; 5],
    persist_daily_equity: Option<&str>,
) -> Result<(Decimal, f64), String> {
    let top_k = params.top_k.clamp(1, 200);
    let rebalance_every = params.rebalance_every.clamp(1, 60);

    let mut cash = initial_cash;
    let mut holdings: std::collections::BTreeMap<String, Decimal> = std::collections::BTreeMap::new();
    let mut days_since_rebalance: i64 = 10_000;

    for &d in calendar {
        if d < start_date {
            continue;
        }
        if d > end_date {
            break;
        }

        let do_rebalance = holdings.is_empty() || days_since_rebalance >= rebalance_every;
        if do_rebalance {
            // 先把持仓按当日净值全部卖出（忽略清算/结算延迟，用于“策略级”回测）。
            if !holdings.is_empty() {
                let mut liquidated = Decimal::ZERO;
                for (code, shares) in holdings.iter() {
                    if *shares <= Decimal::ZERO {
                        continue;
                    }
                    let nav = db::nav_on_or_before(pool, code, source_name, d).await?;
                    let Some(nav) = nav else { continue };
                    liquidated += (*shares) * nav;
                }
                if liquidated > Decimal::ZERO {
                    let fee = Decimal::from_f64(sell_fee_rate).unwrap_or(Decimal::ZERO) * liquidated;
                    cash += (liquidated - fee).max(Decimal::ZERO);
                }
                holdings.clear();
            }

            // 选出当日 topK
            let picked = pick_topk_by_snapshot_score(pool, d, top_k, w).await?;
            if !picked.is_empty() && cash > Decimal::ZERO {
                let k = picked.len() as i64;
                let amount_each = cash / Decimal::from(k);
                let fee_rate = Decimal::from_f64(buy_fee_rate).unwrap_or(Decimal::ZERO);
                let mut spent = Decimal::ZERO;
                for code in picked {
                    let nav = db::nav_on_or_before(pool, &code, source_name, d).await?;
                    let Some(nav) = nav else {
                        continue;
                    };
                    if nav <= Decimal::ZERO {
                        continue;
                    }

                    let gross = amount_each;
                    let fee = gross * fee_rate;
                    let net = (gross - fee).max(Decimal::ZERO);
                    let shares = net / nav;
                    if shares > Decimal::ZERO {
                        holdings.insert(code, shares);
                        spent += gross;
                    }
                }
                cash = (cash - spent).max(Decimal::ZERO);
            }

            days_since_rebalance = 0;
        } else {
            days_since_rebalance += 1;
        }

        // 计算当日权益（用当日净值评估）
        let mut positions_value = Decimal::ZERO;
        for (code, shares) in holdings.iter() {
            if *shares <= Decimal::ZERO {
                continue;
            }
            let nav = db::nav_on_or_before(pool, code, source_name, d).await?;
            let Some(nav) = nav else { continue };
            positions_value += (*shares) * nav;
        }
        let total_equity = cash + positions_value;

        if let Some(run_id) = persist_daily_equity {
            upsert_daily_equity(
                pool,
                run_id,
                d,
                total_equity.to_f64().unwrap_or(0.0),
                cash.to_f64().unwrap_or(0.0),
                0.0,
                0.0,
                positions_value.to_f64().unwrap_or(0.0),
            )
            .await?;
        }
    }

    let final_equity = {
        // 以 end_date 最近的交易日估值
        let last = calendar
            .iter()
            .copied()
            .filter(|d| *d >= start_date && *d <= end_date)
            .last()
            .unwrap_or(end_date);

        let mut positions_value = Decimal::ZERO;
        for (code, shares) in holdings.iter() {
            if *shares <= Decimal::ZERO {
                continue;
            }
            let nav = db::nav_on_or_before(pool, code, source_name, last).await?;
            let Some(nav) = nav else { continue };
            positions_value += (*shares) * nav;
        }
        cash + positions_value
    };

    let total_return = if initial_cash > Decimal::ZERO {
        ((final_equity - initial_cash) / initial_cash)
            .to_f64()
            .unwrap_or(0.0)
    } else {
        0.0
    };

    Ok((final_equity, total_return))
}

async fn simulate_auto_topk_ts_timing_final_equity(
    pool: &sqlx::AnyPool,
    source_name: &str,
    calendar: &[NaiveDate],
    start_date: NaiveDate,
    end_date: NaiveDate,
    initial_cash: Decimal,
    buy_fee_rate: f64,
    sell_fee_rate: f64,
    params: &AutoTopkTsTimingParams,
    w: [f64; 5],
    persist_daily_equity: Option<&str>,
) -> Result<(Decimal, f64), String> {
    let top_k = params.top_k.clamp(1, 200);
    let rebalance_every = params.rebalance_every.clamp(1, 60);

    let refer_index_code = params.refer_index_code.trim();
    let refer_source = "eastmoney";

    let quant_base = params
        .quant_service_url
        .trim()
        .trim_end_matches('/')
        .to_string();
    let url_macd = format!("{quant_base}/api/quant/macd");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let idx_start = start_date - chrono::Duration::days(450);
    let idx_series = crate::index_series::load_or_fetch_index_close_series(
        pool,
        &client,
        crate::db::database_kind_from_pool(pool),
        refer_index_code,
        refer_source,
        idx_start,
        end_date,
        3,
    )
        .await
        .unwrap_or_default();
    let idx_points = idx_series
        .iter()
        .enumerate()
        .map(|(i, (d, v))| json!({ "index": i, "date": fmt_date(*d), "val": v.to_f64().unwrap_or(0.0) }))
        .collect::<Vec<_>>();

    let sell_position = params
        .sell_macd_point
        .unwrap_or(0.0)
        .clamp(0.0, 100.0)
        / 100.0;
    let buy_position = params
        .buy_macd_point
        .unwrap_or(0.0)
        .clamp(0.0, 100.0)
        / 100.0;

    let macd_resp = client
        .post(&url_macd)
        .json(&json!({
          "series": idx_points,
          "sell_position": sell_position,
          "buy_position": buy_position
        }))
        .send()
        .await
        .map_err(|e| format!("macd request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("macd http error: {e}"))?
        .json::<Value>()
        .await
        .map_err(|e| format!("macd json failed: {e}"))?;

    let mut buy_days: std::collections::HashSet<NaiveDate> = std::collections::HashSet::new();
    let mut sell_days: std::collections::HashSet<NaiveDate> = std::collections::HashSet::new();
    if let Some(points) = macd_resp.get("points").and_then(|v| v.as_array()) {
        for p in points {
            let d = p.get("date").and_then(|v| v.as_str()).unwrap_or("").trim();
            let Ok(dd) = NaiveDate::parse_from_str(d, "%Y-%m-%d") else { continue };
            let txn = p.get("txnType")
                .and_then(|v| v.as_str())
                .or_else(|| p.get("txn_type").and_then(|v| v.as_str()))
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if txn == "buy" {
                buy_days.insert(dd);
            } else if txn == "sell" {
                sell_days.insert(dd);
            }
        }
    }

    let sh_series = crate::index_series::load_or_fetch_index_close_series(
        pool,
        &client,
        crate::db::database_kind_from_pool(pool),
        "1.000001",
        refer_source,
        start_date,
        end_date,
        3,
    )
        .await
        .unwrap_or_default();
    let mut sh_iter = sh_series.into_iter().peekable();
    let mut sh_latest: Option<(NaiveDate, Decimal)> = None;
    let mut max_equity_seen = initial_cash.to_f64().unwrap_or(0.0);

    let mut cash = initial_cash;
    let mut holdings: std::collections::BTreeMap<String, Decimal> = std::collections::BTreeMap::new();
    let mut avg_cost: std::collections::BTreeMap<String, Decimal> = std::collections::BTreeMap::new();
    let mut days_since_rebalance: i64 = 10_000;
    let mut picked_cache: Vec<String> = Vec::new();

    for &d in calendar {
        if d < start_date {
            continue;
        }
        if d > end_date {
            break;
        }

        while let Some((dd, _)) = sh_iter.peek().copied() {
            if dd <= d {
                sh_latest = sh_iter.next();
            } else {
                break;
            }
        }
        let sh_close = sh_latest.map(|x| x.1).unwrap_or(Decimal::ZERO);

        // value positions (pre-trade)
        let mut positions_value = Decimal::ZERO;
        for (code, shares) in holdings.iter() {
            if *shares <= Decimal::ZERO {
                continue;
            }
            let nav = db::nav_on_or_before(pool, code, source_name, d).await?;
            let Some(nav) = nav else { continue };
            positions_value += (*shares) * nav;
        }
        let mut total_equity = cash + positions_value;

        // stop-profit overlay (portfolio-level) — evaluate BEFORE any buy for the day.
        // If it triggers, we skip any buy/rebalance on the same day to avoid churn.
        let invested_ratio = if total_equity > Decimal::ZERO {
            (positions_value / total_equity).to_f64().unwrap_or(0.0)
        } else {
            0.0
        };
        let total_return = if initial_cash > Decimal::ZERO {
            ((total_equity - initial_cash) / initial_cash)
                .to_f64()
                .unwrap_or(0.0)
        } else {
            0.0
        };

        let sell_timing_ok = params.sell_macd_point.is_none() || sell_days.contains(&d);
        let sell_at_top_ok = !params.sell_at_top
            || total_equity.to_f64().unwrap_or(0.0) >= max_equity_seen - 1e-9;

        let mut stop_profit_triggered = false;
        if !holdings.is_empty()
            && sell_timing_ok
            && sell_at_top_ok
            && sh_close.to_f64().unwrap_or(0.0) > params.sh_composite_index
            && invested_ratio > params.fund_position.clamp(0.0, 100.0) / 100.0
            && total_return > params.profit_rate.clamp(-100.0, 10_000.0) / 100.0
        {
            stop_profit_triggered = true;

            let fee_rate = Decimal::from_f64(sell_fee_rate).unwrap_or(Decimal::ZERO);
            let sell_unit = params.sell_unit.trim();
            let mut new_holdings: std::collections::BTreeMap<String, Decimal> =
                std::collections::BTreeMap::new();
            let mut new_avg_cost: std::collections::BTreeMap<String, Decimal> =
                std::collections::BTreeMap::new();

            // compute current values
            let mut values: Vec<(String, Decimal, Decimal, Decimal)> = Vec::new(); // code, shares, nav, value
            for (code, shares) in holdings.iter() {
                let nav = db::nav_on_or_before(pool, code, source_name, d).await?;
                let Some(nav) = nav else { continue };
                let value = (*shares) * nav;
                values.push((code.clone(), *shares, nav, value));
            }

            if sell_unit == "amount" {
                let remaining_amount =
                    Decimal::from_f64(params.sell_num.max(0.0)).unwrap_or(Decimal::ZERO);
                let total_value: Decimal = values.iter().map(|x| x.3).sum();
                for (code, shares, nav, value) in values {
                    let mut sell_gross = Decimal::ZERO;
                    if total_value > Decimal::ZERO {
                        sell_gross = (remaining_amount * (value / total_value)).min(value);
                    }
                    let sell_shares = if nav > Decimal::ZERO {
                        sell_gross / nav
                    } else {
                        Decimal::ZERO
                    };
                    let sell_shares = sell_shares.min(shares).max(Decimal::ZERO);
                    let gross = sell_shares * nav;
                    let net = (gross - gross * fee_rate).max(Decimal::ZERO);
                    cash += net;
                    let left_shares = shares - sell_shares;
                    if left_shares > Decimal::ZERO {
                        new_holdings.insert(code.clone(), left_shares);
                        if let Some(c) = avg_cost.get(&code) {
                            new_avg_cost.insert(code, *c);
                        }
                    }
                }
            } else {
                let pct = (params.sell_num / 100.0).clamp(0.0, 1.0);
                for (code, shares, nav, value) in values {
                    let sell_gross = value * Decimal::from_f64(pct).unwrap_or(Decimal::ZERO);
                    let sell_shares = if nav > Decimal::ZERO {
                        sell_gross / nav
                    } else {
                        Decimal::ZERO
                    };
                    let sell_shares = sell_shares.min(shares).max(Decimal::ZERO);
                    let gross = sell_shares * nav;
                    let net = (gross - gross * fee_rate).max(Decimal::ZERO);
                    cash += net;
                    let left_shares = shares - sell_shares;
                    if left_shares > Decimal::ZERO {
                        new_holdings.insert(code.clone(), left_shares);
                        if let Some(c) = avg_cost.get(&code) {
                            new_avg_cost.insert(code, *c);
                        }
                    }
                }
            }

            holdings = new_holdings;
            avg_cost = new_avg_cost;

            // recompute equity after stop-profit
            positions_value = Decimal::ZERO;
            for (code, shares) in holdings.iter() {
                if *shares <= Decimal::ZERO {
                    continue;
                }
                let nav = db::nav_on_or_before(pool, code, source_name, d).await?;
                let Some(nav) = nav else { continue };
                positions_value += (*shares) * nav;
            }
            total_equity = cash + positions_value;
        }

        if stop_profit_triggered {
            max_equity_seen = max_equity_seen.max(total_equity.to_f64().unwrap_or(0.0));
            if let Some(run_id) = persist_daily_equity {
                upsert_daily_equity(
                    pool,
                    run_id,
                    d,
                    total_equity.to_f64().unwrap_or(0.0),
                    cash.to_f64().unwrap_or(0.0),
                    0.0,
                    0.0,
                    positions_value.to_f64().unwrap_or(0.0),
                )
                .await?;
            }
            days_since_rebalance += 1;
            continue;
        }

        let is_buy_signal_day = buy_days.contains(&d);
        let can_trade_today = params.buy_macd_point.is_none() || is_buy_signal_day;
        let wants_rebalance = holdings.is_empty() || days_since_rebalance >= rebalance_every;
        let wants_add_on_buy = params.buy_macd_point.is_some() && is_buy_signal_day;

        // rebalance: only on allowed trade days when buy timing is enabled (matches “只在买点调仓/建仓”的 TS 直觉)
        if wants_rebalance && can_trade_today {
            picked_cache = pick_topk_by_snapshot_score(pool, d, top_k, w).await?;

            // sell funds that are no longer in picks (do NOT force liquidate everything)
            if !holdings.is_empty() && !picked_cache.is_empty() {
                let picked_set: std::collections::HashSet<&str> =
                    picked_cache.iter().map(|s| s.as_str()).collect();
                let fee_rate = Decimal::from_f64(sell_fee_rate).unwrap_or(Decimal::ZERO);
                let mut to_remove: Vec<String> = Vec::new();
                for (code, shares) in holdings.iter() {
                    if picked_set.contains(code.as_str()) {
                        continue;
                    }
                    if *shares <= Decimal::ZERO {
                        to_remove.push(code.clone());
                        continue;
                    }
                    let nav = db::nav_on_or_before(pool, code, source_name, d).await?;
                    let Some(nav) = nav else { continue };
                    let gross = (*shares) * nav;
                    let net = (gross - gross * fee_rate).max(Decimal::ZERO);
                    cash += net;
                    to_remove.push(code.clone());
                }
                for code in to_remove {
                    holdings.remove(&code);
                    avg_cost.remove(&code);
                }
            }

            // buy budget across picked list (including top-up existing holdings)
            if !picked_cache.is_empty() && cash > Decimal::ZERO {
                let k = picked_cache.len();
                let buy_amount_percent = params.buy_amount_percent.max(0.0);
                let mut budget = if buy_amount_percent <= 100.0 {
                    cash * Decimal::from_f64(buy_amount_percent / 100.0).unwrap_or(Decimal::ZERO)
                } else {
                    Decimal::from_f64(buy_amount_percent).unwrap_or(Decimal::ZERO)
                };
                budget = budget.min(cash).max(Decimal::ZERO);
                if budget > Decimal::ZERO {
                    let amount_each = budget / Decimal::from(k as i64);
                    let fee_rate = Decimal::from_f64(buy_fee_rate).unwrap_or(Decimal::ZERO);
                    let mut spent = Decimal::ZERO;
                    for code in picked_cache.iter() {
                        let nav = db::nav_on_or_before(pool, code, source_name, d).await?;
                        let Some(nav) = nav else { continue };
                        if nav <= Decimal::ZERO {
                            continue;
                        }

                        let gross = amount_each;
                        let fee = gross * fee_rate;
                        let net = (gross - fee).max(Decimal::ZERO);
                        let buy_shares = net / nav;
                        if buy_shares <= Decimal::ZERO {
                            continue;
                        }

                        let old_shares = holdings.get(code).copied().unwrap_or(Decimal::ZERO);
                        let new_shares = old_shares + buy_shares;
                        holdings.insert(code.clone(), new_shares);

                        // weighted avg_cost (gross includes fee)
                        let old_cost = avg_cost.get(code).copied().unwrap_or(Decimal::ZERO);
                        let new_cost = if old_shares > Decimal::ZERO {
                            ((old_cost * old_shares) + gross) / new_shares
                        } else {
                            gross / new_shares
                        };
                        avg_cost.insert(code.clone(), new_cost);

                        spent += gross;
                    }
                    cash = (cash - spent).max(Decimal::ZERO);
                }
            }

            days_since_rebalance = 0;
        } else {
            // add-on buy: when timing is enabled, every BUY day can add budget (without rebalance / liquidation)
            if wants_add_on_buy {
                if picked_cache.is_empty() {
                    picked_cache = pick_topk_by_snapshot_score(pool, d, top_k, w).await?;
                }
                if !picked_cache.is_empty() && cash > Decimal::ZERO {
                    let k = picked_cache.len();
                    let buy_amount_percent = params.buy_amount_percent.max(0.0);
                    let mut budget = if buy_amount_percent <= 100.0 {
                        cash * Decimal::from_f64(buy_amount_percent / 100.0).unwrap_or(Decimal::ZERO)
                    } else {
                        Decimal::from_f64(buy_amount_percent).unwrap_or(Decimal::ZERO)
                    };
                    budget = budget.min(cash).max(Decimal::ZERO);
                    if budget > Decimal::ZERO {
                        let amount_each = budget / Decimal::from(k as i64);
                        let fee_rate = Decimal::from_f64(buy_fee_rate).unwrap_or(Decimal::ZERO);
                        let mut spent = Decimal::ZERO;
                        for code in picked_cache.iter() {
                            let nav = db::nav_on_or_before(pool, code, source_name, d).await?;
                            let Some(nav) = nav else { continue };
                            if nav <= Decimal::ZERO {
                                continue;
                            }

                            let gross = amount_each;
                            let fee = gross * fee_rate;
                            let net = (gross - fee).max(Decimal::ZERO);
                            let buy_shares = net / nav;
                            if buy_shares <= Decimal::ZERO {
                                continue;
                            }

                            let old_shares = holdings.get(code).copied().unwrap_or(Decimal::ZERO);
                            let new_shares = old_shares + buy_shares;
                            holdings.insert(code.clone(), new_shares);

                            let old_cost = avg_cost.get(code).copied().unwrap_or(Decimal::ZERO);
                            let new_cost = if old_shares > Decimal::ZERO {
                                ((old_cost * old_shares) + gross) / new_shares
                            } else {
                                gross / new_shares
                            };
                            avg_cost.insert(code.clone(), new_cost);

                            spent += gross;
                        }
                        cash = (cash - spent).max(Decimal::ZERO);
                    }
                }
            }

            days_since_rebalance += 1;
        }

        // value positions (post-trade) and persist daily equity
        positions_value = Decimal::ZERO;
        for (code, shares) in holdings.iter() {
            if *shares <= Decimal::ZERO {
                continue;
            }
            let nav = db::nav_on_or_before(pool, code, source_name, d).await?;
            let Some(nav) = nav else { continue };
            positions_value += (*shares) * nav;
        }
        total_equity = cash + positions_value;

        max_equity_seen = max_equity_seen.max(total_equity.to_f64().unwrap_or(0.0));

        if let Some(run_id) = persist_daily_equity {
            upsert_daily_equity(
                pool,
                run_id,
                d,
                total_equity.to_f64().unwrap_or(0.0),
                cash.to_f64().unwrap_or(0.0),
                0.0,
                0.0,
                positions_value.to_f64().unwrap_or(0.0),
            )
            .await?;
        }
    }

    let final_equity = {
        let last = calendar
            .iter()
            .copied()
            .filter(|d| *d >= start_date && *d <= end_date)
            .last()
            .unwrap_or(end_date);

        let mut positions_value = Decimal::ZERO;
        for (code, shares) in holdings.iter() {
            if *shares <= Decimal::ZERO {
                continue;
            }
            let nav = db::nav_on_or_before(pool, code, source_name, last).await?;
            let Some(nav) = nav else { continue };
            positions_value += (*shares) * nav;
        }
        cash + positions_value
    };

    let total_return = if initial_cash > Decimal::ZERO {
        ((final_equity - initial_cash) / initial_cash)
            .to_f64()
            .unwrap_or(0.0)
    } else {
        0.0
    };

    Ok((final_equity, total_return))
}

async fn backtest_run_auto_topk_snapshot(
    pool: &sqlx::AnyPool,
    run: &db::RunRow,
) -> Result<(), String> {
    let params: AutoTopkSnapshotParams = serde_json::from_str(&run.strategy_params_json)
        .map_err(|e| format!("invalid strategy_params_json: {e}"))?;
    let w = normalize_weights(params.weights.clone());

    sqlx::query("DELETE FROM sim_daily_equity WHERE CAST(run_id AS TEXT) = $1")
        .bind(&run.id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    let _ = simulate_auto_topk_snapshot_final_equity(
        pool,
        &run.source_name,
        &run.calendar,
        run.start_date,
        run.end_date,
        run.initial_cash,
        run.buy_fee_rate,
        run.sell_fee_rate,
        &params,
        w,
        Some(&run.id),
    )
    .await?;

    sqlx::query(
        r#"
        UPDATE sim_run
        SET status='done', updated_at=CURRENT_TIMESTAMP
        WHERE CAST(id AS TEXT) = $1
        "#,
    )
    .bind(&run.id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn backtest_create_auto_topk_ts_timing(
    pool: &sqlx::AnyPool,
    user_id: i64,
    name: &str,
    source_name: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    initial_cash: Decimal,
    buy_fee_rate: f64,
    sell_fee_rate: f64,
    settlement_days: i64,
    params: AutoTopkTsTimingParams,
) -> Result<String, String> {
    let calendar =
        db::build_calendar_for_source(pool, source_name, start_date, end_date, settlement_days + 5)
            .await?;
    if calendar.is_empty() {
        return Err("empty trading calendar (no nav history in range)".to_string());
    }

    let params_json = serde_json::to_string(&params).map_err(|e| e.to_string())?;
    db::create_run(
        pool,
        user_id,
        "backtest",
        name,
        source_name,
        &[],
        "auto_topk_ts_timing",
        &params_json,
        start_date,
        end_date,
        &calendar,
        initial_cash,
        buy_fee_rate,
        sell_fee_rate,
        settlement_days,
    )
    .await
}

async fn backtest_run_auto_topk_ts_timing(pool: &sqlx::AnyPool, run: &db::RunRow) -> Result<(), String> {
    let params: AutoTopkTsTimingParams = serde_json::from_str(&run.strategy_params_json)
        .map_err(|e| format!("invalid strategy_params_json: {e}"))?;
    let w = normalize_weights(params.weights.clone());

    sqlx::query("DELETE FROM sim_daily_equity WHERE CAST(run_id AS TEXT) = $1")
        .bind(&run.id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    let _ = simulate_auto_topk_ts_timing_final_equity(
        pool,
        &run.source_name,
        &run.calendar,
        run.start_date,
        run.end_date,
        run.initial_cash,
        run.buy_fee_rate,
        run.sell_fee_rate,
        &params,
        w,
        Some(&run.id),
    )
    .await?;

    sqlx::query(
        r#"
        UPDATE sim_run
        SET status='done', updated_at=CURRENT_TIMESTAMP
        WHERE CAST(id AS TEXT) = $1
        "#,
    )
    .bind(&run.id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct TrainRoundOut {
    pub round: i64,
    pub best_total_return: f64,
    pub best_final_equity: f64,
    pub best_weights: Vec<f64>,
}

fn rand_normal(rng: &mut impl rand::Rng, mean: f64, std: f64) -> f64 {
    // Box-Muller
    let u1: f64 = rng.gen_range(1e-12..1.0);
    let u2: f64 = rng.gen_range(0.0..1.0);
    let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
    mean + z0 * std
}

pub async fn train_auto_topk_snapshot(
    pool: &sqlx::AnyPool,
    run_id: &str,
    rounds: i64,
    population: i64,
    elite_ratio: f64,
    seed: Option<u64>,
) -> Result<Vec<TrainRoundOut>, String> {
    let run = db::load_run(pool, run_id)
        .await?
        .ok_or_else(|| "run not found".to_string())?;
    if run.mode != "backtest" {
        return Err("run is not backtest mode".to_string());
    }
    if run.strategy != "auto_topk_snapshot" {
        return Err("run.strategy is not auto_topk_snapshot".to_string());
    }

    let is_postgres = crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;

    let mut params: AutoTopkSnapshotParams = serde_json::from_str(&run.strategy_params_json)
        .map_err(|e| format!("invalid strategy_params_json: {e}"))?;

    let rounds = rounds.clamp(1, 200);
    let population = population.clamp(5, 200);
    let elite_ratio = elite_ratio.clamp(0.05, 0.5);
    let elite_count = ((population as f64) * elite_ratio).round().clamp(1.0, population as f64)
        as usize;

    sqlx::query("DELETE FROM sim_train_round WHERE CAST(run_id AS TEXT) = $1")
        .bind(&run.id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut mean = normalize_weights(params.weights.clone()).to_vec();
    let mut std = vec![0.8_f64; 5];
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed.unwrap_or(42));

    let mut out: Vec<TrainRoundOut> = Vec::with_capacity(rounds as usize);
    let mut best_overall: Option<(f64, f64, Vec<f64>)> = None;

    for round in 1..=rounds {
        let mut scored: Vec<(f64, f64, Vec<f64>)> = Vec::with_capacity(population as usize);

        for _ in 0..population {
            let mut wv: Vec<f64> = Vec::with_capacity(5);
            for i in 0..5 {
                let v = rand_normal(&mut rng, mean[i], std[i]).clamp(-3.0, 3.0);
                wv.push(v);
            }
            let w = normalize_weights(Some(wv.clone()));
            let (final_equity, total_return) = simulate_auto_topk_snapshot_final_equity(
                pool,
                &run.source_name,
                &run.calendar,
                run.start_date,
                run.end_date,
                run.initial_cash,
                run.buy_fee_rate,
                run.sell_fee_rate,
                &params,
                w,
                None,
            )
            .await?;

            scored.push((
                total_return,
                final_equity.to_f64().unwrap_or(0.0),
                wv,
            ));
        }

        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        let elites = scored.iter().take(elite_count).collect::<Vec<_>>();

        // 更新 mean/std
        for i in 0..5 {
            let m = elites.iter().map(|x| x.2[i]).sum::<f64>() / (elites.len() as f64);
            let v = elites
                .iter()
                .map(|x| (x.2[i] - m) * (x.2[i] - m))
                .sum::<f64>()
                / (elites.len() as f64);
            mean[i] = m;
            std[i] = v.sqrt().clamp(0.05, 2.0);
        }

        let (best_ret, best_equity, best_w) = scored[0].clone();
        if best_overall
            .as_ref()
            .map(|x| best_ret > x.0)
            .unwrap_or(true)
        {
            best_overall = Some((best_ret, best_equity, best_w.clone()));
        }

        let best_weights_json = serde_json::to_string(&best_w).map_err(|e| e.to_string())?;
        sqlx::query(sim_train_round_upsert_sql(is_postgres))
        .bind(&run.id)
        .bind(round)
        .bind(best_ret)
        .bind(best_equity)
        .bind(best_weights_json)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        out.push(TrainRoundOut {
            round,
            best_total_return: best_ret,
            best_final_equity: best_equity,
            best_weights: best_w,
        });
    }

    if let Some((_, _, w)) = best_overall.clone() {
        params.weights = Some(w);
        let params_json = serde_json::to_string(&params).map_err(|e| e.to_string())?;
        sqlx::query(
            r#"
            UPDATE sim_run
            SET strategy_params_json = $2, updated_at = CURRENT_TIMESTAMP
            WHERE CAST(id AS TEXT) = $1
            "#,
        )
        .bind(&run.id)
        .bind(params_json)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(out)
}

pub async fn backtest_run_buy_and_hold_equal(
    pool: &sqlx::AnyPool,
    run_id: &str,
) -> Result<(), String> {
    let mut run = db::load_run(pool, run_id)
        .await?
        .ok_or_else(|| "run not found".to_string())?;
    if run.mode != "backtest" {
        return Err("run is not backtest mode".to_string());
    }
    if run.fund_codes.is_empty() {
        return Err("fund_codes empty".to_string());
    }

    let mut cur = run.current_date.unwrap_or(run.start_date);

    // Place initial equal-weight buys at start date (T), executed at next nav date.
    let n = run.fund_codes.len() as i64;
    let amount_each = if n > 0 {
        run.cash_available / Decimal::from(n)
    } else {
        Decimal::ZERO
    };

    for code in &run.fund_codes {
        let exec_date = db::next_nav_date(pool, code, &run.source_name, cur)
            .await?
            .ok_or_else(|| format!("no next nav date for {code} after {}", fmt_date(cur)))?;
        if amount_each > Decimal::ZERO {
            if run.cash_available < amount_each {
                break;
            }
            run.cash_available -= amount_each;
            run.cash_frozen += amount_each;
            let _ = create_order(
                pool,
                &run.id,
                cur,
                exec_date,
                Side::Buy,
                code,
                Some(amount_each),
                None,
            )
            .await?;
        }
    }

    // Step forward until end_date (inclusive)
    loop {
        let next = add_trading_days(&run.calendar, cur, 1)
            .ok_or_else(|| "no next trading day".to_string())?;
        update_run_cash_and_date(pool, &run.id, run.cash_available, run.cash_frozen, next).await?;
        run = db::load_run(pool, run_id).await?.ok_or("run missing")?;

        execute_orders_for_date(pool, &run, next).await?;
        let settled = settle_receivable_for_date(pool, &run.id, next).await?;
        if settled > Decimal::ZERO {
            run.cash_available += settled;
            update_run_cash_and_date(pool, &run.id, run.cash_available, run.cash_frozen, next)
                .await?;
            run = db::load_run(pool, run_id).await?.ok_or("run missing")?;
        }

        let _ = compute_observation(pool, &run, next).await?;

        cur = next;
        if cur >= run.end_date {
            break;
        }
    }

    sqlx::query(
        r#"
        UPDATE sim_run
        SET status='done', updated_at=CURRENT_TIMESTAMP
        WHERE CAST(id AS TEXT) = $1
        "#,
    )
    .bind(&run.id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}
