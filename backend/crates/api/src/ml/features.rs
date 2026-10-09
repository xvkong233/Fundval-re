//! 共享特征工程：训练（dataset.rs）与在线推理（compute.rs）必须使用
//! 完全相同的特征定义与顺序。此处集中实现，避免两边漂移。

/// 特征名（顺序与 [`build_features`] 返回一致）
pub const FEATURE_NAMES: [&str; 13] = [
    "dd_mag",       // 回撤幅度 [0,1]
    "ret5",         // 5日简单收益
    "ret20",        // 20日简单收益
    "vol20",        // 20日波动率（日收益标准差）
    "rsi14",        // 14日 RSI / 100
    "dist_ma20",    // 相对20日均线距离
    "dd_days_norm", // 回撤持续天数 / 20（封顶1）
    "rebound3",     // 近3日收益（是否已启动反弹）
    "dd_mag_sq",    // 回撤幅度的平方（非线性：深跌反弹更强）
    "dd_oversold",  // dd_mag * (1 - rsi14)（超卖深度交互）
    "dist_ma_sq",   // 均线距离的平方（偏离越远回归越强）
    "ret60",        // 60日简单收益（长周期趋势）
    "trend20",      // 20日线性趋势斜率（动量方向）
];

/// 透视因子特征名（追加在技术特征之后）
pub const LOOKTHROUGH_FEATURE_NAMES: [&str; 3] = [
    "lt_hhi",         // 持仓集中度 HHI [0,1]
    "lt_top_weight",  // 第一大重仓占比 [0,1]
    "lt_effective_n", // 有效持仓数 / 10 [0,1]
];

/// 完整特征名（13 技术 + 3 透视）
pub fn full_feature_names() -> Vec<String> {
    FEATURE_NAMES
        .iter()
        .chain(LOOKTHROUGH_FEATURE_NAMES.iter())
        .map(|s| s.to_string())
        .collect()
}

/// 兼容旧 13 维特征名
pub fn feature_names() -> Vec<String> {
    FEATURE_NAMES.iter().map(|s| s.to_string()).collect()
}

/// 对 navs[idx] 这一时点构造 8 维特征向量。
///
/// navs 按日期升序；idx 为当前下标。历史不足时按可用范围计算，
/// 与旧逻辑保持一致（compute.rs 的 `unwrap_or` 回退语义）。
pub fn build_features(navs: &[(String, f64)], idx: usize) -> Vec<f64> {
    let dd_mag = drawdown_mag(navs, idx, 252);
    let ret5 = simple_return(navs, idx, 5).unwrap_or(0.0);
    let ret20 = simple_return(navs, idx, 20).unwrap_or(ret5);
    let vol20 = vol(navs, idx, 20).unwrap_or(0.0);
    let rsi14 = rsi(navs, idx, 14).unwrap_or(0.5);
    let dist_ma20 = dist_ma(navs, idx, 20).unwrap_or(0.0);
    let dd_days_norm = (drawdown_days(navs, idx, 252) as f64 / 20.0).min(1.0);
    let rebound3 = simple_return(navs, idx, 3).unwrap_or(0.0);
    // 非线性项：均值回归强度与回撤深度呈非线性关系
    let dd_mag_sq = dd_mag * dd_mag;
    let dd_oversold = dd_mag * (1.0 - rsi14);
    let dist_ma_sq = dist_ma20 * dist_ma20;
    // 长周期特征：捕捉 20 天以上 horizon 的趋势信号
    let ret60 = simple_return(navs, idx, 60).unwrap_or(ret20);
    let trend20 = lin_trend(navs, idx, 20).unwrap_or(0.0);
    vec![
        dd_mag,
        ret5,
        ret20,
        vol20,
        rsi14,
        dist_ma20,
        dd_days_norm,
        rebound3,
        dd_mag_sq,
        dd_oversold,
        dist_ma_sq,
        ret60,
        trend20,
    ]
}

fn drawdown_mag(navs: &[(String, f64)], idx: usize, lookback: usize) -> f64 {
    if navs.is_empty() || idx >= navs.len() {
        return 0.0;
    }
    let lookback = lookback.max(1).min(idx + 1);
    let start = idx + 1 - lookback;
    let mut max_v = f64::MIN;
    for &(_, v) in navs.iter().take(idx + 1).skip(start) {
        if v > max_v {
            max_v = v;
        }
    }
    let now = navs[idx].1;
    if max_v <= 0.0 || now <= 0.0 {
        return 0.0;
    }
    ((max_v - now) / max_v).max(0.0)
}

/// 当前处于回撤中的持续天数（回撤起点到 idx 的天数；不在回撤中返回 0）
fn drawdown_days(navs: &[(String, f64)], idx: usize, lookback: usize) -> usize {
    if navs.is_empty() || idx >= navs.len() {
        return 0;
    }
    let lookback = lookback.max(1).min(idx + 1);
    let start = idx + 1 - lookback;
    let now = navs[idx].1;
    // 找到 lookback 窗口内的最高点位置
    let mut peak_idx = start;
    let mut peak_v = f64::MIN;
    for (k, &(_, v)) in navs.iter().enumerate().take(idx + 1).skip(start) {
        if v >= peak_v {
            peak_v = v;
            peak_idx = k;
        }
    }
    if now >= peak_v || peak_v <= 0.0 {
        return 0;
    }
    idx - peak_idx
}

fn simple_return(navs: &[(String, f64)], idx: usize, lookback: usize) -> Option<f64> {
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

fn vol(navs: &[(String, f64)], idx: usize, lookback: usize) -> Option<f64> {
    if idx < lookback || idx >= navs.len() {
        return None;
    }
    let start = idx + 1 - lookback;
    let mut rets: Vec<f64> = Vec::with_capacity(lookback);
    for i in (start + 1)..=idx {
        let prev = navs[i - 1].1;
        let now = navs[i].1;
        if prev <= 0.0 || now <= 0.0 {
            continue;
        }
        rets.push(now / prev - 1.0);
    }
    if rets.len() < 2 {
        return None;
    }
    let mean = rets.iter().sum::<f64>() / (rets.len() as f64);
    let var = rets
        .iter()
        .map(|r| {
            let d = r - mean;
            d * d
        })
        .sum::<f64>()
        / (rets.len() as f64);
    Some(var.sqrt())
}

/// 经典 RSI（Wilder 平滑的简化版：窗口内平均涨幅/平均跌幅）
fn rsi(navs: &[(String, f64)], idx: usize, period: usize) -> Option<f64> {
    if idx < period || idx >= navs.len() {
        return None;
    }
    let mut gain = 0.0_f64;
    let mut loss = 0.0_f64;
    for i in (idx + 1 - period)..=idx {
        let prev = navs[i - 1].1;
        let now = navs[i].1;
        if prev <= 0.0 || now <= 0.0 {
            continue;
        }
        let chg = now / prev - 1.0;
        if chg > 0.0 {
            gain += chg;
        } else {
            loss -= chg;
        }
    }
    let avg_gain = gain / period as f64;
    let avg_loss = loss / period as f64;
    if avg_gain + avg_loss < 1e-12 {
        return Some(0.5);
    }
    if avg_loss < 1e-12 {
        return Some(1.0);
    }
    let rs = avg_gain / avg_loss;
    Some(1.0 - 1.0 / (1.0 + rs))
}

/// 相对 N 日均线距离：(now - ma) / ma
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

/// 20 日（或 N 日）对数净值的线性趋势斜率（每天）
fn lin_trend(navs: &[(String, f64)], idx: usize, period: usize) -> Option<f64> {    if idx + 1 < period || idx >= navs.len() {
        return None;
    }
    let start = idx + 1 - period;
    let mut sx = 0.0_f64;
    let mut sy = 0.0_f64;
    let mut sxx = 0.0_f64;
    let mut sxy = 0.0_f64;
    let n = period as f64;
    for (k, &(_, v)) in navs.iter().enumerate().take(idx + 1).skip(start) {
        if v <= 0.0 {
            return None;
        }
        let x = (k - start) as f64;
        let y = v.ln();
        sx += x;
        sy += y;
        sxx += x * x;
        sxy += x * y;
    }
    let denom = n * sxx - sx * sx;
    if denom.abs() < 1e-12 {
        return Some(0.0);
    }
    Some((n * sxy - sx * sy) / denom)
}

/// 预测专用特征名（用于 OLS 净值预测）
pub const FORECAST_FEATURE_NAMES: [&str; 8] = [
    "ret1",      // 上一日对数收益（动量）
    "ret5",      // 5日对数收益
    "rsi14",     // 14日 RSI / 100
    "dist_ma20", // 相对20日均线距离（均值回归）
    "vol20",     // 20日波动率
    "dd_mag",    // 回撤幅度
    "ret20",     // 20日对数收益
    "vol_ratio", // vol5/vol20（波动率突变）
];

pub fn forecast_feature_names() -> Vec<String> {
    FORECAST_FEATURE_NAMES
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// 为预测下一日收益构造特征。navs 为截至 idx（含）的净值序列。
pub fn build_forecast_features(navs: &[f64], idx: usize) -> Option<Vec<f64>> {
    if navs.len() < 25 || idx >= navs.len() || idx < 24 {
        return None;
    }
    let labeled: Vec<(String, f64)> = navs
        .iter()
        .enumerate()
        .map(|(i, &v)| (format!("{i}"), v))
        .collect();
    let ret1 = (navs[idx] / navs[idx - 1]).ln();
    let ret5 = (navs[idx] / navs[idx - 5]).ln();
    let ret20 = (navs[idx] / navs[idx - 20]).ln();
    let rsi14 = rsi(&labeled, idx, 14).unwrap_or(0.5);
    let dist_ma20 = dist_ma(&labeled, idx, 20).unwrap_or(0.0);
    let vol20 = vol(&labeled, idx, 20).unwrap_or(0.01);
    let dd_mag = drawdown_mag(&labeled, idx, 60);
    let vol5 = vol(&labeled, idx, 5).unwrap_or(vol20);
    let vol_ratio = (vol5 / vol20.max(1e-6)).min(5.0);
    Some(vec![
        ret1, ret5, rsi14, dist_ma20, vol20, dd_mag, ret20, vol_ratio,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_match_legacy_for_basic_inputs() {
        // 单调上涨：无回撤，rsi=1, dist_ma>0
        let navs: Vec<(String, f64)> = (0..30)
            .map(|i| (format!("d{i}"), 1.0 + i as f64 * 0.01))
            .collect();
        let f = build_features(&navs, 29);
        assert_eq!(f.len(), 13);
        assert!((f[0] - 0.0).abs() < 1e-9); // dd_mag
        assert!((f[4] - 1.0).abs() < 1e-9); // rsi
        assert!(f[5] > 0.0); // dist_ma20
        assert_eq!(f[6], 0.0); // dd_days_norm
        assert!((f[8] - 0.0).abs() < 1e-9); // dd_mag_sq
        assert!((f[9] - 0.0).abs() < 1e-9); // dd_oversold
    }

    #[test]
    fn features_capture_drawdown() {
        // 先涨后跌：有回撤
        let mut navs: Vec<(String, f64)> = (0..20)
            .map(|i| (format!("d{i}"), 1.0 + i as f64 * 0.01))
            .collect();
        for i in 20..30 {
            navs.push((format!("d{i}"), 1.19 - (i - 20) as f64 * 0.02));
        }
        let f = build_features(&navs, 29);
        assert!(f[0] > 0.1, "dd_mag={}", f[0]); // 回撤约 15%
        assert!(f[4] < 0.5, "rsi={}", f[4]); // RSI 走弱
        assert!(f[5] < 0.0, "dist_ma={}", f[5]); // 跌破均线
        assert!(f[6] > 0.0, "dd_days={}", f[6]); // 回撤持续中
        assert!(f[7] < 0.0, "rebound3={}", f[7]); // 近3天仍在跌
    }
}
