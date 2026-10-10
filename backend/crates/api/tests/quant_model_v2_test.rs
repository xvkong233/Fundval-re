//! 模型 v2 改进验证：提高真实胜率。
//!
//! 四个改进方向：
//! 1. 加长因子：加入 IC 测试中最强的长周期因子（dist_ma250/mom_12m/sharpe_252）
//! 2. 趋势过滤：只在价格 > MA250（上升趋势）中买跌，下跌趋势不入场
//! 3. 波动过滤：vol_20 超过历史 90 分位时放弃（极端波动不可预测）
//! 4. Bagging 集成：5 个自助采样模型平均，降方差
//!
//! 对比基线 v1（13特征+3透视，单模型，无过滤）。

use api::ml::dataset::trade_profitable;
use api::ml::features::build_features;
use api::ml::logreg::{train_logreg, LogRegModel, LogRegTrainConfig};
use api::quant::backtest::backtest_single_fund;
use api::quant::calibration::PlattCalibrator;
use api::quant::lookthrough::LookThroughFactors;
use api::quant::signal::{SignalGenerator, TradingSignal};
use api::quant::strategy::{Strategy, StrategyConfig};

const DATA_DIR: &str = "/home/hatch/workspace/fund_data";

// ── v2 特征：v1 的 13 个 + 6 个长周期因子 ──

fn build_features_v2(navs: &[(String, f64)], idx: usize) -> Vec<f64> {
    let mut v = build_features(navs, idx);
    v.push(dist_ma(navs, idx, 120).unwrap_or(0.0)); // dist_ma120
    v.push(dist_ma(navs, idx, 250).unwrap_or(0.0)); // dist_ma250
    v.push(simple_ret(navs, idx, 126).unwrap_or(0.0)); // mom_6m
    v.push(simple_ret(navs, idx, 252).unwrap_or(0.0)); // mom_12m
    v.push(sharpe(navs, idx, 252).unwrap_or(0.0)); // sharpe_252
    v.push(price_pos(navs, idx, 252).unwrap_or(0.5)); // price_pos_252
    v
}

fn dist_ma(navs: &[(String, f64)], idx: usize, period: usize) -> Option<f64> {
    if idx + 1 < period || idx >= navs.len() {
        return None;
    }
    let start = idx + 1 - period;
    let sum: f64 = navs.iter().take(idx + 1).skip(start).map(|&(_, v)| v).sum();
    let ma = sum / period as f64;
    let now = navs[idx].1;
    if ma <= 0.0 {
        return None;
    }
    Some(now / ma - 1.0)
}

fn simple_ret(navs: &[(String, f64)], idx: usize, lookback: usize) -> Option<f64> {
    if idx < lookback || idx >= navs.len() {
        return None;
    }
    let base = navs[idx - lookback].1;
    let now = navs[idx].1;
    if base <= 0.0 {
        return None;
    }
    Some(now / base - 1.0)
}

fn sharpe(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    if idx < n || idx >= navs.len() {
        return None;
    }
    let mut rets = Vec::new();
    for i in (idx + 1 - n)..=idx {
        let prev = navs[i - 1].1;
        let now = navs[i].1;
        if prev > 0.0 && now > 0.0 {
            rets.push(now / prev - 1.0 - 0.02 / 252.0);
        }
    }
    if rets.len() < 2 {
        return None;
    }
    let mean = rets.iter().sum::<f64>() / rets.len() as f64;
    let var = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / rets.len() as f64;
    let std = var.sqrt();
    if std < 1e-12 {
        return Some(0.0);
    }
    Some(mean / std * 252.0_f64.sqrt())
}

fn price_pos(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    if idx + 1 < n || idx >= navs.len() {
        return None;
    }
    let start = idx + 1 - n;
    let mut hi = f64::MIN;
    let mut lo = f64::MAX;
    for &(_, v) in navs.iter().take(idx + 1).skip(start) {
        if v > hi {
            hi = v;
        }
        if v < lo {
            lo = v;
        }
    }
    let now = navs[idx].1;
    if hi - lo < 1e-12 {
        return Some(0.5);
    }
    Some((now - lo) / (hi - lo))
}

// ── 趋势/波动过滤 ──

/// 趋势过滤：价格 > MA250 才允许入场（上升趋势中买跌）
fn trend_filter_ok(navs: &[(String, f64)], idx: usize) -> bool {
    match dist_ma(navs, idx, 250) {
        Some(d) => d > -0.03, // 允许小幅低于均线（缓冲）
        None => match dist_ma(navs, idx, 120) {
            Some(d) => d > -0.03,
            None => true, // 数据不足时不过滤
        },
    }
}

/// 波动过滤：当前 20 日波动率 < 历史 90 分位
fn vol_filter_ok(navs: &[(String, f64)], idx: usize) -> bool {
    let cur_vol = match vol20(navs, idx) {
        Some(v) => v,
        None => return true,
    };
    // 计算过去 252 天的 vol20 序列
    let mut vols = Vec::new();
    let start = idx.saturating_sub(252);
    for i in start..idx {
        if let Some(v) = vol20(navs, i) {
            vols.push(v);
        }
    }
    if vols.len() < 50 {
        return true;
    }
    vols.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p90 = vols[vols.len() * 90 / 100];
    cur_vol < p90
}

fn vol20(navs: &[(String, f64)], idx: usize) -> Option<f64> {
    if idx < 20 || idx >= navs.len() {
        return None;
    }
    let mut rets = Vec::new();
    for i in (idx - 19)..=idx {
        let prev = navs[i - 1].1;
        let now = navs[i].1;
        if prev > 0.0 && now > 0.0 {
            rets.push(now / prev - 1.0);
        }
    }
    if rets.len() < 2 {
        return None;
    }
    let mean = rets.iter().sum::<f64>() / rets.len() as f64;
    let var = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / rets.len() as f64;
    Some(var.sqrt())
}

// ── Bagging 集成 ──

struct Ensemble {
    models: Vec<(LogRegModel, PlattCalibrator)>,
}

impl Ensemble {
    fn predict(&self, x: &[f64]) -> f64 {
        if self.models.is_empty() {
            return 0.5;
        }
        let sum: f64 = self
            .models
            .iter()
            .map(|(m, c)| {
                let p = m.predict_proba(x).unwrap_or(0.5);
                c.calibrate(p)
            })
            .sum();
        sum / self.models.len() as f64
    }
}

/// 简单 xorshift 随机数（避免引入 rand 依赖）
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn train_ensemble(
    all_x: &[Vec<f64>],
    all_y: &[f64],
    n_models: usize,
    seed: u64,
) -> Ensemble {
    let cfg = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 400,
        l2: 0.1, // v2 加大正则，对抗"高置信反向"过拟合
        pos_weight: {
            let n_pos = all_y.iter().filter(|v| **v >= 0.5).count() as f64;
            (all_y.len() as f64 - n_pos) / n_pos.max(1.0)
        },
    };
    let mut rng = Rng(seed);
    let mut models = Vec::new();
    for _ in 0..n_models {
        // 自助采样
        let n = all_x.len();
        let mut bx = Vec::with_capacity(n);
        let mut by = Vec::with_capacity(n);
        for _ in 0..n {
            let i = rng.below(n);
            bx.push(all_x[i].clone());
            by.push(all_y[i]);
        }
        if let Some(m) = train_logreg(&bx, &by, &cfg) {
            let probs: Vec<f64> = bx.iter().map(|x| m.predict_proba(x).unwrap_or(0.5)).collect();
            let cal = PlattCalibrator::fit(&probs, &by).unwrap_or_else(PlattCalibrator::identity);
            models.push((m, cal));
        }
    }
    Ensemble { models }
}

#[test]
fn model_v2_improvement() {
    // 加载数据
    let mut funds: Vec<(String, Vec<(String, f64)>, [f64; 3])> = Vec::new();
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
            let nav: f64 = item.get("nav").and_then(|n| n.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            if !date.is_empty() && nav > 0.0 {
                navs.push((date, nav));
            }
        }
        if navs.len() < 800 {
            continue;
        }
        let hpath = format!("{DATA_DIR}/holdings_{code}.json");
        let htext = std::fs::read_to_string(&hpath).unwrap_or_default();
        let hitems: Vec<serde_json::Value> = serde_json::from_str(&htext).unwrap_or_default();
        let weights: Vec<f64> = hitems.iter().filter_map(|h| h.get("weight_pct").and_then(|w| w.as_f64())).collect();
        let lt = if weights.is_empty() {
            [0.0, 0.0, 0.0]
        } else {
            let f = LookThroughFactors::from_weights(&weights);
            let v = f.as_features();
            [v[0], v[1], v[2]]
        };
        funds.push((code, navs, lt));
    }
    println!("\n加载 {} 只基金", funds.len());

    // 训练 v1（基线）：13特征+3透视，单模型
    println!("训练 v1 基线...");
    let mut v1_x = Vec::new();
    let mut v1_y = Vec::new();
    // 训练 v2：19特征+3透视
    let mut v2_x = Vec::new();
    let mut v2_y = Vec::new();
    for (_, navs, lt) in &funds {
        let n = navs.len();
        let train_end = n * 70 / 100;
        let labeled = (*navs).clone();
        for idx in 260..train_end {
            // v2 需要 250 天历史
            let mut x1 = build_features(navs, idx);
            x1.extend_from_slice(lt);
            let mut x2 = build_features_v2(navs, idx);
            x2.extend_from_slice(lt);
            let y = if trade_profitable(&labeled, idx, 20, 8.0, 4.0) { 1.0 } else { 0.0 };
            v1_x.push(x1);
            v1_y.push(y);
            v2_x.push(x2);
            v2_y.push(y);
        }
    }
    println!("v1 样本: {}, v2 样本: {}", v1_x.len(), v2_x.len());

    // v1 单模型
    let n_pos = v1_y.iter().filter(|v| **v >= 0.5).count() as f64;
    let cfg_v1 = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 400,
        l2: 0.05,
        pos_weight: (v1_y.len() as f64 - n_pos) / n_pos.max(1.0),
    };
    let m1 = train_logreg(&v1_x, &v1_y, &cfg_v1).expect("train v1");
    let p1: Vec<f64> = v1_x.iter().map(|x| m1.predict_proba(x).unwrap_or(0.5)).collect();
    let cal1 = PlattCalibrator::fit(&p1, &v1_y).unwrap_or_else(PlattCalibrator::identity);
    let sig1 = SignalGenerator {
        dip_buy_model: m1.clone(),
        magic_rebound_model: m1,
        dip_buy_calibrator: cal1.clone(),
        magic_calibrator: cal1,
        enter_threshold: 0.0,
    };

    // v2 集成
    println!("训练 v2 集成（5模型）...");
    let ensemble = train_ensemble(&v2_x, &v2_y, 5, 42);

    // 回测对比
    let mut scfg = StrategyConfig::default();
    scfg.enter_threshold = 0.55;
    let strategy = Strategy::new(scfg);

    struct Agg {
        wr: f64,
        n: usize,
        trades: usize,
        ret: f64,
    }
    let mut agg_v1 = Agg { wr: 0.0, n: 0, trades: 0, ret: 0.0 };
    let mut agg_v2 = Agg { wr: 0.0, n: 0, trades: 0, ret: 0.0 };
    let mut agg_v2_nofilter = Agg { wr: 0.0, n: 0, trades: 0, ret: 0.0 };

    for (code, navs, lt) in &funds {
        let n = navs.len();
        let test_start = n * 70 / 100;
        if n - test_start < 100 {
            continue;
        }
        let test_navs = &navs[test_start..];

        // v1 信号
        let sigs_v1: Vec<Option<TradingSignal>> = (0..test_navs.len())
            .map(|i| {
                let mut feat = build_features(test_navs, i);
                feat.extend_from_slice(lt);
                sig1.generate(&test_navs[i].0, code, &feat, feat[3]).and_then(|sig| {
                    let combined = sig.dip_buy_proba * 0.6 + sig.magic_rebound_proba * 0.4;
                    if combined >= 0.55 {
                        Some(sig)
                    } else {
                        None
                    }
                })
            })
            .collect();

        // v2 信号（带过滤）
        let sigs_v2: Vec<Option<TradingSignal>> = (0..test_navs.len())
            .map(|i| {
                let gi = test_start + i; // 全局索引（用于过滤器）
                if !trend_filter_ok(navs, gi) || !vol_filter_ok(navs, gi) {
                    return None;
                }
                let mut feat = build_features_v2(test_navs, i);
                feat.extend_from_slice(lt);
                let p = ensemble.predict(&feat);
                if p >= 0.55 {
                    // 构造一个最小信号（仅用于触发策略）
                    sig1.generate(&test_navs[i].0, code, &{
                        let mut f1 = build_features(test_navs, i);
                        f1.extend_from_slice(lt);
                        f1
                    }, feat[3]).and_then(|mut sig| {
                        sig.dip_buy_proba = p;
                        sig.magic_rebound_proba = p;
                        Some(sig)
                    })
                } else {
                    None
                }
            })
            .collect();

        // v2 信号（无过滤，仅换模型+特征）
        let sigs_v2nf: Vec<Option<TradingSignal>> = (0..test_navs.len())
            .map(|i| {
                let mut feat = build_features_v2(test_navs, i);
                feat.extend_from_slice(lt);
                let p = ensemble.predict(&feat);
                if p >= 0.55 {
                    sig1.generate(&test_navs[i].0, code, &{
                        let mut f1 = build_features(test_navs, i);
                        f1.extend_from_slice(lt);
                        f1
                    }, feat[3]).and_then(|mut sig| {
                        sig.dip_buy_proba = p;
                        sig.magic_rebound_proba = p;
                        Some(sig)
                    })
                } else {
                    None
                }
            })
            .collect();

        for (sigs, agg) in [
            (&sigs_v1, &mut agg_v1),
            (&sigs_v2, &mut agg_v2),
            (&sigs_v2nf, &mut agg_v2_nofilter),
        ] {
            let result =
                backtest_single_fund(code, test_navs, &|i| sigs[i].clone(), &strategy, 100_000.0);
            if result.metrics.total_trades >= 3 {
                agg.n += 1;
                agg.wr += result.metrics.win_rate;
                agg.trades += result.metrics.total_trades;
                agg.ret += result.metrics.total_return_pct;
            }
        }
    }

    println!("\n=== v1 vs v2 对比（阈值0.55，策略8%止盈/4%止损/20天） ===");
    for (name, a) in [
        ("v1基线(13+3特征,单模型)", &agg_v1),
        ("v2(19+3特征,集成,无过滤)", &agg_v2_nofilter),
        ("v2(19+3特征,集成,趋势+波动过滤)", &agg_v2),
    ] {
        println!(
            "{:<32} 可评估{:>3} 平均胜率{:>5.1}% 平均交易{:>5.1} 平均收益{:>7.1}%",
            name,
            a.n,
            if a.n > 0 { a.wr / a.n as f64 * 100.0 } else { 0.0 },
            if a.n > 0 { a.trades as f64 / a.n as f64 } else { 0.0 },
            if a.n > 0 { a.ret / a.n as f64 } else { 0.0 },
        );
    }
}
