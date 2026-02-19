use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::eastmoney::{NavRow, RealtimeNavData};

const TUSHARE_API_URL: &str = "https://api.tushare.pro";

#[derive(Debug, Serialize)]
struct TushareRequest<T> {
    api_name: &'static str,
    token: String,
    params: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct TushareResponse {
    code: i64,
    msg: Option<String>,
    data: Option<TushareData>,
}

#[derive(Debug, Deserialize)]
struct TushareData {
    fields: Vec<String>,
    items: Vec<Vec<Value>>,
}

fn normalize_yyyymmdd(date: NaiveDate) -> String {
    date.format("%Y%m%d").to_string()
}

fn parse_date_any(input: &str) -> Option<NaiveDate> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) {
        return NaiveDate::parse_from_str(s, "%Y%m%d").ok();
    }
    if s.contains('-') {
        return NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
    }
    None
}

fn build_ts_code_candidates(fund_code: &str) -> Vec<String> {
    let code = fund_code.trim();
    if code.is_empty() {
        return vec![];
    }
    if code.contains('.') {
        return vec![code.to_string()];
    }
    vec![
        format!("{code}.OF"),
        format!("{code}.SZ"),
        format!("{code}.SH"),
        code.to_string(),
    ]
}

fn parse_nav_rows_from_data(data: &TushareData) -> Result<Vec<NavRow>, String> {
    let mut idx_date: Option<usize> = None;
    let mut idx_unit: Option<usize> = None;
    let mut idx_acc: Option<usize> = None;

    for (i, f) in data.fields.iter().enumerate() {
        let name = f.trim().to_ascii_lowercase();
        match name.as_str() {
            "nav_date" | "end_date" => idx_date = Some(i),
            "unit_nav" => idx_unit = Some(i),
            "accum_nav" => idx_acc = Some(i),
            _ => {}
        }
    }

    let (Some(i_date), Some(i_unit)) = (idx_date, idx_unit) else {
        return Err("tushare 返回缺少 nav_date/unit_nav 字段".to_string());
    };

    let mut out: Vec<NavRow> = Vec::with_capacity(data.items.len());
    for row in &data.items {
        let date_val = row.get(i_date).cloned().unwrap_or(Value::Null);
        let unit_val = row.get(i_unit).cloned().unwrap_or(Value::Null);
        let acc_val = idx_acc.and_then(|i| row.get(i).cloned());

        let date_str = date_val.as_str().unwrap_or("").trim();
        let Some(nav_date) = parse_date_any(date_str) else {
            continue;
        };

        let unit_str = match unit_val {
            Value::String(s) => s,
            Value::Number(n) => n.to_string(),
            _ => "".to_string(),
        };
        let unit_str = unit_str.trim();
        if unit_str.is_empty() {
            continue;
        }
        let unit_nav = Decimal::from_str_exact(unit_str).map_err(|e| format!("unit_nav 解析失败: {e}"))?;

        let accumulated_nav = match acc_val {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => {
                let t = s.trim();
                if t.is_empty() { None } else { Decimal::from_str_exact(t).ok() }
            }
            Some(Value::Number(n)) => Decimal::from_str_exact(&n.to_string()).ok(),
            _ => None,
        };

        out.push(NavRow {
            nav_date,
            unit_nav,
            accumulated_nav,
            daily_growth: None,
        });
    }

    Ok(out)
}

pub async fn fetch_nav_history(
    client: &reqwest::Client,
    token: &str,
    fund_code: &str,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> Result<Vec<NavRow>, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("tushare token 未配置".to_string());
    }

    let start = start_date.map(normalize_yyyymmdd);
    let end = end_date.map(normalize_yyyymmdd);

    #[derive(Debug, Serialize)]
    struct Params {
        ts_code: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        start_date: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        end_date: Option<String>,
    }

    for ts_code in build_ts_code_candidates(fund_code) {
        let mut offset: i64 = 0;
        let limit: i64 = 2000;
        let mut all: Vec<NavRow> = Vec::new();

        for _ in 0..10 {
            let req = TushareRequest {
                api_name: "fund_nav",
                token: token.to_string(),
                params: Params {
                    ts_code: ts_code.clone(),
                    start_date: start.clone(),
                    end_date: end.clone(),
                },
                fields: Some("ts_code,nav_date,unit_nav,accum_nav".to_string()),
                offset: Some(offset),
                limit: Some(limit),
            };

            let resp = client
                .post(TUSHARE_API_URL)
                .json(&req)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .json::<TushareResponse>()
                .await
                .map_err(|e| e.to_string())?;

            if resp.code != 0 {
                let msg = resp.msg.unwrap_or_else(|| "tushare 上游返回错误".to_string());
                return Err(format!("tushare 上游错误: code={} msg={}", resp.code, msg));
            }

            let Some(data) = resp.data else {
                break;
            };
            if data.items.is_empty() {
                break;
            }

            let rows = parse_nav_rows_from_data(&data)?;
            let got = rows.len() as i64;
            all.extend(rows);

            if got < limit {
                break;
            }
            offset += limit;
        }

        if !all.is_empty() {
            return Ok(all);
        }
    }

    Ok(vec![])
}

pub async fn fetch_realtime_nav(
    client: &reqwest::Client,
    token: &str,
    fund_code: &str,
) -> Result<Option<RealtimeNavData>, String> {
    let rows = fetch_nav_history(client, token, fund_code, None, None).await?;
    let mut best: Option<&NavRow> = None;
    for row in &rows {
        best = match best {
            None => Some(row),
            Some(cur) => {
                if row.nav_date > cur.nav_date { Some(row) } else { Some(cur) }
            }
        };
    }
    Ok(best.map(|r| RealtimeNavData {
        fund_code: fund_code.trim().to_string(),
        nav: r.unit_nav,
        nav_date: r.nav_date,
    }))
}

#[cfg(test)]
mod tests {
    use super::{parse_nav_rows_from_data, TushareData};

    #[test]
    fn parses_rows_by_field_names() {
        let data = TushareData {
            fields: vec![
                "ts_code".into(),
                "nav_date".into(),
                "unit_nav".into(),
                "accum_nav".into(),
            ],
            items: vec![vec![
                serde_json::Value::String("161725.OF".into()),
                serde_json::Value::String("20260213".into()),
                serde_json::Value::String("0.7037".into()),
                serde_json::Value::String("2.4198".into()),
            ]],
        };
        let rows = parse_nav_rows_from_data(&data).expect("parse ok");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].nav_date.to_string(), "2026-02-13");
        assert_eq!(rows[0].unit_nav.to_string(), "0.7037");
        assert_eq!(rows[0].accumulated_nav.as_ref().unwrap().to_string(), "2.4198");
    }
}

