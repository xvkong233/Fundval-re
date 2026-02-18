use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::eastmoney::{NavRow, RealtimeNavData};

pub fn nav_history_url(fund_code: &str, page: i64, size: i64) -> String {
    let code = fund_code.trim();
    let p = page.max(1);
    let s = size.clamp(1, 5000);
    format!("https://danjuanapp.com/djapi/fund/nav/history/{code}?page={p}&size={s}")
}

#[derive(Debug, Deserialize)]
struct DanjuanNavHistoryResponse {
    data: Option<DanjuanNavHistoryData>,
    result_code: Option<i64>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DanjuanNavHistoryData {
    items: Vec<DanjuanNavHistoryItem>,
}

#[derive(Debug, Deserialize)]
struct DanjuanNavHistoryItem {
    date: String,
    nav: String,
    percentage: Option<String>,
}

pub fn parse_nav_history_json(text: &str) -> Result<Vec<NavRow>, String> {
    let resp: DanjuanNavHistoryResponse =
        serde_json::from_str(text).map_err(|e| format!("JSON 解析失败: {e}"))?;

    let code = resp.result_code.unwrap_or(-1);
    if code != 0 {
        let msg = resp
            .message
            .unwrap_or_else(|| "danjuan 上游返回错误".to_string());
        return Err(format!("danjuan 上游错误: code={code} msg={msg}"));
    }

    let data = resp.data.ok_or_else(|| "danjuan 响应缺少 data 字段".to_string())?;
    let mut out: Vec<NavRow> = Vec::with_capacity(data.items.len());

    for item in data.items {
        let nav_date =
            NaiveDate::parse_from_str(item.date.trim(), "%Y-%m-%d").map_err(|e| e.to_string())?;
        let unit_nav =
            Decimal::from_str_exact(item.nav.trim()).map_err(|e| format!("nav 解析失败: {e}"))?;
        let daily_growth = match item.percentage.as_deref().map(|s| s.trim()) {
            None | Some("") => None,
            Some(v) => Some(
                Decimal::from_str_exact(v).map_err(|e| format!("percentage 解析失败: {e}"))?,
            ),
        };

        out.push(NavRow {
            nav_date,
            unit_nav,
            accumulated_nav: None,
            daily_growth,
        });
    }

    Ok(out)
}

pub async fn fetch_nav_history(
    client: &reqwest::Client,
    fund_code: &str,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> Result<Vec<NavRow>, String> {
    // 说明：蛋卷的开放接口未必支持任意范围查询；这里采用“抓取较大第一页 + 本地过滤”的策略，
    // 在开箱即用前提下优先保证可用性。
    let url = nav_history_url(fund_code, 1, 5000);
    let text = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let mut rows = parse_nav_history_json(&text)?;

    // 上游通常是倒序；为了后续一致性这里不强制排序，只做范围过滤。
    if start_date.is_some() || end_date.is_some() {
        rows.retain(|r| {
            if let Some(sd) = start_date
                && r.nav_date < sd
            {
                return false;
            }
            if let Some(ed) = end_date
                && r.nav_date > ed
            {
                return false;
            }
            true
        });
    }

    Ok(rows)
}

pub async fn fetch_realtime_nav(
    client: &reqwest::Client,
    fund_code: &str,
) -> Result<Option<RealtimeNavData>, String> {
    let Some(row) = fetch_latest_row(client, fund_code).await? else {
        return Ok(None);
    };

    Ok(Some(RealtimeNavData {
        fund_code: fund_code.trim().to_string(),
        nav: row.unit_nav,
        nav_date: row.nav_date,
    }))
}

pub async fn fetch_latest_row(
    client: &reqwest::Client,
    fund_code: &str,
) -> Result<Option<NavRow>, String> {
    let url = nav_history_url(fund_code, 1, 1);
    let text = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let rows = parse_nav_history_json(&text)?;
    Ok(rows.into_iter().next())
}
