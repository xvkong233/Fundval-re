use chrono::{NaiveDate, NaiveDateTime, TimeZone, Utc};
use regex::Regex;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, REFERER};
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct EstimateData {
    pub fund_code: String,
    pub fund_name: String,
    pub estimate_nav: Decimal,
    pub estimate_growth: Decimal,
    pub estimate_time: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct RealtimeNavData {
    pub fund_code: String,
    pub nav: Decimal,
    pub nav_date: NaiveDate,
}

#[derive(Debug, Clone)]
pub struct FundGzSnapshot {
    pub fund_code: String,
    pub fund_name: Option<String>,
    pub latest_nav: Option<Decimal>,
    pub latest_nav_date: Option<NaiveDate>,
    pub estimate_nav: Option<Decimal>,
    pub estimate_growth: Option<Decimal>,
    pub estimate_time: Option<NaiveDateTime>,
    pub gztime_raw: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FundListItem {
    pub fund_code: String,
    pub fund_name: String,
    pub fund_type: String,
}

#[derive(Debug, Clone)]
pub struct NavRow {
    pub nav_date: NaiveDate,
    pub unit_nav: Decimal,
    pub accumulated_nav: Option<Decimal>,
    pub daily_growth: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct HoldingItem {
    pub stock_code: String,
    pub stock_name: String,
    /// 持仓占比（%）
    pub weight_pct: f64,
    /// 交易所前缀代码（如 "1"=上证，"0"=深证）
    pub exchange: String,
}

#[derive(Debug, Clone)]
pub struct StockQuote {
    pub stock_code: String,
    pub stock_name: String,
    pub price: Option<f64>,
    /// 涨跌幅（%）
    pub change_pct: Option<f64>,
}

fn extract_jsonpgz_payload(text: &str) -> Option<&str> {
    let text = text.trim();
    let start = text.find("jsonpgz(")? + "jsonpgz(".len();

    let tail = text[start..].trim_end();
    let end = if tail.ends_with(");") {
        tail.len().saturating_sub(2)
    } else if tail.ends_with(')') {
        tail.len().saturating_sub(1)
    } else {
        tail.rfind(')')?
    };

    let payload = tail[..end].trim();
    if payload.is_empty() {
        None
    } else {
        Some(payload)
    }
}

fn extract_jsonp_payload(text: &str) -> Option<&str> {
    let text = text.trim();
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close <= open {
        return None;
    }
    let payload = text[open + 1..close].trim();
    if payload.is_empty() { None } else { Some(payload) }
}

pub fn build_client() -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    headers.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
    );
    headers.insert(
        REFERER,
        HeaderValue::from_static("https://fund.eastmoney.com/"),
    );

    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        // 使用接近浏览器的 UA，降低被上游拦截概率
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
        .default_headers(headers);
    // 显式使用环境代理（本沙箱出口经代理，reqwest 默认可能未启用）
    for key in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
        if let Ok(proxy_url) = std::env::var(key) {
            if !proxy_url.trim().is_empty() {
                if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                    builder = builder.proxy(proxy);
                    break;
                }
            }
        }
    }
    builder.build().map_err(|e| e.to_string())
}

#[derive(Debug, Clone)]
pub struct IndexKlineRow {
    pub trade_date: NaiveDate,
    pub close: Decimal,
}

pub async fn fetch_index_kline_daily(
    client: &reqwest::Client,
    index_code: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<Vec<IndexKlineRow>, String> {
    let beg = start_date.format("%Y%m%d").to_string();
    let end = end_date.format("%Y%m%d").to_string();

    // Eastmoney kline daily (klt=101): JSONP if cb present, so we always pass cb and parse payload.
    let url = format!(
        "http://60.push2his.eastmoney.com/api/qt/stock/kline/get?secid={index_code}&fields1=f1,f2,f3,f4,f5&fields2=f51,f52,f53,f54,f55,f56,f57&klt=101&fqt=0&beg={beg}&end={end}&ut=fa5fd1943c7b386f172d6893dbfba10b&cb=cb"
    );
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

    let payload = extract_jsonp_payload(&text).unwrap_or(text.trim());
    let v: Value = serde_json::from_str(payload).map_err(|e| e.to_string())?;
    let klines = v
        .get("data")
        .and_then(|d| d.get("klines"))
        .and_then(|k| k.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out: Vec<IndexKlineRow> = Vec::with_capacity(klines.len());
    for item in klines {
        let Some(s) = item.as_str() else { continue; };
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() < 3 {
            continue;
        }
        let date = parts[0].trim();
        let close = parts[2].trim();
        let Ok(d) = NaiveDate::parse_from_str(date, "%Y-%m-%d") else { continue; };
        let Ok(c) = Decimal::from_str_exact(close) else { continue; };
        out.push(IndexKlineRow { trade_date: d, close: c });
    }

    Ok(out)
}

pub async fn fetch_estimate(
    client: &reqwest::Client,
    fund_code: &str,
) -> Result<Option<EstimateData>, String> {
    // 与原项目（Python requests）保持一致：使用 fundgz 的 jsonpgz JSONP
    let url = format!("http://fundgz.1234567.com.cn/js/{fund_code}.js");
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

    let Some(json_str) = extract_jsonpgz_payload(&text) else {
        return Ok(None);
    };

    let v: Value = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
    let fundcode = v
        .get("fundcode")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim();
    let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("").trim();
    let gsz = v.get("gsz").and_then(|x| x.as_str()).unwrap_or("").trim();
    let gszzl = v.get("gszzl").and_then(|x| x.as_str()).unwrap_or("").trim();
    let gztime = v
        .get("gztime")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim();

    if fundcode.is_empty()
        || name.is_empty()
        || gsz.is_empty()
        || gszzl.is_empty()
        || gztime.is_empty()
    {
        return Ok(None);
    }

    let estimate_nav = Decimal::from_str_exact(gsz).map_err(|e| e.to_string())?;
    let estimate_growth = Decimal::from_str_exact(gszzl).map_err(|e| e.to_string())?;
    let estimate_time =
        NaiveDateTime::parse_from_str(gztime, "%Y-%m-%d %H:%M").map_err(|e| e.to_string())?;

    Ok(Some(EstimateData {
        fund_code: fundcode.to_string(),
        fund_name: name.to_string(),
        estimate_nav,
        estimate_growth,
        estimate_time,
    }))
}

pub async fn fetch_fundgz_snapshot(
    client: &reqwest::Client,
    fund_code: &str,
) -> Result<Option<FundGzSnapshot>, String> {
    let url = format!("http://fundgz.1234567.com.cn/js/{fund_code}.js");
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

    let Some(json_str) = extract_jsonpgz_payload(&text) else {
        return Ok(None);
    };

    let v: Value = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
    let fundcode = v
        .get("fundcode")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim();
    if fundcode.is_empty() {
        return Ok(None);
    }

    let name = v.get("name").and_then(|x| x.as_str()).map(|s| s.trim().to_string());

    let latest_nav = v
        .get("dwjz")
        .and_then(|x| x.as_str())
        .and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Decimal::from_str_exact(t).ok()
            }
        });

    let latest_nav_date = v
        .get("jzrq")
        .and_then(|x| x.as_str())
        .and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                NaiveDate::parse_from_str(t, "%Y-%m-%d").ok()
            }
        });

    let estimate_nav = v
        .get("gsz")
        .and_then(|x| x.as_str())
        .and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Decimal::from_str_exact(t).ok()
            }
        });

    let estimate_growth = v
        .get("gszzl")
        .and_then(|x| x.as_str())
        .and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Decimal::from_str_exact(t).ok()
            }
        });

    let gztime_raw = v
        .get("gztime")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let estimate_time = gztime_raw
        .as_deref()
        .and_then(|s| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").ok());

    Ok(Some(FundGzSnapshot {
        fund_code: fundcode.to_string(),
        fund_name: name,
        latest_nav,
        latest_nav_date,
        estimate_nav,
        estimate_growth,
        estimate_time,
        gztime_raw,
    }))
}

pub async fn fetch_realtime_nav(
    client: &reqwest::Client,
    fund_code: &str,
) -> Result<Option<RealtimeNavData>, String> {
    let url = format!("http://fundgz.1234567.com.cn/js/{fund_code}.js");
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

    let Some(json_str) = extract_jsonpgz_payload(&text) else {
        return Ok(None);
    };

    let v: Value = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
    let fundcode = v
        .get("fundcode")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim();
    let dwjz = v.get("dwjz").and_then(|x| x.as_str()).unwrap_or("").trim();
    let jzrq = v.get("jzrq").and_then(|x| x.as_str()).unwrap_or("").trim();

    if fundcode.is_empty() || dwjz.is_empty() || jzrq.is_empty() {
        return Ok(None);
    }

    let nav = Decimal::from_str_exact(dwjz).map_err(|e| e.to_string())?;
    let nav_date = NaiveDate::parse_from_str(jzrq, "%Y-%m-%d").map_err(|e| e.to_string())?;

    Ok(Some(RealtimeNavData {
        fund_code: fundcode.to_string(),
        nav,
        nav_date,
    }))
}

pub async fn fetch_fund_list(client: &reqwest::Client) -> Result<Vec<FundListItem>, String> {
    let url = "http://fund.eastmoney.com/js/fundcode_search.js";
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

    let re = Regex::new(r"(?s)var\s+r\s*=\s*(\[.*\]);?").map_err(|e| e.to_string())?;
    let Some(caps) = re.captures(&text) else {
        return Ok(vec![]);
    };
    let json_str = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
    let v: Value = serde_json::from_str(json_str).map_err(|e| e.to_string())?;

    let Some(arr) = v.as_array() else {
        return Ok(vec![]);
    };
    let mut out: Vec<FundListItem> = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(item_arr) = item.as_array() else {
            continue;
        };
        if item_arr.len() < 4 {
            continue;
        }
        let code = item_arr[0].as_str().unwrap_or("").trim();
        let name = item_arr[2].as_str().unwrap_or("").trim();
        let fund_type = item_arr[3].as_str().unwrap_or("").trim();
        if code.is_empty() || name.is_empty() {
            continue;
        }
        out.push(FundListItem {
            fund_code: code.to_string(),
            fund_name: name.to_string(),
            fund_type: fund_type.to_string(),
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
    let url = format!("http://fund.eastmoney.com/pingzhongdata/{fund_code}.js");
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

    let unit_re =
        Regex::new(r"(?s)var\s+Data_netWorthTrend\s*=\s*(\[.*?\]);").map_err(|e| e.to_string())?;
    let Some(unit_caps) = unit_re.captures(&text) else {
        return Ok(vec![]);
    };
    let unit_json = unit_caps.get(1).unwrap().as_str();
    let unit_data: Value = serde_json::from_str(unit_json).map_err(|e| e.to_string())?;
    let unit_arr = unit_data.as_array().ok_or("unit nav data is not array")?;

    let acc_re =
        Regex::new(r"(?s)var\s+Data_ACWorthTrend\s*=\s*(\[.*?\]);").map_err(|e| e.to_string())?;
    let acc_arr = acc_re
        .captures(&text)
        .and_then(|c| c.get(1))
        .and_then(|m| serde_json::from_str::<Value>(m.as_str()).ok())
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();

    let mut acc_map: HashMap<i64, Decimal> = HashMap::new();
    for item in acc_arr {
        if let Some(obj) = item.as_object() {
            let x = obj.get("x").and_then(|v| v.as_i64());
            let y = obj.get("y").and_then(|v| v.as_f64());
            if let (Some(x), Some(y)) = (x, y) {
                acc_map.insert(x, Decimal::from_f64_retain(y).unwrap_or(Decimal::ZERO));
            }
            continue;
        }
        if let Some(arr) = item.as_array()
            && arr.len() >= 2
        {
            let x = arr[0].as_i64();
            let y = arr[1].as_f64();
            if let (Some(x), Some(y)) = (x, y) {
                acc_map.insert(x, Decimal::from_f64_retain(y).unwrap_or(Decimal::ZERO));
            }
        }
    }

    let mut out: Vec<NavRow> = Vec::new();
    for item in unit_arr {
        let obj = match item.as_object() {
            None => continue,
            Some(v) => v,
        };
        let x_ms = match obj.get("x").and_then(|v| v.as_i64()) {
            None => continue,
            Some(v) => v,
        };
        let y = match obj.get("y") {
            None => continue,
            Some(v) => v,
        };

        let ts = Utc.timestamp_millis_opt(x_ms).single();
        let Some(ts) = ts else {
            continue;
        };
        let nav_date = ts.date_naive();

        if let Some(sd) = start_date
            && nav_date < sd
        {
            continue;
        }
        if let Some(ed) = end_date
            && nav_date > ed
        {
            continue;
        }

        let unit_nav = if let Some(s) = y.as_str() {
            Decimal::from_str_exact(s).unwrap_or(Decimal::ZERO)
        } else if let Some(f) = y.as_f64() {
            Decimal::from_f64_retain(f).unwrap_or(Decimal::ZERO)
        } else {
            Decimal::ZERO
        };

        let equity_return = obj.get("equityReturn");
        let daily_growth = match equity_return {
            None => None,
            Some(v) if v.is_null() => None,
            Some(v) if v.as_str().is_some() => Decimal::from_str_exact(v.as_str().unwrap()).ok(),
            Some(v) if v.as_f64().is_some() => Decimal::from_f64_retain(v.as_f64().unwrap()),
            _ => None,
        };

        let accumulated_nav = acc_map.get(&x_ms).cloned();

        out.push(NavRow {
            nav_date,
            unit_nav,
            accumulated_nav,
            daily_growth,
        });
    }

    Ok(out)
}

/// 移动端历史净值（上游 FundVal-Live 移植：Web pingzhongdata API 已失效，
/// 改用 FundMNewApi/FundMNHisNetList）。
pub async fn fetch_nav_history_mobile(
    client: &reqwest::Client,
    fund_code: &str,
) -> Result<Vec<NavRow>, String> {
    let url = "https://fundmobapi.eastmoney.com/FundMNewApi/FundMNHisNetList";
    let v: Value = client
        .get(url)
        .query(&[
            ("FCODE", fund_code.trim()),
            ("IsShareNet", "true"),
            ("MobileKey", "1"),
            ("appType", "ttjj"),
            ("appVersion", "6.2.8"),
            ("cToken", "1"),
            ("deviceid", "1"),
            ("pageIndex", "1"),
            ("pageSize", "100000"),
            ("plat", "Iphone"),
            ("product", "EFund"),
            ("serverVersion", "6.2.8"),
            ("uToken", "1"),
            ("userId", "1"),
            ("version", "6.2.8"),
        ])
        .header("User-Agent", "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X)")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<Value>()
        .await
        .map_err(|e| e.to_string())?;

    let mut out: Vec<NavRow> = Vec::new();
    let items = v.get("Datas").and_then(|d| d.as_array()).cloned().unwrap_or_default();
    for item in items {
        let date_str = item.get("FSRQ").and_then(|x| x.as_str()).unwrap_or("");
        let nav_str = item.get("DWJZ").and_then(|x| x.as_str()).unwrap_or("");
        if date_str.is_empty() || nav_str.is_empty() {
            continue;
        }
        let nav_date = match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };
        let unit_nav: Decimal = nav_str.trim().parse().unwrap_or(Decimal::ZERO);
        if unit_nav <= Decimal::ZERO {
            continue;
        }
        out.push(NavRow {
            nav_date,
            unit_nav,
            accumulated_nav: None,
            daily_growth: None,
        });
    }
    // API 返回倒序，改为升序
    out.sort_by(|a, b| a.nav_date.cmp(&b.nav_date));
    Ok(out)
}

/// 基金持仓（前十大重仓股，含权重；上游 FundVal-Live 移植）。
pub async fn fetch_fund_holdings(
    client: &reqwest::Client,
    fund_code: &str,
) -> Result<Vec<HoldingItem>, String> {
    let url = "https://fundmobapi.eastmoney.com/FundMNewApi/FundMNInverstPosition";
    let v: Value = client
        .get(url)
        .query(&[
            ("FCODE", fund_code.trim()),
            ("deviceid", "x"),
            ("plat", "Android"),
            ("product", "EFund"),
            ("version", "1.0.0"),
        ])
        .header("User-Agent", "Mozilla/5.0 (Linux; Android 10) AppleWebKit/537.36 Chrome/91.0.4472.120 Mobile Safari/537.36")
        .header("Referer", "https://fundmobapi.eastmoney.com/")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<Value>()
        .await
        .map_err(|e| e.to_string())?;

    let mut out: Vec<HoldingItem> = Vec::new();
    let stocks = v
        .get("Datas")
        .and_then(|d| d.get("fundStocks"))
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    for s in stocks {
        let code = s.get("GPDM").and_then(|x| x.as_str()).unwrap_or("").trim();
        let name = s.get("GPJC").and_then(|x| x.as_str()).unwrap_or("").trim();
        let weight = s
            .get("JZBL")
            .and_then(|x| x.as_str())
            .and_then(|x| x.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        let exch = s.get("NEWTEXCH").and_then(|x| x.as_str()).unwrap_or("").trim();
        if code.is_empty() {
            continue;
        }
        out.push(HoldingItem {
            stock_code: code.to_string(),
            stock_name: name.to_string(),
            weight_pct: weight,
            exchange: exch.to_string(),
        });
    }
    Ok(out)
}

/// 批量股票行情（push2 ulist.np；上游 FundVal-Live 移植）。
/// `secids`: 如 ["1.600519", "0.300308"]（前缀为交易所代码）。
pub async fn fetch_stock_quotes_batch(
    client: &reqwest::Client,
    secids: &[String],
) -> Result<Vec<StockQuote>, String> {
    if secids.is_empty() {
        return Ok(vec![]);
    }
    // push2 单次建议不超过 200 个
    let mut out: Vec<StockQuote> = Vec::with_capacity(secids.len());
    for chunk in secids.chunks(200) {
        let url = "https://push2.eastmoney.com/api/qt/ulist.np/get";
        let v: Value = client
            .get(url)
            .query(&[
                ("secids", chunk.join(",").as_str()),
                ("fields", "f12,f14,f2,f3"),
                ("fltt", "2"),
            ])
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<Value>()
            .await
            .map_err(|e| e.to_string())?;
        let diff = v
            .get("data")
            .and_then(|d| d.get("diff"))
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        for item in diff {
            let code = item.get("f12").and_then(|x| x.as_str()).unwrap_or("");
            let name = item.get("f14").and_then(|x| x.as_str()).unwrap_or("");
            let price = item.get("f2").and_then(|x| {
                if x.is_string() {
                    x.as_str().and_then(|s| if s == "-" { None } else { s.parse::<f64>().ok() })
                } else {
                    x.as_f64()
                }
            });
            let chg = item.get("f3").and_then(|x| {
                if x.is_string() {
                    x.as_str().and_then(|s| if s == "-" { None } else { s.parse::<f64>().ok() })
                } else {
                    x.as_f64()
                }
            });
            out.push(StockQuote {
                stock_code: code.to_string(),
                stock_name: name.to_string(),
                price,
                change_pct: chg,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::extract_jsonpgz_payload;

    #[test]
    fn jsonpgz_payload_handles_parentheses_in_string() {
        let text = r#"jsonpgz({"fundcode":"161725","name":"招商中证白酒指数(LOF)A","jzrq":"2026-02-12","dwjz":"0.7035","gsz":"0.7034","gszzl":"-0.01","gztime":"2026-02-13 15:00"});"#;
        let payload = extract_jsonpgz_payload(text).expect("payload");
        let v: serde_json::Value = serde_json::from_str(payload).expect("valid json");
        assert_eq!(v.get("fundcode").and_then(|x| x.as_str()), Some("161725"));
        assert_eq!(
            v.get("name").and_then(|x| x.as_str()),
            Some("招商中证白酒指数(LOF)A")
        );
    }

    #[test]
    fn jsonpgz_payload_works_without_semicolon_and_whitespace() {
        let text = "  jsonpgz({\"a\":\"b(c)d\"}) \n";
        let payload = extract_jsonpgz_payload(text).expect("payload");
        let v: serde_json::Value = serde_json::from_str(payload).expect("valid json");
        assert_eq!(v.get("a").and_then(|x| x.as_str()), Some("b(c)d"));
    }
}
