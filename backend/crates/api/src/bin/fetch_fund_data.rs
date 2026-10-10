//! 真实基金数据抓取：获取200只股混基金的净值历史与持仓。
//!
//! 用法：cargo run -p api --bin fetch_fund_data
//! 输出：/home/hatch/workspace/fund_data/nav_<code>.json, /home/hatch/workspace/fund_data/holdings_<code>.json

use api::eastmoney::{
    build_client, fetch_fund_holdings, fetch_fund_list, fetch_nav_history_mobile,
};
use std::fs;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), String> {
    let client = build_client()?;
    fs::create_dir_all("/home/hatch/workspace/fund_data").map_err(|e| e.to_string())?;

    // 1. 获取基金列表，筛选股混型
    println!("获取基金列表...");
    let funds = fetch_fund_list(&client).await?;
    println!("共 {} 只基金", funds.len());

    let mut candidates: Vec<(String, String)> = Vec::new();
    for f in funds {
        let ft = f.fund_type.as_str();
        // 筛选：混合型、股票型（排除债券、货币、QDII、FOF）
        let is_target = (ft.contains("混合") || ft.contains("股票"))
            && !ft.contains("债券")
            && !ft.contains("货币")
            && !ft.contains("QDII");
        if is_target {
            candidates.push((f.fund_code.clone(), f.fund_name.clone()));
        }
    }
    println!("筛选出 {} 只股混基金", candidates.len());

    // 取前200只（按代码排序，保证确定性）
    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    let selected: Vec<_> = candidates.into_iter().take(200).collect();
    println!("选择 {} 只进行抓取", selected.len());

    // 保存基金清单
    let list_json = serde_json::to_string_pretty(
        &selected
            .iter()
            .map(|(c, n)| serde_json::json!({"code": c, "name": n}))
            .collect::<Vec<_>>(),
    )
    .map_err(|e| e.to_string())?;
    fs::write("/home/hatch/workspace/fund_data/fund_list.json", list_json).map_err(|e| e.to_string())?;

    // 2. 抓取净值历史
    let mut nav_ok = 0;
    let mut nav_fail = 0;
    for (i, (code, name)) in selected.iter().enumerate() {
        let path = format!("/home/hatch/workspace/fund_data/nav_{code}.json");
        if std::path::Path::new(&path).exists() {
            nav_ok += 1;
            continue; // 已抓取则跳过
        }
        match fetch_nav_history_mobile(&client, code).await {
            Ok(navs) => {
                if navs.len() >= 500 {
                    let data: Vec<_> = navs
                        .iter()
                        .map(|r| {
                            serde_json::json!({
                                "date": r.nav_date.to_string(),
                                "nav": r.unit_nav.to_string(),
                            })
                        })
                        .collect();
                    let json = serde_json::to_string(&data).map_err(|e| e.to_string())?;
                    fs::write(&path, json).map_err(|e| e.to_string())?;
                    nav_ok += 1;
                } else {
                    nav_fail += 1;
                }
            }
            Err(e) => {
                eprintln!("  {code} {name} 净值失败: {e}");
                nav_fail += 1;
            }
        }
        if (i + 1) % 20 == 0 {
            println!("  净值进度: {}/{} (成功{nav_ok} 失败{nav_fail})", i + 1, selected.len());
        }
        tokio::time::sleep(Duration::from_millis(200)).await; // 限速
    }
    println!("净值抓取完成: 成功{nav_ok} 失败{nav_fail}");

    // 3. 抓取持仓
    let mut hold_ok = 0;
    for (i, (code, _)) in selected.iter().enumerate() {
        let path = format!("/home/hatch/workspace/fund_data/holdings_{code}.json");
        if std::path::Path::new(&path).exists() {
            hold_ok += 1;
            continue;
        }
        match fetch_fund_holdings(&client, code).await {
            Ok(holdings) => {
                if !holdings.is_empty() {
                    let data: Vec<_> = holdings
                        .iter()
                        .map(|h| {
                            serde_json::json!({
                                "stock_code": h.stock_code,
                                "stock_name": h.stock_name,
                                "weight_pct": h.weight_pct,
                                "exchange": h.exchange,
                            })
                        })
                        .collect();
                    let json = serde_json::to_string(&data).map_err(|e| e.to_string())?;
                    fs::write(&path, json).map_err(|e| e.to_string())?;
                    hold_ok += 1;
                }
            }
            Err(_) => {}
        }
        if (i + 1) % 20 == 0 {
            println!("  持仓进度: {}/{} (成功{hold_ok})", i + 1, selected.len());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    println!("持仓抓取完成: 成功{hold_ok}");

    println!("\n完成！数据在 /home/hatch/workspace/fund_data/");
    Ok(())
}
