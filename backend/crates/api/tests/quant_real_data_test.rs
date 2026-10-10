//! 真实数据训练验证：用 /home/hatch/workspace/fund_data/ 的真实净值+持仓训练16特征模型。

use api::ml::dataset::trade_profitable;
use api::ml::features::build_features;
use api::ml::logreg::{train_logreg, LogRegTrainConfig};
use api::quant::backtest::backtest_single_fund;
use api::quant::calibration::PlattCalibrator;
use api::quant::lookthrough::LookThroughFactors;
use api::quant::signal::{SignalGenerator, TradingSignal};
use api::quant::strategy::{Strategy, StrategyConfig};

#[derive(Debug, Clone)]
struct FundData {
    code: String,
    navs: Vec<(String, f64)>,
    lt_features: [f64; 3],
}

fn load_fund_data() -> Vec<FundData> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir("/home/hatch/workspace/fund_data").expect("read dir");
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.starts_with("nav_") || !name.ends_with(".json") {
            continue;
        }
        let code = name[4..name.len() - 5].to_string();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
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
        let lt_features = load_lookthrough(&code);
        out.push(FundData { code, navs, lt_features });
    }
    out.sort_by(|a, b| a.code.cmp(&b.code));
    out
}

fn load_lookthrough(code: &str) -> [f64; 3] {
    let path = format!("/home/hatch/workspace/fund_data/holdings_{code}.json");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    if text.is_empty() {
        return [0.0, 0.0, 0.0];
    }
    let items: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();
    let weights: Vec<f64> = items
        .iter()
        .filter_map(|h| h.get("weight_pct").and_then(|w| w.as_f64()))
        .collect();
    if weights.is_empty() {
        return [0.0, 0.0, 0.0];
    }
    let factors = LookThroughFactors::from_weights(&weights);
    let feats = factors.as_features();
    [feats[0], feats[1], feats[2]]
}

fn build_16features(navs: &[(String, f64)], idx: usize, lt: &[f64; 3]) -> Vec<f64> {
    let mut f = build_features(navs, idx);
    f.extend_from_slice(lt);
    f
}

#[test]
fn train_on_real_data() {
    let funds = load_fund_data();
    println!("\n加载 {} 只基金真实数据", funds.len());
    assert!(funds.len() >= 50, "至少需要50只基金");

    let avg_hhi: f64 = funds.iter().map(|f| f.lt_features[0]).sum::<f64>() / funds.len() as f64;
    println!("平均 HHI: {avg_hhi:.3}");

    // Pooled 训练：每只基金前70%训练
    let mut all_x: Vec<Vec<f64>> = Vec::new();
    let mut all_y: Vec<f64> = Vec::new();

    for fund in &funds {
        let n = fund.navs.len();
        let train_end = n * 70 / 100;
        let navs_f: Vec<f64> = fund.navs.iter().map(|(_, v)| *v).collect();
        let labeled: Vec<(String, f64)> = fund.navs.clone();
        for idx in 60..train_end {
            let x = build_16features(&fund.navs, idx, &fund.lt_features);
            let y = if trade_profitable(&labeled, idx, 20, 5.0, 2.5) { 1.0 } else { 0.0 };
            all_x.push(x);
            all_y.push(y);
        }
    }
    println!("训练样本: {} 个", all_x.len());

    let n_pos = all_y.iter().filter(|v| **v >= 0.5).count() as f64;
    let n_neg = all_y.len() as f64 - n_pos;
    let cfg = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 400,
        l2: 0.05,
        pos_weight: if n_pos > 0.0 { n_neg / n_pos } else { 1.0 },
    };
    let model = train_logreg(&all_x, &all_y, &cfg).expect("train");
    println!("训练完成，正样本比例: {:.1}%", n_pos / all_y.len() as f64 * 100.0);

    // 校准
    let probs: Vec<f64> = all_x.iter().map(|x| model.predict_proba(x).unwrap_or(0.5)).collect();
    let calibrator = PlattCalibrator::fit(&probs, &all_y).unwrap_or_else(PlattCalibrator::identity);
    let sig_gen = SignalGenerator {
        dip_buy_model: model.clone(),
        magic_rebound_model: model,
        dip_buy_calibrator: calibrator.clone(),
        magic_calibrator: calibrator,
        enter_threshold: 0.0,
    };

    // 概率分布诊断
    {
        let mut buckets = [0; 10];
        for fund in funds.iter().take(10) {
            let n = fund.navs.len();
            let test_start = n * 70 / 100;
            for i in (test_start..n).step_by(10) {
                let feat = build_16features(&fund.navs, i, &fund.lt_features);
                let raw = sig_gen.dip_buy_model.predict_proba(&feat).unwrap_or(0.5);
                let cal = sig_gen.dip_buy_calibrator.calibrate(raw);
                let b = ((cal * 10.0) as usize).min(9);
                buckets[b] += 1;
            }
        }
        println!("原始概率分布:");
        let mut raw_buckets = [0; 10];
        for fund in funds.iter().take(10) {
            let n = fund.navs.len();
            let test_start = n * 70 / 100;
            for i in (test_start..n).step_by(10) {
                let feat = build_16features(&fund.navs, i, &fund.lt_features);
                let raw = sig_gen.dip_buy_model.predict_proba(&feat).unwrap_or(0.5);
                let b = ((raw * 10.0) as usize).min(9);
                raw_buckets[b] += 1;
            }
        }
        for (i, c) in raw_buckets.iter().enumerate() {
            println!("  {:.1}-{:.1}: {}", i as f64 / 10.0, (i + 1) as f64 / 10.0, c);
        }
        println!("校准后概率分布:");
        for (i, c) in buckets.iter().enumerate() {
            println!("  {:.1}-{:.1}: {}", i as f64 / 10.0, (i + 1) as f64 / 10.0, c);
        }
    }

    // 每只基金后30%验证
    for thresh in [0.55, 0.60, 0.65, 0.70] {
        let mut qualified = 0;
        let mut passed = 0;
        let mut total_wr = 0.0;
        let mut total_trades = 0;
        let mut scfg = StrategyConfig::default();
        scfg.enter_threshold = thresh;
        let strategy = Strategy::new(scfg);

    for fund in &funds {
        let n = fund.navs.len();
        let test_start = n * 70 / 100;
        if n - test_start < 100 {
            continue;
        }
        let test_navs = &fund.navs[test_start..];
        // 生成信号
        let signals: Vec<Option<TradingSignal>> = (0..test_navs.len())
            .map(|i| {
                let feat = build_16features(test_navs, i, &fund.lt_features);
                sig_gen.generate(&test_navs[i].0, &fund.code, &feat, feat[3]).and_then(|sig| {
                    let combined = sig.dip_buy_proba * 0.6 + sig.magic_rebound_proba * 0.4;
                    if combined >= thresh { Some(sig) } else { None }
                })
            })
            .collect();
        let result = backtest_single_fund(&fund.code, test_navs, &|i| signals[i].clone(), &strategy, 100_000.0);

        let m = &result.metrics;
        if m.total_trades >= 3 {
            qualified += 1;
            total_wr += m.win_rate;
            total_trades += m.total_trades;
            if m.win_rate >= 0.80 {
                passed += 1;
            }
        }
    }

    println!("\n阈值 {thresh}: 可评估{qualified} 达标{passed} 平均胜率{:.1}% 总交易{total_trades}",
        if qualified > 0 { total_wr / qualified as f64 * 100.0 } else { 0.0 });
    }
}
