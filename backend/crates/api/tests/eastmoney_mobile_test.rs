//! 真实数据抓取验证：从东方财富移动端 API 获取基金净值历史与持仓。

use api::eastmoney::{
    build_client, fetch_fund_holdings, fetch_nav_history_mobile, fetch_stock_quotes_batch,
};

#[tokio::test]
async fn fetch_real_fund_data() {
    let client = build_client().expect("client");

    // 净值历史
    let navs = fetch_nav_history_mobile(&client, "000001")
        .await;
    if let Err(e) = &navs {
        println!("NAV ERR DEBUG: {e:?}");
    }
    let navs = navs.expect("nav history");
    println!("\n000001 净值条数: {}", navs.len());
    assert!(navs.len() > 500, "应有500+条净值");
    if let (Some(first), Some(last)) = (navs.first(), navs.last()) {
        println!("  {} {} -> {} {}", first.nav_date, first.unit_nav, last.nav_date, last.unit_nav);
    }

    // 持仓
    let holdings = fetch_fund_holdings(&client, "000001")
        .await
        .expect("holdings");
    println!("000001 持仓数: {}", holdings.len());
    assert!(!holdings.is_empty(), "应有持仓数据");
    let total_w: f64 = holdings.iter().map(|h| h.weight_pct).sum();
    println!("  前三大: {:?}", holdings.iter().take(3).map(|h| (&h.stock_code, &h.stock_name, h.weight_pct)).collect::<Vec<_>>());
    println!("  持仓权重合计: {total_w:.1}%");

    // 批量行情
    let secids: Vec<String> = holdings
        .iter()
        .take(5)
        .map(|h| format!("{}.{}", h.exchange, h.stock_code))
        .collect();
    let quotes = fetch_stock_quotes_batch(&client, &secids)
        .await
        .expect("quotes");
    println!("行情条数: {}", quotes.len());
    for q in quotes.iter().take(3) {
        println!("  {} {} 价格{:?} 涨跌{:?}", q.stock_code, q.stock_name, q.price, q.change_pct);
    }
}
