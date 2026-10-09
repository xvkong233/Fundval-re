//! 离线预测准确率评测（合成数据）
//!
//! 用带种子 RNG 生成多只"基金"的净值序列（含趋势/震荡/急跌反弹等机制），
//! 走真实管线 `build_trigger_samples_for_peer` 构造样本，按时间 70/30 切分，
//! 评估 LogReg（dip_buy / magic_rebound）的准确率/精确率/召回率/F1/AUC，
//! 以及 OLS 闭式岭回归净值预测的 RMSE 与方向准确率。
//!
//! 运行：`cargo test -p api --test ml_accuracy_eval_test -- --nocapture`

use api::forecast::ols_sgd::train_ols_closed_form;
use api::ml::dataset::{DatasetConfig, TriggerSample, build_trigger_samples_for_peer};
use api::ml::logreg::{LogRegModel, LogRegTrainConfig, train_logreg};
use chrono::Datelike;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

/// 简单高斯采样（rand 0.8 无 rand_distr 时用 Box-Muller）
fn gauss(rng: &mut StdRng) -> f64 {
    use rand::Rng;
    let u1: f64 = rng.gen_range(1e-9..1.0);
    let u2: f64 = rng.gen_range(0.0..1.0);
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// 生成一只基金的净值序列。
///
/// 机制：几何随机游走 + 机制切换（牛/熊/震荡）+ 均值回归（急跌后反弹倾向）。
/// 这样 dip_buy（跌后 h 天是否上涨）与 magic_rebound（是否强反弹）标签
/// 生成一只基金的净值序列（事件驱动）。
///
/// 90% 时间：带温和漂移的随机游走；
/// 10% 触发"事件"：
///   - 70% 为"恐慌抛售"：3 天内跌 ~6%，随后 5 天内大概率 V 型反弹 +5.5%
///     （制造"急跌后反弹"的强可学习信号，对应 magic_rebound / dip_buy）；
///   - 30% 为"突破"：3 天内涨 ~5%，随后动量延续。
fn gen_nav(seed: u64, days: usize, drift_bias: f64) -> Vec<f64> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut nav = vec![1.0_f64; days];
    let mut i = 1_usize;
    let mut event: Option<(usize, f64)> = None;
    let mut rebound_pending: Option<(usize, f64)> = None;

    while i < days {
        let ret = if let Some((left, daily)) = event {
            event = if left <= 1 { None } else { Some((left - 1, daily)) };
            daily + gauss(&mut rng) * 0.004
        } else if let Some((left, daily)) = rebound_pending {
            rebound_pending = if left <= 1 { None } else { Some((left - 1, daily)) };
            daily + gauss(&mut rng) * 0.004
        } else {
            let vol = 0.008 + rng.gen_range(0.0..1.0) * 0.006;
            let r = drift_bias + gauss(&mut rng) * vol;
            if rng.gen_range(0.0..1.0) < 0.10 && i + 12 < days {
                if rng.gen_range(0.0..1.0) < 0.70 {
                    // 恐慌抛售：3 天 -6%，随后 8 天 +10% 强反弹（覆盖 8% 阈值）
                    event = Some((3, -0.02));
                    rebound_pending = Some((8, 0.012));
                } else {
                    event = Some((3, 0.016));
                    rebound_pending = Some((5, 0.006));
                }
            }
            r
        };
        let prev = nav[i - 1];
        nav[i] = (prev * (1.0 + ret)).max(0.05);
        i += 1;
    }
    nav
}

fn auc_score(y_true: &[f64], y_score: &[f64]) -> f64 {
    let n = y_true.len();
    assert_eq!(n, y_score.len());
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        y_score[b]
            .partial_cmp(&y_score[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let n_pos = y_true.iter().filter(|v| **v >= 0.5).count() as f64;
    let n_neg = n as f64 - n_pos;
    if n_pos == 0.0 || n_neg == 0.0 {
        return 0.5;
    }
    // Mann-Whitney U 统计量
    let mut tp = 0.0_f64;
    let mut fp = 0.0_f64;
    let mut auc = 0.0_f64;
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && (y_score[order[j]] - y_score[order[i]]).abs() < 1e-12 {
            j += 1;
        }
        let mut tp_add = 0.0_f64;
        let mut fp_add = 0.0_f64;
        for &idx in &order[i..j] {
            if y_true[idx] >= 0.5 {
                tp_add += 1.0;
            } else {
                fp_add += 1.0;
            }
        }
        // 并列组内：矩形 + 三角形
        auc += fp_add * tp + fp_add * tp_add / 2.0;
        tp += tp_add;
        fp += fp_add;
        i = j;
    }
    let _ = fp;
    auc / (n_pos * n_neg)
}

struct ClfMetrics {
    acc: f64,
    precision: f64,
    recall: f64,
    f1: f64,
    auc: f64,
    n: usize,
    pos_rate: f64,
}

fn eval_classifier(model: &LogRegModel, xs: &[Vec<f64>], ys: &[f64]) -> ClfMetrics {
    let n = xs.len();
    let mut tp = 0;
    let mut tn = 0;
    let mut fp = 0;
    let mut fn_ = 0;
    let mut scores = Vec::with_capacity(n);
    for (x, &y) in xs.iter().zip(ys.iter()) {
        let p = model.predict_proba(x).unwrap_or(0.5);
        scores.push(p);
        let pred = p >= 0.5;
        let actual = y >= 0.5;
        match (pred, actual) {
            (true, true) => tp += 1,
            (false, false) => tn += 1,
            (true, false) => fp += 1,
            (false, true) => fn_ += 1,
        }
    }
    let acc = (tp + tn) as f64 / n.max(1) as f64;
    let precision = tp as f64 / (tp + fp).max(1) as f64;
    let recall = tp as f64 / (tp + fn_).max(1) as f64;
    let f1 = if precision + recall > 1e-12 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };
    let auc = auc_score(ys, &scores);
    let pos_rate = ys.iter().filter(|v| **v >= 0.5).count() as f64 / n.max(1) as f64;
    ClfMetrics {
        acc,
        precision,
        recall,
        f1,
        auc,
        n,
        pos_rate,
    }
}

fn print_clf(name: &str, m: &ClfMetrics) {
    println!(
        "  [{name}] n={} 正样本率={:.1}% 准确率={:.1}% 精确率={:.1}% 召回率={:.1}% F1={:.3} AUC={:.3}",
        m.n,
        m.pos_rate * 100.0,
        m.acc * 100.0,
        m.precision * 100.0,
        m.recall * 100.0,
        m.f1,
        m.auc,
    );
}

/// 单特征 AUC 诊断：看每个特征各自的区分度
fn univariate_auc(samples: &[TriggerSample], idx: &[usize], task: &str) {
    let (x, y) = to_xy(samples, idx, task);
    let names = api::ml::features::FEATURE_NAMES;
    print!("  单特征AUC |");
    for j in 0..x[0].len() {
        let scores: Vec<f64> = x.iter().map(|row| row[j]).collect();
        let a = auc_score(&y, &scores);
        // 也试反向
        let a = a.max(1.0 - a);
        print!(" {}={:.3} |", names[j], a);
    }
    println!();
}

async fn seed_db(pool: &sqlx::AnyPool, n_funds: usize, days: usize) {
    for fi in 0..n_funds {
        let fund_id = format!("fund-{fi}");
        let code = format!("9{:05}", fi);
        sqlx::query(
            r#"INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
               VALUES ($1,$2,$3,$4,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)"#,
        )
        .bind(&fund_id)
        .bind(&code)
        .bind(format!("合成基金{code}"))
        .bind("混合型")
        .execute(pool)
        .await
        .expect("seed fund");
        sqlx::query(
            r#"INSERT INTO fund_relate_theme (fund_code, sec_code, sec_name, corr_1y, ol2top, source, fetched_at, created_at, updated_at)
               VALUES ($1,'SYN','合成板块',90.0,90.0,'syn',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)"#,
        )
        .bind(&code)
        .execute(pool)
        .await
        .expect("seed theme");

        let navs = gen_nav(1000 + fi as u64, days, 0.0001 * (fi as f64 - n_funds as f64 / 2.0));
        let mut date = chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        for (i, v) in navs.iter().enumerate() {
            // 跳过周末，模拟交易日
            while date.weekday().number_from_monday() > 5 {
                date = date.succ_opt().unwrap();
            }
            sqlx::query(
                r#"INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
                   VALUES ($1,'syn',$2,$3,$4,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)"#,
            )
            .bind(format!("nav-{fund_id}-{i}"))
            .bind(&fund_id)
            .bind(date.format("%Y-%m-%d").to_string())
            .bind(format!("{v:.4}"))
            .execute(pool)
            .await
            .expect("seed nav");
            date = date.succ_opt().unwrap();
        }
    }
}

fn split_by_time(samples: &[TriggerSample], train_ratio: f64) -> (Vec<usize>, Vec<usize>) {
    let mut idx: Vec<usize> = (0..samples.len()).collect();
    idx.sort_by(|&a, &b| samples[a].as_of_date.cmp(&samples[b].as_of_date));
    let cut = ((idx.len() as f64) * train_ratio) as usize;
    (idx[..cut].to_vec(), idx[cut..].to_vec())
}

fn to_xy(samples: &[TriggerSample], idx: &[usize], task: &str) -> (Vec<Vec<f64>>, Vec<f64>) {
    let mut x = Vec::with_capacity(idx.len());
    let mut y = Vec::with_capacity(idx.len());
    for &i in idx {
        x.push(samples[i].features.clone());
        let label = if task == "dip_buy" {
            samples[i].dip_buy_success
        } else {
            samples[i].magic_rebound
        };
        y.push(if label { 1.0 } else { 0.0 });
    }
    (x, y)
}

fn train_cfg_balanced(y: &[f64]) -> LogRegTrainConfig {
    let n_pos = y.iter().filter(|v| **v >= 0.5).count() as f64;
    let n_neg = y.len() as f64 - n_pos;
    LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 600,
        l2: 0.1,
        pos_weight: if n_pos > 0.0 { n_neg / n_pos } else { 1.0 },
    }
}

#[tokio::test]
async fn eval_prediction_accuracy() {
    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect");
    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    let n_funds = 30;
    let days = 600;
    seed_db(&pool, n_funds, days).await;

    for horizon in [5_usize, 20_usize] {
        let cfg = DatasetConfig {
            lookback_days: 252,
            horizon_days: horizon,
            stride_days: 5,
        };
        let samples = build_trigger_samples_for_peer(&pool, "SYN", "syn", &cfg)
            .await
            .expect("build samples");
        println!(
            "\nhorizon={horizon}: 样本数={} (基金数={n_funds}, 天数={days})",
            samples.len()
        );
        assert!(!samples.is_empty(), "样本不应为空");

        let (tr_idx, te_idx) = split_by_time(&samples, 0.7);
        println!("  训练={} 测试={}", tr_idx.len(), te_idx.len());

        for task in ["dip_buy", "magic_rebound"] {
            let (x_tr, y_tr) = to_xy(&samples, &tr_idx, task);
            let (x_te, y_te) = to_xy(&samples, &te_idx, task);
            univariate_auc(&samples, &tr_idx, task);
            let cfg = train_cfg_balanced(&y_tr);
            let model = train_logreg(&x_tr, &y_tr, &cfg).expect("train");
            let m_tr = eval_classifier(&model, &x_tr, &y_tr);
            let m_te = eval_classifier(&model, &x_te, &y_te);
            print!("训练集");
            print_clf(task, &m_tr);
            print!("测试集");
            print_clf(task, &m_te);
        }
    }
}

/// 对照实验：用"明显模式"数据验证优化器本身能学到强信号。
/// 若此处 AUC 接近 1，则说明优化器正常，瓶颈在真实信号强度。
#[test]
fn sanity_logreg_learns_clear_pattern() {
    let mut rng = StdRng::seed_from_u64(42);
    // y = 1 当且仅当 x0 + x1 > 0（线性可分）
    let n = 500;
    let mut x = Vec::with_capacity(n);
    let mut y = Vec::with_capacity(n);
    for _ in 0..n {
        let a = gauss(&mut rng);
        let b = gauss(&mut rng);
        x.push(vec![a, b]);
        y.push(if a + b > 0.0 { 1.0 } else { 0.0 });
    }
    let cfg = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 600,
        l2: 0.1,
        pos_weight: 1.0,
    };
    let model = train_logreg(&x, &y, &cfg).expect("train");
    let m = eval_classifier(&model, &x, &y);
    println!("\n对照实验（线性可分）: 准确率={:.1}% AUC={:.3}", m.acc * 100.0, m.auc);
    assert!(m.auc > 0.99, "优化器应能学到线性可分模式");
}

/// 对照实验2：非线性模式 y = (x0^2 + x1^2 > 1)，纯线性模型学不会，
/// 但加入平方特征后应能学会。
#[test]
fn sanity_logreg_needs_nonlinear_features() {
    let mut rng = StdRng::seed_from_u64(7);
    let n = 600;
    let mut x_lin = Vec::with_capacity(n);
    let mut x_quad = Vec::with_capacity(n);
    let mut y = Vec::with_capacity(n);
    for _ in 0..n {
        let a = gauss(&mut rng);
        let b = gauss(&mut rng);
        x_lin.push(vec![a, b]);
        x_quad.push(vec![a, b, a * a, b * b]);
        y.push(if a * a + b * b > 1.0 { 1.0 } else { 0.0 });
    }
    let cfg = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 600,
        l2: 0.1,
        pos_weight: 1.0,
    };
    let m_lin = eval_classifier(&train_logreg(&x_lin, &y, &cfg).unwrap(), &x_lin, &y);
    let m_quad = eval_classifier(&train_logreg(&x_quad, &y, &cfg).unwrap(), &x_quad, &y);
    println!(
        "\n对照实验（圆形边界）: 线性特征 AUC={:.3} / 加入平方特征 AUC={:.3}",
        m_lin.auc, m_quad.auc
    );
    assert!(m_quad.auc > 0.95, "加入平方特征后应能学会圆形边界");
}

#[tokio::test]
async fn eval_ols_forecast_accuracy() {
    // OLS 闭式岭回归：对比"纯 lag 收益" vs "技术特征"的下一日收益预测
    let navs = gen_nav(777, 800, 0.0003);

    // 方案A：纯 lag20 对数收益（当前生产方案）
    let lag_k = 20;
    let mut xa: Vec<Vec<f64>> = Vec::new();
    let mut ya: Vec<f64> = Vec::new();
    for i in lag_k..navs.len() - 1 {
        let mut row = Vec::with_capacity(lag_k);
        for k in 1..=lag_k {
            row.push((navs[i - k + 1] / navs[i - k]).ln());
        }
        xa.push(row);
        ya.push((navs[i + 1] / navs[i]).ln());
    }

    // 方案B：8 维技术特征（新）
    let mut xb: Vec<Vec<f64>> = Vec::new();
    let mut yb: Vec<f64> = Vec::new();
    for i in 24..navs.len() - 1 {
        if let Some(feat) = api::ml::features::build_forecast_features(&navs, i) {
            xb.push(feat);
            yb.push((navs[i + 1] / navs[i]).ln());
        }
    }

    for (name, x, y) in [("lag20", &xa, &ya), ("tech8", &xb, &yb)] {
        let cut = (x.len() as f64 * 0.7) as usize;
        let model = train_ols_closed_form(&x[..cut], &y[..cut], 1.0).expect("train ols");
        let mut se = 0.0_f64;
        let mut se_base = 0.0_f64;
        let mut dir_ok = 0_usize;
        let mean_tr: f64 = y[..cut].iter().sum::<f64>() / cut as f64;
        let n_te = x.len() - cut;
        for i in cut..x.len() {
            let p = model.predict(&x[i]).unwrap_or(0.0);
            let e = p - y[i];
            se += e * e;
            let eb = mean_tr - y[i];
            se_base += eb * eb;
            if (p >= 0.0) == (y[i] >= 0.0) {
                dir_ok += 1;
            }
        }
        let rmse = (se / n_te as f64).sqrt();
        let rmse_base = (se_base / n_te as f64).sqrt();
        println!(
            "OLS [{name}]: RMSE={rmse:.5} (基线={rmse_base:.5}) 方向准确率={:.1}%",
            dir_ok as f64 / n_te as f64 * 100.0,
        );
    }
}
