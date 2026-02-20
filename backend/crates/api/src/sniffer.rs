use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::time::Duration;

use chrono::{Datelike, NaiveDate, Utc};
use csv::StringRecord;
use reqwest::Client;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Row};
use uuid::Uuid;

use crate::state::AppState;

pub const DEEPQ_STAR_CSV_URL: &str = "https://sq.deepq.tech/star/api/data";
pub const SNIFFER_WATCHLIST_NAME: &str = "嗅探（自动）";

const TZ_OFFSET_SECONDS: i32 = 8 * 60 * 60; // Asia/Shanghai fixed offset (+08:00)
const DAILY_HOUR: u32 = 3;
const DAILY_MINUTE: u32 = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct SnifferRow {
    pub sector: String,
    pub fund_code: String,
    pub fund_name: String,
    pub star_count: Option<i32>,
    pub tags: Vec<String>,
    pub week_growth: Option<Decimal>,
    pub year_growth: Option<Decimal>,
    pub max_drawdown: Option<Decimal>,
    pub fund_size_text: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone)]
pub struct SnifferSyncReport {
    pub run_id: Uuid,
    pub snapshot_id: Uuid,
    pub item_count: i32,
    pub users_updated: i32,
}

fn normalize_header(s: &str) -> String {
    s.trim().trim_start_matches('\u{feff}').to_string()
}

fn parse_percent_decimal(s: &str) -> Option<Decimal> {
    let s = s.trim().trim_end_matches('%').trim();
    if s.is_empty() {
        return None;
    }
    Decimal::from_str(s).ok()
}

fn parse_star_count(s: &str) -> Option<i32> {
    let n = s.chars().filter(|c| *c == '★').count();
    if n == 0 { None } else { Some(n as i32) }
}

fn split_tags(s: &str) -> Vec<String> {
    const TAG_SEPARATORS: &[char] = &['、', ',', '，', ';', '；', '|', '/', ' '];
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for part in s.split(TAG_SEPARATORS) {
        let t = part.trim();
        if t.is_empty() {
            continue;
        }
        if seen.insert(t.to_string()) {
            out.push(t.to_string());
        }
    }
    out
}

fn pick_year_growth_header(headers: &[String]) -> Option<String> {
    // Prefer "YYYY年涨幅" if present; otherwise fallback to "今年涨幅" (if upstream ever changes).
    for h in headers {
        if h.len() == "2025年涨幅".len()
            && h.ends_with("年涨幅")
            && h.chars().take(4).all(|c| c.is_ascii_digit())
        {
            return Some(h.clone());
        }
    }
    if headers.iter().any(|h| h == "今年涨幅") {
        return Some("今年涨幅".to_string());
    }
    None
}

#[derive(Clone, Copy)]
struct CsvIndices {
    sector_i: usize,
    name_i: usize,
    code_i: usize,
    week_i: Option<usize>,
    year_i: Option<usize>,
    drawdown_i: Option<usize>,
    size_i: Option<usize>,
    star_i: Option<usize>,
    tags_i: Option<usize>,
}

pub fn parse_deepq_csv(text: &str) -> Result<Vec<SnifferRow>, String> {
    let text = text.trim_start_matches('\u{feff}');
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = rdr
        .headers()
        .map_err(|e| format!("csv 读取 header 失败: {e}"))?
        .iter()
        .map(normalize_header)
        .collect::<Vec<_>>();

    let mut idx: HashMap<String, usize> = HashMap::new();
    for (i, h) in headers.iter().enumerate() {
        idx.insert(h.clone(), i);
    }

    let get_idx = |name: &str| idx.get(name).copied();
    let Some(sector_i) = get_idx("板块") else {
        return Err("csv 缺少列：板块".to_string());
    };
    let Some(name_i) = get_idx("基金名称") else {
        return Err("csv 缺少列：基金名称".to_string());
    };
    let Some(code_i) = get_idx("基金代码") else {
        return Err("csv 缺少列：基金代码".to_string());
    };
    let week_i = get_idx("近1周涨幅");
    let year_h = pick_year_growth_header(&headers);
    let year_i = year_h.as_deref().and_then(get_idx);
    let drawdown_i = get_idx("今年最大回撤");
    let size_i = get_idx("基金规模");
    let star_i = get_idx("评分星级");
    let tags_i = get_idx("特色标签");

    let indices = CsvIndices {
        sector_i,
        name_i,
        code_i,
        week_i,
        year_i,
        drawdown_i,
        size_i,
        star_i,
        tags_i,
    };

    let mut rows: Vec<SnifferRow> = Vec::new();
    let mut seen_codes: HashSet<String> = HashSet::new();

    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("csv 读取记录失败: {e}"))?;
        if let Some(row) = parse_record(&rec, &indices)
            && seen_codes.insert(row.fund_code.clone())
        {
            rows.push(row);
        }
    }

    rows.sort_by(|a, b| {
        a.sector
            .cmp(&b.sector)
            .then_with(|| b.star_count.unwrap_or(-1).cmp(&a.star_count.unwrap_or(-1)))
            .then_with(|| {
                b.week_growth
                    .unwrap_or(Decimal::MIN)
                    .cmp(&a.week_growth.unwrap_or(Decimal::MIN))
            })
            .then_with(|| a.fund_code.cmp(&b.fund_code))
    });

    Ok(rows)
}

fn get_field(rec: &StringRecord, idx: usize) -> &str {
    rec.get(idx).unwrap_or("")
}

fn parse_record(rec: &StringRecord, idx: &CsvIndices) -> Option<SnifferRow> {
    let sector = get_field(rec, idx.sector_i).trim();
    let fund_name = get_field(rec, idx.name_i).trim();
    let fund_code = get_field(rec, idx.code_i).trim();
    if sector.is_empty() || fund_code.is_empty() || fund_name.is_empty() {
        return None;
    }

    let week_growth = idx
        .week_i
        .and_then(|i| parse_percent_decimal(get_field(rec, i)));
    let year_growth = idx
        .year_i
        .and_then(|i| parse_percent_decimal(get_field(rec, i)));
    let max_drawdown = idx
        .drawdown_i
        .and_then(|i| parse_percent_decimal(get_field(rec, i)));
    let fund_size_text = idx
        .size_i
        .map(|i| get_field(rec, i).trim().to_string())
        .filter(|s| !s.is_empty());
    let star_count = idx.star_i.and_then(|i| parse_star_count(get_field(rec, i)));
    let tags = idx
        .tags_i
        .map(|i| split_tags(get_field(rec, i)))
        .unwrap_or_default();

    let raw = json!({
        "sector": sector,
        "fund_name": fund_name,
        "fund_code": fund_code,
        "week_growth": week_growth.map(|v| v.to_string()),
        "year_growth": year_growth.map(|v| v.to_string()),
        "max_drawdown": max_drawdown.map(|v| v.to_string()),
        "fund_size_text": fund_size_text.clone(),
        "star_count": star_count,
        "tags": tags.clone(),
    });

    Some(SnifferRow {
        sector: sector.to_string(),
        fund_code: fund_code.to_string(),
        fund_name: fund_name.to_string(),
        star_count,
        tags,
        week_growth,
        year_growth,
        max_drawdown,
        fund_size_text,
        raw,
    })
}

pub fn next_daily_run_utc(now_utc: chrono::DateTime<Utc>) -> chrono::DateTime<Utc> {
    let local = now_utc + chrono::Duration::seconds(TZ_OFFSET_SECONDS as i64);
    let today = NaiveDate::from_ymd_opt(local.year(), local.month(), local.day())
        .unwrap_or_else(|| Utc::now().date_naive());
    let target_today = today
        .and_hms_opt(DAILY_HOUR, DAILY_MINUTE, 0)
        .unwrap_or_else(|| today.and_hms_opt(3, 10, 0).unwrap());
    let next_local = if local.naive_utc() < target_today {
        target_today
    } else {
        (today.succ_opt().unwrap_or(today))
            .and_hms_opt(DAILY_HOUR, DAILY_MINUTE, 0)
            .unwrap()
    };
    chrono::DateTime::<Utc>::from_naive_utc_and_offset(
        next_local - chrono::Duration::seconds(TZ_OFFSET_SECONDS as i64),
        Utc,
    )
}

pub async fn run_scheduler_forever(state: AppState) {
    loop {
        let now = Utc::now();
        let next = next_daily_run_utc(now);
        let dur = match (next - now).to_std() {
            Ok(d) => d,
            Err(_) => Duration::from_secs(0),
        };
        tracing::info!(next_run_utc=%next.to_rfc3339(), sleep_seconds=?dur.as_secs(), "sniffer scheduler sleeping");
        tokio::time::sleep(dur).await;

        match run_sync_once(state.clone()).await {
            Ok(r) => {
                tracing::info!(run_id=%r.run_id, snapshot_id=%r.snapshot_id, item_count=r.item_count, users_updated=r.users_updated, "sniffer sync ok");
            }
            Err(e) => {
                tracing::warn!(error=%e, "sniffer sync failed");
            }
        }
    }
}

async fn fetch_deepq_csv(client: &Client) -> Result<String, String> {
    let resp = client
        .get(DEEPQ_STAR_CSV_URL)
        .send()
        .await
        .map_err(|e| format!("请求 DeepQ 数据失败: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("DeepQ 上游返回非 200: status={status}"));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取 DeepQ 响应失败: {e}"))?;
    Ok(text)
}

pub async fn run_sync_once(state: AppState) -> Result<SnifferSyncReport, String> {
    let _guard = state.sniffer_lock().lock().await;
    run_sync_once_unlocked(&state).await
}

async fn run_sync_once_unlocked(state: &AppState) -> Result<SnifferSyncReport, String> {
    let pool = state
        .pool()
        .ok_or_else(|| "database not configured".to_string())?;

    let run_id = Uuid::new_v4();
    let _ = sqlx::query(
        r#"
        INSERT INTO sniffer_run (id, source_url, started_at, ok)
        VALUES ($1, $2, NOW(), FALSE)
        "#,
    )
    .bind(run_id)
    .bind(DEEPQ_STAR_CSV_URL)
    .execute(pool)
    .await;

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Fundval-re sniffer/1.0")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let csv_text = match fetch_deepq_csv(&client).await {
        Ok(v) => v,
        Err(e) => {
            let _ = mark_run_failed(pool, run_id, &e).await;
            return Err(e);
        }
    };

    let rows = match parse_deepq_csv(&csv_text) {
        Ok(v) => v,
        Err(e) => {
            let _ = mark_run_failed(pool, run_id, &e).await;
            return Err(e);
        }
    };

    let snapshot_id = Uuid::new_v4();
    let item_count = rows.len() as i32;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("开启事务失败: {e}"))?;

    sqlx::query(
        r#"
        INSERT INTO sniffer_snapshot (id, source_url, fetched_at, item_count, run_id)
        VALUES ($1, $2, NOW(), $3, $4)
        "#,
    )
    .bind(snapshot_id)
    .bind(DEEPQ_STAR_CSV_URL)
    .bind(item_count)
    .bind(run_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("写入 sniffer_snapshot 失败: {e}"))?;

    let mut fund_ids: Vec<Uuid> = Vec::with_capacity(rows.len());

    for r in &rows {
        let fund_id: Uuid = sqlx::query(
            r#"
            INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
            VALUES ($1, $2, $3, NULL, NOW(), NOW())
            ON CONFLICT (fund_code) DO UPDATE
              SET fund_name = EXCLUDED.fund_name,
                  updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(&r.fund_code)
        .bind(&r.fund_name)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("upsert fund 失败: {e}"))?
        .get("id");
        fund_ids.push(fund_id);

        sqlx::query(
            r#"
            INSERT INTO sniffer_item (
              id, snapshot_id, fund_id, sector, tags, star_count,
              week_growth, year_growth, max_drawdown, fund_size_text, raw, created_at
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,NOW())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(snapshot_id)
        .bind(fund_id)
        .bind(&r.sector)
        .bind(&r.tags)
        .bind(r.star_count)
        .bind(r.week_growth)
        .bind(r.year_growth)
        .bind(r.max_drawdown)
        .bind(&r.fund_size_text)
        .bind(Json(&r.raw))
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("写入 sniffer_item 失败: {e}"))?;
    }

    let user_rows = sqlx::query("SELECT id FROM auth_user ORDER BY id ASC")
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| format!("读取用户列表失败: {e}"))?;
    let user_ids: Vec<i64> = user_rows
        .into_iter()
        .map(|r| r.get::<i64, _>("id"))
        .collect();

    let mut users_updated: i32 = 0;
    for user_id in &user_ids {
        let watchlist_id: Uuid = sqlx::query(
            r#"
            INSERT INTO watchlist (id, user_id, name, created_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (user_id, name) DO UPDATE
              SET name = EXCLUDED.name
            RETURNING id
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(*user_id)
        .bind(SNIFFER_WATCHLIST_NAME)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("创建/获取 watchlist 失败: {e}"))?
        .get("id");

        sqlx::query("DELETE FROM watchlist_item WHERE watchlist_id = $1")
            .bind(watchlist_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("清空 watchlist_item 失败: {e}"))?;

        if !fund_ids.is_empty() {
            let now = Utc::now();
            let mut qb: sqlx::QueryBuilder<Postgres> = sqlx::QueryBuilder::new(
                "INSERT INTO watchlist_item (id, watchlist_id, fund_id, \"order\", created_at) ",
            );
            qb.push_values(fund_ids.iter().enumerate(), |mut b, (i, fund_id)| {
                b.push_bind(Uuid::new_v4())
                    .push_bind(watchlist_id)
                    .push_bind(*fund_id)
                    .push_bind(i as i32)
                    .push_bind(now);
            });
            qb.build()
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("批量写入 watchlist_item 失败: {e}"))?;
        }

        users_updated += 1;
    }

    tx.commit()
        .await
        .map_err(|e| format!("提交事务失败: {e}"))?;

    let _ = sqlx::query(
        r#"
        UPDATE sniffer_run
        SET finished_at = NOW(),
            ok = TRUE,
            item_count = $2,
            snapshot_id = $3
        WHERE id = $1
        "#,
    )
    .bind(run_id)
    .bind(item_count)
    .bind(snapshot_id)
    .execute(pool)
    .await;

    Ok(SnifferSyncReport {
        run_id,
        snapshot_id,
        item_count,
        users_updated,
    })
}

async fn mark_run_failed(pool: &PgPool, run_id: Uuid, error: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE sniffer_run
        SET finished_at = NOW(),
            ok = FALSE,
            error = $2
        WHERE id = $1
        "#,
    )
    .bind(run_id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn latest_snapshot_id(pool: &PgPool) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query("SELECT id FROM sniffer_snapshot ORDER BY fetched_at DESC LIMIT 1")
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get::<Uuid, _>("id")))
}
