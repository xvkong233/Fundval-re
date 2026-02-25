use sqlx::Row;

use super::dataset::DatasetConfig;
use super::signals::bucket_for_percentile;
use super::train::{MlTask, get_sector_model, train_and_store_sector_model};

#[derive(Debug, Clone, Copy)]
pub struct ComputeOpts {
    pub train_if_missing: bool,
}

impl Default for ComputeOpts {
    fn default() -> Self {
        Self {
            train_if_missing: true,
        }
    }
}

pub async fn compute_and_store_fund_snapshot(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    peer_code: &str,
    source_name: &str,
) -> Result<(), String> {
    compute_and_store_fund_snapshot_with_opts(
        pool,
        fund_code,
        peer_code,
        source_name,
        ComputeOpts::default(),
    )
    .await
}

pub async fn compute_and_store_fund_snapshot_with_opts(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    peer_code: &str,
    source_name: &str,
    opts: ComputeOpts,
) -> Result<(), String> {
    let fund_code = fund_code.trim();
    let peer_code = peer_code.trim();
    let source_name = source_name.trim();
    if fund_code.is_empty() || peer_code.is_empty() || source_name.is_empty() {
        return Err("missing fund_code/peer_code/source_name".to_string());
    }

    let nav_rows = sqlx::query(
        r#"
        SELECT
          CAST(h.nav_date AS TEXT) as nav_date,
          CAST(h.unit_nav AS TEXT) as unit_nav
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE f.fund_code = $1 AND h.source_name = $2
        ORDER BY h.nav_date ASC
        "#,
    )
    .bind(fund_code)
    .bind(source_name)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut navs: Vec<(String, f64)> = Vec::with_capacity(nav_rows.len());
    for r in nav_rows {
        let d: String = r.get("nav_date");
        let s: String = r.get("unit_nav");
        if let Ok(v) = s.trim().parse::<f64>() {
            navs.push((d, v));
        }
    }
    if navs.len() < 30 {
        return Err("not enough nav history".to_string());
    }
    let as_of_date = navs.last().map(|(d, _)| d.clone()).unwrap_or_default();

    let features = compute_features(&navs)?;

    let (dip_buy_5t, dip_buy_20t) = predict_two_horizons(
        pool,
        peer_code,
        source_name,
        MlTask::DipBuy,
        &features,
        opts,
    )
    .await?;
    let (magic_5t, magic_20t) = predict_two_horizons(
        pool,
        peer_code,
        source_name,
        MlTask::MagicRebound,
        &features,
        opts,
    )
    .await?;

    let (pos_pct, pos_bucket) = compute_position_percentile_and_bucket(
        pool,
        peer_code,
        source_name,
        &as_of_date,
        fund_code,
    )
    .await?;

    upsert_snapshot(
        pool,
        fund_code,
        peer_code,
        &as_of_date,
        SnapshotValues {
            position_percentile_0_100: pos_pct,
            position_bucket: pos_bucket,
            dip_buy_proba_5t: dip_buy_5t,
            dip_buy_proba_20t: dip_buy_20t,
            magic_rebound_proba_5t: magic_5t,
            magic_rebound_proba_20t: magic_20t,
        },
    )
    .await?;

    Ok(())
}

async fn predict_two_horizons(
    pool: &sqlx::AnyPool,
    peer_code: &str,
    source_name: &str,
    task: MlTask,
    features: &[f64],
    opts: ComputeOpts,
) -> Result<(Option<f64>, Option<f64>), String> {
    let p5 = predict_one(pool, peer_code, source_name, task, 5, features, opts).await?;
    let p20 = predict_one(pool, peer_code, source_name, task, 20, features, opts).await?;
    Ok((p5, p20))
}

async fn predict_one(
    pool: &sqlx::AnyPool,
    peer_code: &str,
    source_name: &str,
    task: MlTask,
    horizon_days: i64,
    features: &[f64],
    opts: ComputeOpts,
) -> Result<Option<f64>, String> {
    if let Some(model) = get_sector_model(pool, peer_code, task, horizon_days).await? {
        return Ok(model.model.predict_proba(features));
    }

    if !opts.train_if_missing {
        return Ok(None);
    }

    // 兜底：缺模型时尝试训练一次（训练没数据则保持 None）。
    let cfg = DatasetConfig {
        lookback_days: 252,
        horizon_days: horizon_days as usize,
        stride_days: 5,
    };
    let _ = train_and_store_sector_model(pool, peer_code, source_name, task, &cfg).await;

    let model = get_sector_model(pool, peer_code, task, horizon_days).await?;
    Ok(model.and_then(|m| m.model.predict_proba(features)))
}

fn compute_features(navs: &[(String, f64)]) -> Result<Vec<f64>, String> {
    let idx = navs.len().saturating_sub(1);
    let lookback = navs.len().clamp(2, 252);
    let dd_mag = drawdown_mag(navs, idx, lookback);
    let ret5 = simple_return(navs, idx, 5).unwrap_or(0.0);
    let ret20 = simple_return(navs, idx, 20).unwrap_or(ret5);
    let vol20 = vol(navs, idx, 20).unwrap_or(0.0);
    Ok(vec![dd_mag, ret5, ret20, vol20])
}

async fn compute_position_percentile_and_bucket(
    pool: &sqlx::AnyPool,
    peer_code: &str,
    source_name: &str,
    as_of_date: &str,
    target_fund_code: &str,
) -> Result<(Option<f64>, Option<String>), String> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT t.fund_code as fund_code
        FROM fund_relate_theme t
        WHERE t.sec_code = $1
        ORDER BY t.fund_code ASC
        LIMIT 300
        "#,
    )
    .bind(peer_code)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if rows.len() < 3 {
        return Ok((None, None));
    }

    let mut scores: Vec<(String, f64)> = Vec::new();
    for r in rows {
        let code: String = r.get("fund_code");
        let nav_rows = sqlx::query(
            r#"
            SELECT CAST(h.unit_nav AS TEXT) as unit_nav
            FROM fund_nav_history h
            JOIN fund f ON f.id = h.fund_id
            WHERE f.fund_code = $1
              AND h.source_name = $2
              AND h.nav_date <= DATE($3)
            ORDER BY h.nav_date DESC
            LIMIT 252
            "#,
        )
        .bind(code.trim())
        .bind(source_name)
        .bind(as_of_date)
        .fetch_all(pool)
        .await;

        let nav_rows = match nav_rows {
            Ok(v) => v,
            Err(_) => continue,
        };
        if nav_rows.len() < 10 {
            continue;
        }

        let mut navs: Vec<(String, f64)> = Vec::with_capacity(nav_rows.len());
        for (i, rr) in nav_rows.into_iter().rev().enumerate() {
            let s: String = rr.get("unit_nav");
            if let Ok(v) = s.trim().parse::<f64>() {
                navs.push((format!("{i}"), v));
            }
        }
        let idx = navs.len() - 1;
        let dd_mag = drawdown_mag(&navs, idx, navs.len().clamp(2, 252));
        let score = (1.0 - dd_mag).clamp(0.0, 1.0);
        scores.push((code, score));
    }

    if scores.len() < 3 {
        return Ok((None, None));
    }

    let target_score = scores
        .iter()
        .find(|(c, _)| c == target_fund_code)
        .map(|(_, s)| *s);
    let Some(target_score) = target_score else {
        return Ok((None, None));
    };

    scores.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let n = scores.len();
    let mut rank = 0_usize;
    for (i, (_, s)) in scores.iter().enumerate() {
        if *s <= target_score {
            rank = i;
        }
    }
    let pct = if n <= 1 {
        50.0
    } else {
        (rank as f64) * 100.0 / ((n - 1) as f64)
    };
    let bucket = bucket_for_percentile(pct).as_str().to_string();
    Ok((Some(pct), Some(bucket)))
}

#[derive(Debug, Clone)]
struct SnapshotValues {
    position_percentile_0_100: Option<f64>,
    position_bucket: Option<String>,
    dip_buy_proba_5t: Option<f64>,
    dip_buy_proba_20t: Option<f64>,
    magic_rebound_proba_5t: Option<f64>,
    magic_rebound_proba_20t: Option<f64>,
}

async fn upsert_snapshot(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    peer_code: &str,
    as_of_date: &str,
    values: SnapshotValues,
) -> Result<(), String> {
    let pos_pct = values.position_percentile_0_100.map(|v| v.to_string());
    let dip_buy_5t = values.dip_buy_proba_5t.map(|v| v.to_string());
    let dip_buy_20t = values.dip_buy_proba_20t.map(|v| v.to_string());
    let magic_5t = values.magic_rebound_proba_5t.map(|v| v.to_string());
    let magic_20t = values.magic_rebound_proba_20t.map(|v| v.to_string());

    let is_postgres = crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let sql = if is_postgres {
        r#"
        INSERT INTO fund_signal_snapshot (
          fund_code, peer_code, as_of_date,
          position_percentile_0_100, position_bucket,
          dip_buy_proba_5t, dip_buy_proba_20t,
          magic_rebound_proba_5t, magic_rebound_proba_20t,
          computed_at, created_at, updated_at
        )
        VALUES (
          $1,$2,DATE($3),
          CAST(CAST($4 AS TEXT) AS DOUBLE PRECISION),$5,
          CAST(CAST($6 AS TEXT) AS DOUBLE PRECISION),CAST(CAST($7 AS TEXT) AS DOUBLE PRECISION),
          CAST(CAST($8 AS TEXT) AS DOUBLE PRECISION),CAST(CAST($9 AS TEXT) AS DOUBLE PRECISION),
          CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )
        ON CONFLICT (fund_code, peer_code, as_of_date) DO UPDATE SET
          position_percentile_0_100 = excluded.position_percentile_0_100,
          position_bucket = excluded.position_bucket,
          dip_buy_proba_5t = excluded.dip_buy_proba_5t,
          dip_buy_proba_20t = excluded.dip_buy_proba_20t,
          magic_rebound_proba_5t = excluded.magic_rebound_proba_5t,
          magic_rebound_proba_20t = excluded.magic_rebound_proba_20t,
          computed_at = CURRENT_TIMESTAMP,
          updated_at = CURRENT_TIMESTAMP
        "#
    } else {
        r#"
        INSERT INTO fund_signal_snapshot (
          fund_code, peer_code, as_of_date,
          position_percentile_0_100, position_bucket,
          dip_buy_proba_5t, dip_buy_proba_20t,
          magic_rebound_proba_5t, magic_rebound_proba_20t,
          computed_at, created_at, updated_at
        )
        VALUES (
          $1,$2,DATE($3),
          CAST($4 AS REAL),$5,
          CAST($6 AS REAL),CAST($7 AS REAL),
          CAST($8 AS REAL),CAST($9 AS REAL),
          CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )
        ON CONFLICT (fund_code, peer_code, as_of_date) DO UPDATE SET
          position_percentile_0_100 = excluded.position_percentile_0_100,
          position_bucket = excluded.position_bucket,
          dip_buy_proba_5t = excluded.dip_buy_proba_5t,
          dip_buy_proba_20t = excluded.dip_buy_proba_20t,
          magic_rebound_proba_5t = excluded.magic_rebound_proba_5t,
          magic_rebound_proba_20t = excluded.magic_rebound_proba_20t,
          computed_at = CURRENT_TIMESTAMP,
          updated_at = CURRENT_TIMESTAMP
        "#
    };

    sqlx::query(sql)
    .bind(fund_code)
    .bind(peer_code)
    .bind(as_of_date)
    .bind(pos_pct)
    .bind(values.position_bucket)
    .bind(dip_buy_5t)
    .bind(dip_buy_20t)
    .bind(magic_5t)
    .bind(magic_20t)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn drawdown_mag(navs: &[(String, f64)], idx: usize, lookback: usize) -> f64 {
    if navs.is_empty() || idx >= navs.len() {
        return 0.0;
    }
    let start = idx + 1 - lookback;
    let mut max_v = f64::MIN;
    for &(_, v) in navs.iter().take(idx + 1).skip(start) {
        if v > max_v {
            max_v = v;
        }
    }
    let now = navs[idx].1;
    if max_v <= 0.0 || now <= 0.0 {
        return 0.0;
    }
    ((max_v - now) / max_v).max(0.0)
}

fn simple_return(navs: &[(String, f64)], idx: usize, lookback: usize) -> Option<f64> {
    if idx < lookback || idx >= navs.len() {
        return None;
    }
    let base = navs[idx - lookback].1;
    let now = navs[idx].1;
    if base <= 0.0 {
        return None;
    }
    Some(now / base - 1.0)
}

fn vol(navs: &[(String, f64)], idx: usize, lookback: usize) -> Option<f64> {
    if idx < lookback || idx >= navs.len() {
        return None;
    }
    let start = idx + 1 - lookback;
    let mut rets: Vec<f64> = Vec::with_capacity(lookback);
    for i in (start + 1)..=idx {
        let prev = navs[i - 1].1;
        let now = navs[i].1;
        if prev <= 0.0 || now <= 0.0 {
            continue;
        }
        rets.push(now / prev - 1.0);
    }
    if rets.len() < 2 {
        return None;
    }
    let mean = rets.iter().sum::<f64>() / (rets.len() as f64);
    let var = rets
        .iter()
        .map(|r| {
            let d = r - mean;
            d * d
        })
        .sum::<f64>()
        / (rets.len() as f64);
    Some(var.sqrt())
}
