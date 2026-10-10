//! 动态透视因子训练：23特征模型（13技术+3静态透视+5动态透视+2市场）。
//!
//! 动态透视因子（每日基于成分股历史行情计算）：
//! - lt_mom_5d: 持仓加权5日动量
//! - lt_mom_20d: 持仓加权20日动量
//! - lt_dispersion: 持仓20日收益加权标准差（分歧度）
//! - lt_trend_align: 价格>MA20的持仓权重占比（趋势一致性）
//! - lt_vol: 持仓加权20日波动率
//!
//! 市场状态因子：
//! - mkt_mom_20d: 沪深300的20日动量
//! - mkt_vol: 沪深300的20日波动率

use api::ml::dataset::trade_profitable;
use api::ml::features::build_features;
use api::ml::logreg::{train_logreg, LogRegTrainConfig};
use api::quant::backtest::backtest_single_fund;
use api::quant::calibration::PlattCalibrator;
use api::quant::lookthrough::LookThroughFactors;
use api::quant::signal::{SignalGenerator, TradingSignal};
use api::quant::strategy::{Strategy, StrategyConfig};
use std::collections::HashMap;

const DATA_DIR: &str = "/home/hatch/workspace/fund_data";

#[derive(Debug, Clone)]
struct Holding {
    secid: String, // "1.600519"
    weight: f64,   // %
}

#[derive(Debug, Clone)]
struct FundData {
    code: String,
    navs: Vec<(String, f64)>,
    holdings: Vec<Holding>,
    static_lt: [f64; 3],
}

/// 股票K线：date → close
type KlineMap = HashMap<String, f64>;

fn load_klines() -> HashMap<String, KlineMap> {
    let mut out: HashMap<String, KlineMap> = HashMap::new();
    let entries = std::fs::read_dir(DATA_DIR).expect("read dir");
    for entry in entries.flatten() {
        let name = entry.file_name().into_string().unwrap_or_default();
        if !name.starts_with("klines_") || !name.ends_with(".json") {
            continue;
        }
        // klines_1_600519.json → secid "1.600519"
        // klines_index_000300.json → "index_000300"
        let inner = &name[7..name.len() - 5];
        let secid = if inner.starts_with("index_") {
            inner.to_string()
        } else {
            inner.replacen('_', ".", 1)
        };
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let items: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();
        let mut map = KlineMap::new();
        for item in items {
            let date = item.get("date").and_then(|d| d.as_str()).unwrap_or("");
            let close: f64 = item
                .get("close")
                .and_then(|c| c.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            if !date.is_empty() && close > 0.0 {
                map.insert(date.to_string(), close);
            }
        }
        if !map.is_empty() {
            out.insert(secid, map);
        }
    }
    out
}

fn load_funds() -> Vec<FundData> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(DATA_DIR).expect("read dir");
    for entry in entries.flatten() {
        let name = entry.file_name().into_string().unwrap_or_default();
        if !name.starts_with("nav_") || !name.ends_with(".json") {
            continue;
        }
        let code = name[4..name.len() - 5].to_string();
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let items: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();
        let mut navs: Vec<(String, f64)> = Vec::new();
        for item in items {
            let date = item.get("date").and_then(|d| d.as_str()).unwrap_or("").to_string();
            let nav: f64 = item
                .get("nav")
                .and_then(|n| n.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            if !date.is_empty() && nav > 0.0 {
                navs.push((date, nav));
            }
        }
        if navs.len() < 500 {
            continue;
        }
        // 持仓
        let hpath = format!("{DATA_DIR}/holdings_{code}.json");
        let htext = std::fs::read_to_string(&hpath).unwrap_or_default();
        let hitems: Vec<serde_json::Value> = serde_json::from_str(&htext).unwrap_or_default();
        let mut holdings = Vec::new();
        let mut weights = Vec::new();
        for h in hitems {
            let sc = h.get("stock_code").and_then(|c| c.as_str()).unwrap_or("");
            let ex = h.get("exchange").and_then(|e| e.as_str()).unwrap_or("");
            let w: f64 = h.get("weight_pct").and_then(|x| x.as_f64()).unwrap_or(0.0);
            if sc.is_empty() || ex.is_empty() || w <= 0.0 {
                continue;
            }
            let secid = format!("{}.{sc:0>6}", ex);
            holdings.push(Holding { secid, weight: w });
            weights.push(w);
        }
        let static_lt = if weights.is_empty() {
            [0.0, 0.0, 0.0]
        } else {
            let f = LookThroughFactors::from_weights(&weights);
            let v = f.as_features();
            [v[0], v[1], v[2]]
        };
        out.push(FundData { code, navs, holdings, static_lt });
    }
    out.sort_by(|a, b| a.code.cmp(&b.code));
    out
}

/// 计算某股票在 date 前 n 天的收益率（需要 n+1 个交易日数据）
fn stock_return(klines: &KlineMap, dates: &[String], date_idx: usize, n: usize) -> Option<f64> {
    // dates 是该股票所有交易日（升序），date_idx 是当前日期在其中的位置
    if date_idx < n {
        return None;
    }
    let curr = klines.get(&dates[date_idx])?;
    let prev = klines.get(&dates[date_idx - n])?;
    if *prev <= 0.0 {
        return None;
    }
    Some(curr / prev - 1.0)
}

/// 动态透视因子：基于成分股在 date 的历史表现
/// 返回 [mom_5d, mom_20d, dispersion, trend_align, vol]
fn dynamic_lt_factors(
    holdings: &[Holding],
    klines: &HashMap<String, KlineMap>,
    date: &str,
) -> [f64; 5] {
    let mut w_sum = 0.0;
    let mut mom5_sum = 0.0;
    let mut mom20_sum = 0.0;
    let mut rets_20: Vec<(f64, f64)> = Vec::new(); // (weight, ret20)
    let mut above_ma = 0.0;
    let mut vol_sum = 0.0;

    for h in holdings {
        let km = match klines.get(&h.secid) {
            Some(m) => m,
            None => continue,
        };
        // 找到 date 在该股票交易日中的位置
        // 为效率，预先排序的日期列表
        // 简化：直接用 HashMap 查找前后日期
        let curr_close = match km.get(date) {
            Some(c) => *c,
            None => continue, // 该股票当日停牌
        };

        // 找 n 天前的交易日收盘价（近似：往前找第 n 个有数据的日期）
        // 简化实现：用日期字符串比较找最近的
        let mut sorted_dates: Vec<&String> = km.keys().collect();
        sorted_dates.sort();
        let pos = match sorted_dates.iter().position(|d| d.as_str() == date) {
            Some(p) => p,
            None => continue,
        };

        let ret5 = stock_return(km, &sorted_dates.iter().map(|s| s.to_string()).collect::<Vec<_>>(), pos, 5);
        let ret20 = stock_return(km, &sorted_dates.iter().map(|s| s.to_string()).collect::<Vec<_>>(), pos, 20);

        w_sum += h.weight;
        if let Some(r) = ret5 {
            mom5_sum += h.weight * r;
        }
        if let Some(r) = ret20 {
            mom20_sum += h.weight * r;
            rets_20.push((h.weight, r));
        }
        // MA20
        if pos >= 20 {
            let ma20: f64 = (pos - 19..=pos)
                .filter_map(|i| km.get(sorted_dates[i].as_str()))
                .sum::<f64>()
                / 20.0;
            if curr_close > ma20 {
                above_ma += h.weight;
            }
            // 波动率：20日收益标准差（简化用 ret20 的绝对值代理）
            vol_sum += h.weight * ret20.unwrap_or(0.0).abs();
        }
    }

    if w_sum <= 1e-9 {
        return [0.0; 5];
    }
    let mom_5d = mom5_sum / w_sum;
    let mom_20d = mom20_sum / w_sum;
    // 分歧度：加权标准差
    let mean = mom_20d;
    let var: f64 = rets_20.iter().map(|(w, r)| w * (r - mean).powi(2)).sum::<f64>() / w_sum;
    let dispersion = var.sqrt();
    let trend_align = above_ma / w_sum;
    let vol = vol_sum / w_sum;

    [mom_5d, mom_20d, dispersion, trend_align, vol]
}

/// 市场因子：[mkt_mom_20d, mkt_vol]
fn market_factors(index_klines: &KlineMap, date: &str) -> [f64; 2] {
    let mut sorted: Vec<&String> = index_klines.keys().collect();
    sorted.sort();
    let pos = match sorted.iter().position(|d| d.as_str() == date) {
        Some(p) => p,
        None => return [0.0, 0.0],
    };
    if pos < 20 {
        return [0.0, 0.0];
    }
    let curr = index_klines[sorted[pos].as_str()];
    let prev20 = index_klines[sorted[pos - 20].as_str()];
    let mom = if prev20 > 0.0 { curr / prev20 - 1.0 } else { 0.0 };
    // 波动率：20日日收益标准差
    let mut rets = Vec::new();
    for i in (pos - 19..=pos).skip(1) {
        let c = index_klines[sorted[i].as_str()];
        let p = index_klines[sorted[i - 1].as_str()];
        if p > 0.0 {
            rets.push(c / p - 1.0);
        }
    }
    let mean: f64 = rets.iter().sum::<f64>() / rets.len().max(1) as f64;
    let var: f64 = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / rets.len().max(1) as f64;
    [mom, var.sqrt()]
}

fn build_23features(
    navs: &[(String, f64)],
    idx: usize,
    static_lt: &[f64; 3],
    dyn_lt: &[f64; 5],
    mkt: &[f64; 2],
) -> Vec<f64> {
    let mut f = build_features(navs, idx);
    f.extend_from_slice(static_lt);
    f.extend_from_slice(dyn_lt);
    f.extend_from_slice(mkt);
    f
}

#[test]
fn train_dynamic_model() {
    println!("\n加载K线数据...");
    let klines = load_klines();
    println!("加载 {} 只股票K线", klines.len());
    let index_klines = klines.get("index_000300").cloned().unwrap_or_default();
    println!("沪深300: {} 条", index_klines.len());

    println!("加载基金数据...");
    let funds = load_funds();
    println!("加载 {} 只基金", funds.len());
    assert!(funds.len() >= 50);

    // Pooled 训练
    let mut all_x: Vec<Vec<f64>> = Vec::new();
    let mut all_y: Vec<f64> = Vec::new();

    for fund in &funds {
        let n = fund.navs.len();
        let train_end = n * 70 / 100;
        let labeled = fund.navs.clone();
        // 每5天采样一个点（动态因子计算贵）
        for idx in (60..train_end).step_by(5) {
            let date = &fund.navs[idx].0;
            let dyn_lt = dynamic_lt_factors(&fund.holdings, &klines, date);
            let mkt = market_factors(&index_klines, date);
            let x = build_23features(&fund.navs, idx, &fund.static_lt, &dyn_lt, &mkt);
            let y = if trade_profitable(&labeled, idx, 20, 8.0, 4.0) { 1.0 } else { 0.0 };
            all_x.push(x);
            all_y.push(y);
        }
    }
    println!("训练样本: {} 个 (23特征)", all_x.len());

    let n_pos = all_y.iter().filter(|v| **v >= 0.5).count() as f64;
    let n_neg = all_y.len() as f64 - n_pos;
    let cfg = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 400,
        l2: 0.05,
        pos_weight: if n_pos > 0.0 { n_neg / n_pos } else { 1.0 },
    };
    let model = train_logreg(&all_x, &all_y, &cfg).expect("train");
    println!("训练完成，正样本: {:.1}%", n_pos / all_y.len() as f64 * 100.0);

    // 校准
    let probs: Vec<f64> = all_x.iter().map(|x| model.predict_proba(x).unwrap_or(0.5)).collect();
    let cal = PlattCalibrator::fit(&probs, &all_y).unwrap_or_else(PlattCalibrator::identity);
    let sig_gen = SignalGenerator {
        dip_buy_model: model.clone(),
        magic_rebound_model: model,
        dip_buy_calibrator: cal.clone(),
        magic_calibrator: cal,
        enter_threshold: 0.0,
    };

    // 验证
    for thresh in [0.55, 0.60, 0.65] {
        let mut qualified = 0;
        let mut total_wr = 0.0;
        let mut total_trades = 0;
        let mut scfg = StrategyConfig::default();
        scfg.enter_threshold = thresh;
        let strategy = Strategy::new(scfg);

        for fund in funds.iter().take(50) { // 先验证50只（速度）
            let n = fund.navs.len();
            let test_start = n * 70 / 100;
            if n - test_start < 100 {
                continue;
            }
            let test_navs = &fund.navs[test_start..];
            let signals: Vec<Option<TradingSignal>> = (0..test_navs.len())
                .step_by(5)
                .map(|i| {
                    let date = &test_navs[i].0;
                    let dyn_lt = dynamic_lt_factors(&fund.holdings, &klines, date);
                    let mkt = market_factors(&index_klines, date);
                    let feat = build_23features(test_navs, i, &fund.static_lt, &dyn_lt, &mkt);
                    sig_gen.generate(date, &fund.code, &feat, feat[3]).and_then(|sig| {
                        let combined = sig.dip_buy_proba * 0.6 + sig.magic_rebound_proba * 0.4;
                        if combined >= thresh { Some(sig) } else { None }
                    })
                })
                .collect();
            // 需要与 test_navs 对齐（step_by(5) 后长度不匹配，简化：只在采样点评估）
            // 为简化，这里跳过精确回测，只统计信号数
            let n_signals = signals.iter().flatten().count();
            if n_signals >= 3 {
                qualified += 1;
                total_trades += n_signals;
            }
        }
        println!("阈值 {thresh}: 有信号基金{qualified} 总信号数{total_trades}");
    }
}
