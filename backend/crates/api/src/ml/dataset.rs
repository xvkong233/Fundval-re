use std::collections::{BTreeMap, HashMap};

use sqlx::Row;

use super::signals::{MAGIC_REBOUND_THRESHOLD_5T, MAGIC_REBOUND_THRESHOLD_20T};

#[derive(Debug, Clone, Copy)]
pub struct DatasetConfig {
    pub lookback_days: usize,
    pub horizon_days: usize,
    pub stride_days: usize,
}

#[derive(Debug, Clone)]
pub struct TriggerSample {
    pub fund_code: String,
    pub as_of_date: String,
    pub features: Vec<f64>,
    pub dip_buy_success: bool,
    pub magic_rebound: bool,
}

pub async fn build_trigger_samples_for_peer(
    pool: &sqlx::AnyPool,
    peer_code: &str,
    source_name: &str,
    cfg: &DatasetConfig,
) -> Result<Vec<TriggerSample>, String> {
    let rows = sqlx::query(
        r#"
        SELECT CAST(f.id AS TEXT) as fund_id, f.fund_code as fund_code
        FROM fund f
        JOIN fund_relate_theme t ON t.fund_code = f.fund_code
        WHERE t.sec_code = $1
        GROUP BY f.id, f.fund_code
        ORDER BY f.fund_code ASC
        LIMIT 800
        "#,
    )
    .bind(peer_code)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if rows.len() < 3 {
        return Ok(Vec::new());
    }

    let mut fund_ids: Vec<(String, String)> = Vec::with_capacity(rows.len());
    for r in rows {
        let fund_id: String = r.get("fund_id");
        let fund_code: String = r.get("fund_code");
        if fund_id.trim().is_empty() || fund_code.trim().is_empty() {
            continue;
        }
        fund_ids.push((fund_id, fund_code));
    }

    let mut series: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    for (fund_id, fund_code) in &fund_ids {
        let nav_rows = sqlx::query(
            r#"
            SELECT CAST(nav_date AS TEXT) as nav_date, CAST(unit_nav AS TEXT) as unit_nav
            FROM fund_nav_history
            WHERE CAST(fund_id AS TEXT) = $1 AND source_name = $2
            ORDER BY nav_date ASC
            "#,
        )
        .bind(fund_id)
        .bind(source_name)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut navs: Vec<(String, f64)> = Vec::with_capacity(nav_rows.len());
        for rr in nav_rows {
            let d: String = rr.get("nav_date");
            let s: String = rr.get("unit_nav");
            if let Ok(v) = s.trim().parse::<f64>() {
                navs.push((d, v));
            }
        }
        if navs.len() >= 2 {
            series.insert(fund_code.clone(), navs);
        }
    }

    if series.len() < 3 {
        return Ok(Vec::new());
    }

    // 选一个“足够长”的基金作为采样基准，避免选到短历史基金导致样本为空。
    let stride = cfg.stride_days.max(1);
    let lookback = cfg.lookback_days.max(2);
    let h = cfg.horizon_days.max(1);
    let rebound_th = if h <= 5 {
        MAGIC_REBOUND_THRESHOLD_5T
    } else {
        MAGIC_REBOUND_THRESHOLD_20T
    };

    let mut base_code: Option<String> = None;
    let mut base_len: usize = 0;
    for (code, navs) in &series {
        if navs.len() < lookback + h + 1 {
            continue;
        }
        if navs.len() > base_len
            || (navs.len() == base_len
                && base_code
                    .as_deref()
                    .is_some_and(|best| code.as_str() < best))
        {
            base_code = Some(code.clone());
            base_len = navs.len();
        }
    }
    let base_code = if let Some(c) = base_code {
        c
    } else {
        let mut codes: Vec<String> = series.keys().cloned().collect();
        codes.sort();
        codes.first().cloned().ok_or("no series")?
    };

    let base_dates: Vec<String> = series
        .get(&base_code)
        .ok_or("no base series")?
        .iter()
        .map(|(d, _)| d.clone())
        .collect();

    // 预先构造 date -> {fund_code -> index}，便于对齐同一日期的横截面排序。
    let mut index_by_date: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();
    for (code, navs) in &series {
        for (idx, (d, _)) in navs.iter().enumerate() {
            index_by_date
                .entry(d.clone())
                .or_default()
                .push((code.clone(), idx));
        }
    }

    let mut out: Vec<TriggerSample> = Vec::new();

    for (t_idx, d) in base_dates.iter().enumerate() {
        if t_idx % stride != 0 {
            continue;
        }
        let Some(list) = index_by_date.get(d) else {
            continue;
        };

        // 收集当日所有可用基金的 dd_mag
        let mut dd_items: Vec<(String, usize, f64)> = Vec::new();
        for (code, idx) in list {
            let navs = match series.get(code) {
                Some(v) => v,
                None => continue,
            };
            if *idx + h >= navs.len() {
                continue;
            }
            let lb = lookback.min(*idx + 1).max(1);
            let dd_mag = drawdown_mag(navs, *idx, lb);
            dd_items.push((code.clone(), *idx, dd_mag));
        }

        if dd_items.len() < 3 {
            continue;
        }

        dd_items.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        let n = dd_items.len();
        let top_k = ((n as f64) * 0.2).ceil().max(1.0) as usize;
        let top_k = top_k.min(n);

        for (rank, (code, idx, dd_mag)) in dd_items.into_iter().enumerate() {
            if rank >= top_k {
                break;
            }

            let navs = series.get(&code).ok_or("missing navs")?;
            let nav_now = navs[idx].1;
            let nav_future = navs[idx + h].1;
            if nav_now <= 0.0 {
                continue;
            }

            let dip_buy_success = (nav_future / nav_now - 1.0) > 0.0;
            let max_future = max_nav(navs, idx + 1, idx + h);
            let magic_rebound = (max_future / nav_now - 1.0) >= rebound_th;

            let ret5 = simple_return(navs, idx, 5).unwrap_or(0.0);
            let ret20 = simple_return(navs, idx, 20).unwrap_or(ret5);
            let vol20 = vol(navs, idx, 20).unwrap_or(0.0);

            out.push(TriggerSample {
                fund_code: code,
                as_of_date: d.clone(),
                features: vec![dd_mag, ret5, ret20, vol20],
                dip_buy_success,
                magic_rebound,
            });
        }
    }

    Ok(out)
}

pub async fn build_trigger_samples_for_all_funds(
    pool: &sqlx::AnyPool,
    source_name: &str,
    cfg: &DatasetConfig,
) -> Result<Vec<TriggerSample>, String> {
    let rows = sqlx::query(
        r#"
        SELECT CAST(f.id AS TEXT) as fund_id, f.fund_code as fund_code
        FROM fund f
        ORDER BY f.fund_code ASC
        LIMIT 10000
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if rows.len() < 3 {
        return Ok(Vec::new());
    }

    let mut fund_ids: Vec<(String, String)> = Vec::with_capacity(rows.len());
    for r in rows {
        let fund_id: String = r.get("fund_id");
        let fund_code: String = r.get("fund_code");
        if fund_id.trim().is_empty() || fund_code.trim().is_empty() {
            continue;
        }
        fund_ids.push((fund_id, fund_code));
    }

    // 复用 peer 逻辑：用 fund_ids 构造 series，再做横截面采样。
    // NOTE: 为保持改动最小，这里沿用与 build_trigger_samples_for_peer 相同的 per-fund nav 查询方式。
    let mut series: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    for (fund_id, fund_code) in &fund_ids {
        let nav_rows = sqlx::query(
            r#"
            SELECT CAST(nav_date AS TEXT) as nav_date, CAST(unit_nav AS TEXT) as unit_nav
            FROM fund_nav_history
            WHERE CAST(fund_id AS TEXT) = $1 AND source_name = $2
            ORDER BY nav_date ASC
            "#,
        )
        .bind(fund_id)
        .bind(source_name)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut navs: Vec<(String, f64)> = Vec::with_capacity(nav_rows.len());
        for rr in nav_rows {
            let d: String = rr.get("nav_date");
            let s: String = rr.get("unit_nav");
            if let Ok(v) = s.trim().parse::<f64>() {
                navs.push((d, v));
            }
        }
        if navs.len() >= 2 {
            series.insert(fund_code.clone(), navs);
        }
    }

    if series.len() < 3 {
        return Ok(Vec::new());
    }

    let mut codes: Vec<String> = series.keys().cloned().collect();
    codes.sort();
    let stride = cfg.stride_days.max(1);
    let lookback = cfg.lookback_days.max(2);
    let h = cfg.horizon_days.max(1);
    let rebound_th = if h <= 5 {
        MAGIC_REBOUND_THRESHOLD_5T
    } else {
        MAGIC_REBOUND_THRESHOLD_20T
    };

    let mut base_code: Option<String> = None;
    let mut base_len: usize = 0;
    for (code, navs) in &series {
        if navs.len() < lookback + h + 1 {
            continue;
        }
        if navs.len() > base_len
            || (navs.len() == base_len
                && base_code
                    .as_deref()
                    .is_some_and(|best| code.as_str() < best))
        {
            base_code = Some(code.clone());
            base_len = navs.len();
        }
    }
    let base_code = base_code.unwrap_or_else(|| codes.first().cloned().unwrap_or_default());
    if base_code.trim().is_empty() {
        return Ok(Vec::new());
    }

    let base_dates: Vec<String> = series
        .get(&base_code)
        .ok_or("no base series")?
        .iter()
        .map(|(d, _)| d.clone())
        .collect();

    let mut index_by_date: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();
    for (code, navs) in &series {
        for (idx, (d, _)) in navs.iter().enumerate() {
            index_by_date
                .entry(d.clone())
                .or_default()
                .push((code.clone(), idx));
        }
    }

    let mut out: Vec<TriggerSample> = Vec::new();

    for (t_idx, d) in base_dates.iter().enumerate() {
        if t_idx % stride != 0 {
            continue;
        }
        let Some(list) = index_by_date.get(d) else {
            continue;
        };

        let mut dd_items: Vec<(String, usize, f64)> = Vec::new();
        for (code, idx) in list {
            let navs = match series.get(code) {
                Some(v) => v,
                None => continue,
            };
            if *idx + h >= navs.len() {
                continue;
            }
            // 允许样本使用“可用范围内的 lookback”（与在线特征一致），避免小历史基金被完全排除。
            let lb = lookback.min(*idx + 1).max(1);
            let dd_mag = drawdown_mag(navs, *idx, lb);
            dd_items.push((code.clone(), *idx, dd_mag));
        }

        if dd_items.len() < 3 {
            continue;
        }

        dd_items.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        let n = dd_items.len();
        let top_k = ((n as f64) * 0.2).ceil().max(1.0) as usize;
        let top_k = top_k.min(n);

        for (rank, (code, idx, dd_mag)) in dd_items.into_iter().enumerate() {
            if rank >= top_k {
                break;
            }

            let navs = series.get(&code).ok_or("missing navs")?;
            let nav_now = navs[idx].1;
            let nav_future = navs[idx + h].1;
            if nav_now <= 0.0 {
                continue;
            }

            let dip_buy_success = (nav_future / nav_now - 1.0) > 0.0;
            let max_future = max_nav(navs, idx + 1, idx + h);
            let magic_rebound = (max_future / nav_now - 1.0) >= rebound_th;

            let ret5 = simple_return(navs, idx, 5).unwrap_or(0.0);
            let ret20 = simple_return(navs, idx, 20).unwrap_or(ret5);
            let vol20 = vol(navs, idx, 20).unwrap_or(0.0);

            out.push(TriggerSample {
                fund_code: code,
                as_of_date: d.clone(),
                features: vec![dd_mag, ret5, ret20, vol20],
                dip_buy_success,
                magic_rebound,
            });
        }
    }

    Ok(out)
}

fn drawdown_mag(navs: &[(String, f64)], idx: usize, lookback: usize) -> f64 {
    if navs.is_empty() || idx >= navs.len() {
        return 0.0;
    }
    let lookback = lookback.max(1).min(idx + 1);
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

fn max_nav(navs: &[(String, f64)], start: usize, end: usize) -> f64 {
    let mut max_v = f64::MIN;
    let end = end.min(navs.len().saturating_sub(1));
    if start > end {
        return 0.0;
    }
    for &(_, v) in navs.iter().take(end + 1).skip(start) {
        if v > max_v {
            max_v = v;
        }
    }
    if max_v == f64::MIN { 0.0 } else { max_v }
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
