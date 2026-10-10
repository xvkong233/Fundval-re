//! 抓取成分股历史K线：为动态透视因子准备数据。
//!
//! 用法：cargo run -p api --bin fetch_stock_klines
//! 输出：/home/hatch/workspace/fund_data/klines_<secid>.json

use api::eastmoney::{build_client, fetch_index_kline_daily};
use chrono::NaiveDate;
use std::collections::HashSet;
use std::fs;

#[tokio::main]
async fn main() -> Result<(), String> {
    let client = build_client()?;
    fs::create_dir_all("/home/hatch/workspace/fund_data").map_err(|e| e.to_string())?;

    // 1. 收集所有成分股 secid
    let mut secids: HashSet<String> = HashSet::new();
    let entries = fs::read_dir("/home/hatch/workspace/fund_data").map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.starts_with("holdings_") || !name.ends_with(".json") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let items: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();
        for h in items {
            let code = h.get("stock_code").and_then(|c| c.as_str()).unwrap_or("");
            let exch = h.get("exchange").and_then(|e| e.as_str()).unwrap_or("");
            if code.is_empty() || exch.is_empty() {
                continue;
            }
            // secid 格式：1.600519（上证）/ 0.000001（深证）
            // exchange: "1"=上证，"0"=深证；需要补全为6位代码
            let code_padded = format!("{:0>6}", code);
            secids.insert(format!("{exch}.{code_padded}"));
        }
    }
    println!("去重后 {} 只股票", secids.len());

    // 2. 抓取K线（2015-01-01 至今）
    let start = NaiveDate::from_ymd_opt(2015, 1, 1).unwrap();
    let end = chrono::Local::now().date_naive();
    let mut ok = 0;
    let mut fail = 0;
    let mut secid_list: Vec<String> = secids.into_iter().collect();
    secid_list.sort();

    for (i, secid) in secid_list.iter().enumerate() {
        let safe_name = secid.replace('.', "_");
        let path = format!("/home/hatch/workspace/fund_data/klines_{safe_name}.json");
        if std::path::Path::new(&path).exists() {
            ok += 1;
            continue;
        }
        match fetch_index_kline_daily(&client, secid, start, end).await {
            Ok(klines) => {
                if klines.len() >= 100 {
                    let data: Vec<_> = klines
                        .iter()
                        .map(|k| {
                            serde_json::json!({
                                "date": k.trade_date.to_string(),
                                "close": k.close.to_string(),
                            })
                        })
                        .collect();
                    let json = serde_json::to_string(&data).map_err(|e| e.to_string())?;
                    fs::write(&path, json).map_err(|e| e.to_string())?;
                    ok += 1;
                } else {
                    fail += 1;
                }
            }
            Err(e) => {
                eprintln!("  {secid} 失败: {e}");
                fail += 1;
            }
        }
        if (i + 1) % 50 == 0 {
            println!("  进度: {}/{} (成功{ok} 失败{fail})", i + 1, secid_list.len());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    println!("\n完成！成功{ok} 失败{fail}");
    Ok(())
}
