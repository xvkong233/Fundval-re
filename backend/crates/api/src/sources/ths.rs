use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::eastmoney::{NavRow, RealtimeNavData};

pub fn dwjz_url(fund_code: &str) -> String {
    let code = fund_code.trim();
    format!("https://fund.10jqka.com.cn/{code}/json/jsondwjz.json")
}

pub fn latest_nav(rows: &[NavRow]) -> Option<RealtimeNavData> {
    let mut best: Option<&NavRow> = None;
    for row in rows {
        best = match best {
            None => Some(row),
            Some(current) => {
                if row.nav_date > current.nav_date {
                    Some(row)
                } else {
                    Some(current)
                }
            }
        };
    }

    best.map(|r| RealtimeNavData {
        fund_code: "".to_string(),
        nav: r.unit_nav,
        nav_date: r.nav_date,
    })
}

pub async fn fetch_nav_series(client: &reqwest::Client, fund_code: &str) -> Result<Vec<NavRow>, String> {
    let url = dwjz_url(fund_code);
    let referer = format!("https://fund.10jqka.com.cn/{}/", fund_code.trim());
    let text = client
        .get(url)
        .header(reqwest::header::REFERER, referer)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    parse_nav_series_js(&text)
}

pub async fn fetch_realtime_nav(
    client: &reqwest::Client,
    fund_code: &str,
) -> Result<Option<RealtimeNavData>, String> {
    let rows = fetch_nav_series(client, fund_code).await?;
    let mut latest = latest_nav(&rows);
    if let Some(ref mut v) = latest {
        v.fund_code = fund_code.trim().to_string();
    }
    Ok(latest)
}

pub fn parse_nav_series_js(text: &str) -> Result<Vec<NavRow>, String> {
    // 兼容多种返回：
    // - var dwjz_000001=[["2026-02-13","1.2345"], ...];
    // - [["2026-02-13","1.2345"], ...]
    // - {"data":[["2026-02-13","1.2345"], ...]}
    let trimmed = text.trim();
    let value: serde_json::Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => {
            let start = text.find('[').ok_or_else(|| "无法定位数组开始".to_string())?;
            let end = text.rfind(']').ok_or_else(|| "无法定位数组结束".to_string())?;
            if end <= start {
                return Err("数组范围无效".to_string());
            }
            let json_part = &text[start..=end];
            serde_json::from_str(json_part).map_err(|e| format!("JSON 解析失败: {e}"))?
        }
    };

    let items = if let Some(arr) = value.as_array() {
        arr
    } else if let Some(obj) = value.as_object() {
        obj.get("data")
            .and_then(|v| v.as_array())
            .or_else(|| obj.get("dwjz").and_then(|v| v.as_array()))
            .ok_or_else(|| "无法从对象中提取数组".to_string())?
    } else {
        return Err("返回值不是数组/对象".to_string());
    };

    let mut out: Vec<NavRow> = Vec::with_capacity(items.len());
    for item in items {
        let Some(pair) = item.as_array() else {
            continue;
        };
        if pair.len() < 2 {
            continue;
        }
        let date_str = pair[0].as_str().unwrap_or("").trim();
        let nav_str = pair[1].as_str().unwrap_or("").trim();
        if date_str.is_empty() || nav_str.is_empty() {
            continue;
        }

        let nav_date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|e| e.to_string())?;
        let unit_nav = Decimal::from_str_exact(nav_str).map_err(|e| e.to_string())?;

        out.push(NavRow {
            nav_date,
            unit_nav,
            accumulated_nav: None,
            daily_growth: None,
        });
    }

    Ok(out)
}
