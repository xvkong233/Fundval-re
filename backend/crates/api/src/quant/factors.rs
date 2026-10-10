//! 因子库（参考 QUANTAXIS QAFactor / QAIndicator 设计思想）。
//!
//! 为开放式基金净值序列提供标准化因子计算：
//! - 动量因子：多周期收益、风险调整动量
//! - 波动因子：历史波动率、最大回撤、回撤持续
//! - 趋势因子：均线偏离、多头排列、价格位置
//! - 反转因子：RSI、布林带 %b、高低点位置
//! - 质量因子：夏普率、卡玛比率、胜率
//!
//! 并提供单因子 IC（信息系数）测试，用于因子有效性研究。

use std::collections::HashMap;

/// 全部因子名（顺序与 [`compute_all`] 返回一致）
pub const FACTOR_NAMES: [&str; 24] = [
    // 动量 (6)
    "mom_1m", "mom_3m", "mom_6m", "mom_12m", "mom_adj", "trend_60",
    // 波动 (4)
    "vol_20", "vol_60", "max_dd_252", "dd_days",
    // 趋势 (6)
    "dist_ma20", "dist_ma60", "dist_ma120", "dist_ma250",
    "ma_align", "price_pos_252",
    // 反转 (4)
    "rsi_14", "boll_pctb", "dist_high_252", "dist_low_252",
    // 质量 (4)
    "sharpe_252", "calmar_252", "win_rate_60", "profit_factor_60",
];

/// 对 navs[idx] 计算全部 24 个因子。
///
/// navs 按日期升序；idx 为当前下标。历史不足时用 0.0 填充（中性值）。
pub fn compute_all(navs: &[(String, f64)], idx: usize) -> Vec<f64> {
    vec![
        // 动量
        simple_return(navs, idx, 21).unwrap_or(0.0),
        simple_return(navs, idx, 63).unwrap_or(0.0),
        simple_return(navs, idx, 126).unwrap_or(0.0),
        simple_return(navs, idx, 252).unwrap_or(0.0),
        risk_adjusted_momentum(navs, idx),
        lin_trend(navs, idx, 60).unwrap_or(0.0),
        // 波动
        volatility(navs, idx, 20).unwrap_or(0.0),
        volatility(navs, idx, 60).unwrap_or(0.0),
        max_drawdown(navs, idx, 252),
        (drawdown_days(navs, idx, 252) as f64 / 60.0).min(1.0),
        // 趋势
        dist_ma(navs, idx, 20).unwrap_or(0.0),
        dist_ma(navs, idx, 60).unwrap_or(0.0),
        dist_ma(navs, idx, 120).unwrap_or(0.0),
        dist_ma(navs, idx, 250).unwrap_or(0.0),
        ma_alignment(navs, idx),
        price_position(navs, idx, 252).unwrap_or(0.5),
        // 反转
        rsi(navs, idx, 14).unwrap_or(0.5),
        bollinger_pctb(navs, idx, 20).unwrap_or(0.5),
        dist_high(navs, idx, 252).unwrap_or(0.0),
        dist_low(navs, idx, 252).unwrap_or(0.0),
        // 质量
        sharpe(navs, idx, 252).unwrap_or(0.0),
        calmar(navs, idx, 252).unwrap_or(0.0),
        win_rate(navs, idx, 60).unwrap_or(0.5),
        profit_factor(navs, idx, 60).unwrap_or(1.0),
    ]
}

// ── 基础计算 ──────────────────────────────────────────────

fn closes(navs: &[(String, f64)], idx: usize, n: usize) -> Option<Vec<f64>> {
    if idx >= navs.len() || n == 0 {
        return None;
    }
    let start = idx.saturating_sub(n - 1);
    let v: Vec<f64> = navs[start..=idx].iter().map(|(_, c)| *c).collect();
    if v.len() < 2 || v.iter().any(|&x| x <= 0.0) {
        return None;
    }
    Some(v)
}

fn simple_return(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let v = closes(navs, idx, n + 1)?;
    let base = v.first()?;
    let last = v.last()?;
    Some(last / base - 1.0)
}

fn daily_returns(v: &[f64]) -> Vec<f64> {
    v.windows(2).map(|w| w[1] / w[0] - 1.0).collect()
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn std(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let m = mean(xs);
    (xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() - 1) as f64).sqrt()
}

fn volatility(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let v = closes(navs, idx, n + 1)?;
    let rets = daily_returns(&v);
    Some(std(&rets) * (252.0_f64).sqrt()) // 年化
}

fn sma(v: &[f64], n: usize) -> Option<f64> {
    if v.len() < n {
        return None;
    }
    Some(mean(&v[v.len() - n..]))
}

fn dist_ma(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let v = closes(navs, idx, n)?;
    let ma = sma(&v, n)?;
    let last = v.last()?;
    Some(last / ma - 1.0)
}

fn rsi(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let v = closes(navs, idx, n + 1)?;
    let rets = daily_returns(&v);
    let gains: f64 = rets.iter().filter(|&&r| r > 0.0).sum();
    let losses: f64 = rets.iter().filter(|&&r| r < 0.0).map(|r| -r).sum();
    if gains + losses < 1e-12 {
        return Some(0.5);
    }
    Some(gains / (gains + losses))
}

fn lin_trend(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let v = closes(navs, idx, n)?;
    let log_v: Vec<f64> = v.iter().map(|x| x.ln()).collect();
    let m = log_v.len() as f64;
    let mx = (m - 1.0) / 2.0;
    let my = mean(&log_v);
    let mut num = 0.0;
    let mut den = 0.0;
    for (i, y) in log_v.iter().enumerate() {
        num += (i as f64 - mx) * (y - my);
        den += (i as f64 - mx).powi(2);
    }
    if den < 1e-12 {
        return Some(0.0);
    }
    Some(num / den * 252.0) // 年化斜率
}

/// 风险调整动量：60日收益 / (1 + 60日波动率)
fn risk_adjusted_momentum(navs: &[(String, f64)], idx: usize) -> f64 {
    let r = simple_return(navs, idx, 60).unwrap_or(0.0);
    let v = volatility(navs, idx, 60).unwrap_or(0.0);
    r / (1.0 + v)
}

/// 均线多头排列度：[-1, 1]，1 = 完全多头排列
fn ma_alignment(navs: &[(String, f64)], idx: usize) -> f64 {
    let v = match closes(navs, idx, 250) {
        Some(v) => v,
        None => return 0.0,
    };
    let ma5 = sma(&v, 5);
    let ma20 = sma(&v, 20);
    let ma60 = sma(&v, 60);
    let ma120 = sma(&v, 120);
    let ma250 = sma(&v, 250);
    let mas: Vec<f64> = [ma5, ma20, ma60, ma120, ma250].into_iter().flatten().collect();
    if mas.len() < 2 {
        return 0.0;
    }
    // 计算有序对中升序的比例
    let mut ordered = 0;
    let mut total = 0;
    for i in 0..mas.len() {
        for j in (i + 1)..mas.len() {
            total += 1;
            if mas[i] >= mas[j] {
                ordered += 1;
            }
        }
    }
    (ordered as f64 / total as f64) * 2.0 - 1.0
}

/// 价格在过去 n 日高低点间的位置 [0,1]
fn price_position(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let v = closes(navs, idx, n)?;
    let hi = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
    let last = v.last()?;
    if hi - lo < 1e-12 {
        return Some(0.5);
    }
    Some((last - lo) / (hi - lo))
}

/// 布林带 %b [0,1]，>1 突破上轨，<0 跌破下轨
fn bollinger_pctb(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let v = closes(navs, idx, n)?;
    let ma = sma(&v, n)?;
    let sd = std(&v[v.len() - n..]);
    let last = v.last()?;
    let upper = ma + 2.0 * sd;
    let lower = ma - 2.0 * sd;
    if upper - lower < 1e-12 {
        return Some(0.5);
    }
    Some((last - lower) / (upper - lower))
}

fn dist_high(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let v = closes(navs, idx, n)?;
    let hi = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let last = v.last()?;
    Some(last / hi - 1.0) // 负值，越接近0越强
}

fn dist_low(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let v = closes(navs, idx, n)?;
    let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
    let last = v.last()?;
    Some(last / lo - 1.0) // 正值，越大离底部越远
}

fn max_drawdown(navs: &[(String, f64)], idx: usize, n: usize) -> f64 {
    let v = match closes(navs, idx, n) {
        Some(v) => v,
        None => return 0.0,
    };
    let mut peak = v[0];
    let mut mdd = 0.0;
    for &x in &v {
        if x > peak {
            peak = x;
        }
        let dd = (peak - x) / peak;
        if dd > mdd {
            mdd = dd;
        }
    }
    mdd
}

fn drawdown_days(navs: &[(String, f64)], idx: usize, n: usize) -> usize {
    let v = match closes(navs, idx, n) {
        Some(v) => v,
        None => return 0,
    };
    let mut peak = v[0];
    let mut peak_idx = 0;
    for (i, &x) in v.iter().enumerate() {
        if x >= peak {
            peak = x;
            peak_idx = i;
        }
    }
    v.len() - 1 - peak_idx
}

/// 年化夏普率（无风险利率按 2%）
fn sharpe(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let v = closes(navs, idx, n + 1)?;
    let rets = daily_returns(&v);
    let excess: Vec<f64> = rets.iter().map(|r| r - 0.02 / 252.0).collect();
    let s = std(&excess);
    if s < 1e-12 {
        return Some(0.0);
    }
    Some(mean(&excess) / s * (252.0_f64).sqrt())
}

/// 卡玛比率：年化收益 / 最大回撤
fn calmar(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let r = simple_return(navs, idx, n)?;
    let ann_ret = (1.0 + r).powf(252.0 / n as f64) - 1.0;
    let mdd = max_drawdown(navs, idx, n);
    if mdd < 1e-6 {
        return Some(0.0);
    }
    Some(ann_ret / mdd)
}

fn win_rate(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let v = closes(navs, idx, n + 1)?;
    let rets = daily_returns(&v);
    if rets.is_empty() {
        return None;
    }
    Some(rets.iter().filter(|&&r| r > 0.0).count() as f64 / rets.len() as f64)
}

fn profit_factor(navs: &[(String, f64)], idx: usize, n: usize) -> Option<f64> {
    let v = closes(navs, idx, n + 1)?;
    let rets = daily_returns(&v);
    let gains: f64 = rets.iter().filter(|&&r| r > 0.0).sum();
    let losses: f64 = rets.iter().filter(|&&r| r < 0.0).map(|r| -r).sum();
    if losses < 1e-12 {
        return Some(if gains > 0.0 { 3.0 } else { 1.0 });
    }
    Some((gains / losses).min(5.0))
}

// ── 单因子 IC 测试 ──────────────────────────────────────────

/// 单因子测试结果
#[derive(Debug, Clone)]
pub struct FactorTestResult {
    pub factor_name: String,
    /// IC 均值（因子值与未来收益的秩相关）
    pub ic_mean: f64,
    /// IC 标准差
    pub ic_std: f64,
    /// ICIR = |IC均值| / IC标准差
    pub icir: f64,
    /// IC > 0 的比例
    pub ic_hit_rate: f64,
}

/// 对单只基金做单因子 IC 测试。
///
/// - `horizon`: 未来收益的持有期（天）
/// - `step`: 采样步长（天），避免重叠样本
pub fn factor_ic_test(
    navs: &[(String, f64)],
    factor_idx: usize,
    horizon: usize,
    step: usize,
) -> Option<FactorTestResult> {
    let n = navs.len();
    if n < 300 {
        return None;
    }
    // 每个采样点：因子值 vs 未来 horizon 日收益
    let mut pairs: Vec<(f64, f64)> = Vec::new();
    let mut i = 260;
    while i + horizon < n {
        let f = compute_all(navs, i)[factor_idx];
        let base = navs[i].1;
        let future = navs[i + horizon].1;
        if base > 0.0 && future > 0.0 {
            pairs.push((f, future / base - 1.0));
        }
        i += step;
    }
    if pairs.len() < 20 {
        return None;
    }
    // 滚动窗口 IC：每 20 个样本算一个 IC
    let mut ics = Vec::new();
    for chunk in pairs.chunks(20) {
        if chunk.len() < 10 {
            continue;
        }
        let ic = spearman(
            &chunk.iter().map(|(f, _)| *f).collect::<Vec<_>>(),
            &chunk.iter().map(|(_, r)| *r).collect::<Vec<_>>(),
        );
        ics.push(ic);
    }
    if ics.is_empty() {
        return None;
    }
    let ic_mean = mean(&ics);
    let ic_std = std(&ics);
    Some(FactorTestResult {
        factor_name: FACTOR_NAMES[factor_idx].to_string(),
        ic_mean,
        ic_std,
        icir: if ic_std > 1e-12 { ic_mean.abs() / ic_std } else { 0.0 },
        ic_hit_rate: ics.iter().filter(|&&x| x > 0.0).count() as f64 / ics.len() as f64,
    })
}

/// Spearman 秩相关系数
fn spearman(xs: &[f64], ys: &[f64]) -> f64 {
    let rx = ranks(xs);
    let ry = ranks(ys);
    let n = xs.len() as f64;
    let mx = mean(&rx);
    let my = mean(&ry);
    let mut num = 0.0;
    let mut dx = 0.0;
    let mut dy = 0.0;
    for i in 0..xs.len() {
        num += (rx[i] - mx) * (ry[i] - my);
        dx += (rx[i] - mx).powi(2);
        dy += (ry[i] - my).powi(2);
    }
    if dx < 1e-12 || dy < 1e-12 {
        return 0.0;
    }
    num / (dx * dy).sqrt()
}

fn ranks(xs: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..xs.len()).collect();
    idx.sort_by(|&a, &b| xs[a].partial_cmp(&xs[b]).unwrap_or(std::cmp::Ordering::Equal));
    let mut r = vec![0.0; xs.len()];
    for (rank, &i) in idx.iter().enumerate() {
        r[i] = rank as f64;
    }
    r
}

/// 批量测试全部因子，返回按 |IC| 排序的结果
pub fn test_all_factors(
    funds: &HashMap<String, Vec<(String, f64)>>,
    horizon: usize,
    step: usize,
) -> Vec<FactorTestResult> {
    let mut agg: HashMap<usize, Vec<f64>> = HashMap::new();
    for navs in funds.values() {
        for fi in 0..FACTOR_NAMES.len() {
            if let Some(res) = factor_ic_test(navs, fi, horizon, step) {
                agg.entry(fi).or_default().push(res.ic_mean);
            }
        }
    }
    let mut results = Vec::new();
    for (fi, ics) in agg {
        if ics.len() < 10 {
            continue;
        }
        let ic_mean = mean(&ics);
        let ic_std = std(&ics);
        results.push(FactorTestResult {
            factor_name: FACTOR_NAMES[fi].to_string(),
            ic_mean,
            ic_std,
            icir: if ic_std > 1e-12 { ic_mean.abs() / ic_std } else { 0.0 },
            ic_hit_rate: ics.iter().filter(|&&x| x > 0.0).count() as f64 / ics.len() as f64,
        });
    }
    results.sort_by(|a, b| {
        b.ic_mean
            .abs()
            .partial_cmp(&a.ic_mean.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_navs(n: usize) -> Vec<(String, f64)> {
        (0..n)
            .map(|i| {
                (
                    format!("2020-01-{:02}", (i % 28) + 1),
                    1.0 + i as f64 * 0.001 + (i as f64 * 0.1).sin() * 0.02,
                )
            })
            .collect()
    }

    #[test]
    fn compute_all_length() {
        let navs = sample_navs(300);
        let f = compute_all(&navs, 299);
        assert_eq!(f.len(), FACTOR_NAMES.len());
        assert!(f.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn ic_test_runs() {
        let navs = sample_navs(800);
        let res = factor_ic_test(&navs, 0, 20, 10);
        assert!(res.is_some());
    }
}
