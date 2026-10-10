//! v3 两阶段模型验证（学习 Abu 的 UMP 裁判系统）。
//!
//! Stage 1：v3特征（20维）逻辑回归 → P(盈利)
//! Stage 2（UMP否决）：元分类器，输入 (stage1概率, ADX, 趋势状态, 波动状态, 唐奇安位置)
//!         → P(交易真实盈利)，低于阈值则否决
//!
//! 关键：Stage 2 训练用 Stage 1 的样本外预测，避免数据泄漏（Stacking）。

use api::ml::dataset::trade_profitable;
use api::ml::features_v3::build_features_v3;
use api::ml::logreg::{train_logreg, LogRegModel, LogRegTrainConfig};
use api::quant::backtest::backtest_single_fund;
use api::quant::calibration::PlattCalibrator;
use api::quant::regime::{entry_allowed, TrendState, VolState, trend_state, vol_state};
use api::quant::signal::TradingSignal;
use api::quant::strategy::{Strategy, StrategyConfig};

mod common;

/// UMP元特征：[stage1_proba, adx_norm, trend_up, trend_down, vol_extreme, donchian_pos]
fn build_ump_features(
    stage1_proba: f64,
    navs: &[(String, f64)],
    idx: usize,
    v3_feat: &[f64],
) -> Vec<f64> {
    // v3特征索引：13=adx14, 14=donchian_pos20
    let adx = v3_feat.get(13).copied().unwrap_or(0.0) / 100.0;
    let donch_pos = v3_feat.get(14).copied().unwrap_or(0.5);
    let trend_up = if trend_state(navs, idx) == TrendState::Uptrend { 1.0 } else { 0.0 };
    let trend_down = if trend_state(navs, idx) == TrendState::Downtrend { 1.0 } else { 0.0 };
    let vol_extreme = if vol_state(navs, idx) == VolState::Extreme { 1.0 } else { 0.0 };
    vec![stage1_proba, adx, trend_up, trend_down, vol_extreme, donch_pos]
}

struct TwoStageModel {
    stage1: LogRegModel,
    stage1_cal: PlattCalibrator,
    stage2: LogRegModel,
    stage2_cal: PlattCalibrator,
}

impl TwoStageModel {
    fn predict(&self, v3_feat: &[f64], navs: &[(String, f64)], idx: usize) -> (f64, f64) {
        let p1_raw = self.stage1.predict_proba(v3_feat).unwrap_or(0.5);
        let p1 = self.stage1_cal.calibrate(p1_raw);
        let ump_feat = build_ump_features(p1, navs, idx, v3_feat);
        let p2_raw = self.stage2.predict_proba(&ump_feat).unwrap_or(0.5);
        let p2 = self.stage2_cal.calibrate(p2_raw);
        (p1, p2)
    }
}

#[test]
fn model_v3_two_stage() {
    let funds = common::load_random_funds(150, 123, 800);
    println!("\n随机抽样 {} 只基金（seed=123）", funds.len());

    // 准备 v3 训练数据
    let mut all_x = Vec::new();
    let mut all_y = Vec::new();
    // 保存 (fund_idx, sample_idx, global_nav_idx) 用于两阶段切分
    let mut meta: Vec<(usize, usize)> = Vec::new(); // (fund_pos, global_idx)
    for (fi, (_, navs, lt)) in funds.iter().enumerate() {
        let n = navs.len();
        let train_end = n * 70 / 100;
        let labeled = (*navs).clone();
        for idx in 60..train_end {
            let mut x = build_features_v3(navs, idx);
            x.extend_from_slice(lt);
            let y = if trade_profitable(&labeled, idx, 20, 8.0, 4.0) { 1.0 } else { 0.0 };
            all_x.push(x);
            all_y.push(y);
            meta.push((fi, idx));
        }
    }
    println!("v3 样本: {}", all_x.len());

    // 切分：前半训练stage1，后半训练stage2（用stage1的样本外预测）
    let split = all_x.len() / 2;
    let (x_a, x_b) = all_x.split_at(split);
    let (y_a, y_b) = all_y.split_at(split);
    let (meta_a, meta_b) = meta.split_at(split);

    let cfg1 = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 400,
        l2: 0.1,
        pos_weight: {
            let n_pos = y_a.iter().filter(|v| **v >= 0.5).count() as f64;
            (y_a.len() as f64 - n_pos) / n_pos.max(1.0)
        },
    };
    let stage1 = train_logreg(x_a, y_a, &cfg1).expect("stage1");
    let p1a: Vec<f64> = x_a.iter().map(|x| stage1.predict_proba(x).unwrap_or(0.5)).collect();
    let cal1 = PlattCalibrator::fit(&p1a, y_a).unwrap_or_else(PlattCalibrator::identity);

    // Stage1 在 B 上的样本外预测
    let mut ump_x = Vec::new();
    for (i, x) in x_b.iter().enumerate() {
        let p_raw = stage1.predict_proba(x).unwrap_or(0.5);
        let p1 = cal1.calibrate(p_raw);
        let (fi, gidx) = meta_b[i];
        let (_, navs, _) = &funds[fi];
        let ump_feat = build_ump_features(p1, navs, gidx, x);
        ump_x.push(ump_feat);
    }

    // 训练 stage2（UMP）
    let cfg2 = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 300,
        l2: 0.1,
        pos_weight: {
            let n_pos = y_b.iter().filter(|v| **v >= 0.5).count() as f64;
            (y_b.len() as f64 - n_pos) / n_pos.max(1.0)
        },
    };
    let stage2 = train_logreg(&ump_x, y_b, &cfg2).expect("stage2");
    let p2: Vec<f64> = ump_x.iter().map(|x| stage2.predict_proba(x).unwrap_or(0.5)).collect();
    let cal2 = PlattCalibrator::fit(&p2, y_b).unwrap_or_else(PlattCalibrator::identity);

    let ts_model = TwoStageModel {
        stage1,
        stage1_cal: cal1,
        stage2,
        stage2_cal: cal2,
    };

    // 对比：单阶段 v3 vs 两阶段
    // 单阶段模型（全量训练，用于对比）
    let cfg_full = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 400,
        l2: 0.1,
        pos_weight: {
            let n_pos = all_y.iter().filter(|v| **v >= 0.5).count() as f64;
            (all_y.len() as f64 - n_pos) / n_pos.max(1.0)
        },
    };
    let m_single = train_logreg(&all_x, &all_y, &cfg_full).expect("single");
    let ps: Vec<f64> = all_x.iter().map(|x| m_single.predict_proba(x).unwrap_or(0.5)).collect();
    let cal_s = PlattCalibrator::fit(&ps, &all_y).unwrap_or_else(PlattCalibrator::identity);

    fn make_signal(date: &str, code: &str, proba: f64) -> TradingSignal {
        use api::quant::signal::SignalStrength;
        TradingSignal {
            date: date.to_string(),
            fund_code: code.to_string(),
            dip_buy_proba: proba,
            magic_rebound_proba: proba,
            strength: SignalStrength::Strong,
            enter: true,
            take_profit_pct: 8.0,
            stop_loss_pct: 4.0,
        }
    }

    let mut scfg = StrategyConfig::default();
    scfg.enter_threshold = 0.55;
    scfg.use_atr_stop = true;
    scfg.atr_stop_mult = 4.0;
    scfg.atr_profit_mult = 8.0;
    let strategy = Strategy::new(scfg);

    struct Agg {
        wr: f64,
        n: usize,
        trades: usize,
        ret: f64,
    }
    let mut agg_single = Agg { wr: 0.0, n: 0, trades: 0, ret: 0.0 };
    let mut agg_two = Agg { wr: 0.0, n: 0, trades: 0, ret: 0.0 };

    for (code, navs, lt) in &funds {
        let n = navs.len();
        let test_start = n * 70 / 100;
        if n - test_start < 100 {
            continue;
        }
        let test_navs = &navs[test_start..];

        // 单阶段信号
        let sigs_single: Vec<Option<TradingSignal>> = (0..test_navs.len())
            .map(|i| {
                let gi = test_start + i;
                if !entry_allowed(navs, gi).0 {
                    return None;
                }
                let mut feat = build_features_v3(test_navs, i);
                feat.extend_from_slice(lt);
                let p_raw = m_single.predict_proba(&feat).unwrap_or(0.5);
                let p = cal_s.calibrate(p_raw);
                if p >= 0.55 {
                    Some(make_signal(&test_navs[i].0, code, p))
                } else {
                    None
                }
            })
            .collect();

        // 两阶段信号：stage1 >= 0.55 且 stage2(UMP) >= 0.5
        let sigs_two: Vec<Option<TradingSignal>> = (0..test_navs.len())
            .map(|i| {
                let gi = test_start + i;
                if !entry_allowed(navs, gi).0 {
                    return None;
                }
                let mut feat = build_features_v3(test_navs, i);
                feat.extend_from_slice(lt);
                let (p1, p2) = ts_model.predict(&feat, navs, gi);
                if p1 >= 0.55 && p2 >= 0.5 {
                    Some(make_signal(&test_navs[i].0, code, p1))
                } else {
                    None
                }
            })
            .collect();

        for (sigs, agg) in [(&sigs_single, &mut agg_single), (&sigs_two, &mut agg_two)] {
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

    println!("\n=== v3单阶段 vs 两阶段UMP（含regime过滤+ATR，阈值0.55） ===");
    for (name, a) in [("v3单阶段", &agg_single), ("v3两阶段UMP", &agg_two)] {
        println!(
            "{:<12} 可评估{:>3} 平均胜率{:>5.1}% 平均交易{:>5.1} 平均收益{:>7.1}%",
            name,
            a.n,
            if a.n > 0 { a.wr / a.n as f64 * 100.0 } else { 0.0 },
            if a.n > 0 { a.trades as f64 / a.n as f64 } else { 0.0 },
            if a.n > 0 { a.ret / a.n as f64 } else { 0.0 },
        );
    }
}
