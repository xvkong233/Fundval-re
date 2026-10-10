//! v3 特征工程：从8大开源策略提炼的ML特征。
//!
//! 核心思想：
//! 1. ATR标准化（Trader）：所有距离类特征除以ATR，消除波动率差异
//!    - 高波动基金的3%下跌是噪音，低波动基金的3%下跌是信号
//! 2. ADX趋势强度（Qbot）：均值回归在ADX低时有效，趋势跟踪在ADX高时有效
//! 3. 交互特征：让线性模型学到"状态依赖"的规律
//! 4. 唐奇安位置：价格在N日区间中的位置

use crate::ml::features::build_features;

/// v3 新增特征名
pub const V3_FEATURE_NAMES: [&str; 7] = [
    "adx14",            // 趋势强度 0-100
    "donchian_pos20",   // 20日高低区间位置 [0,1]
    "macd_hist_norm",   // MACD柱 / ATR（标准化动量）
    "dd_atr",           // 回撤幅度 / ATR（波动调整后的超卖）
    "dist_ma20_atr",    // 均线偏离 / ATR
    "rsi_adx_interact", // RSI * ADX/100（趋势中的超卖=危险）
    "ret20_adx_interact", // 20日收益 * ADX/100（趋势中的动量）
];

/// 构建 v3 特征 = v1的13个 + 7个新特征
pub fn build_features_v3(navs: &[(String, f64)], idx: usize) -> Vec<f64> {
    let mut v = build_features(navs, idx);

    let atr = calc_atr_ratio(navs, idx, 14).unwrap_or(0.01).max(0.001);
    let adx = adx_proxy(navs, idx, 14).unwrap_or(0.0);
    let adx_norm = (adx / 100.0).clamp(0.0, 1.0);

    // v1特征索引：0=dd_mag, 1=ret5, 2=ret20, 4=rsi14, 5=dist_ma20
    let dd_mag = v[0];
    let ret20 = v[2];
    let rsi14 = v[4];
    let dist_ma20 = v[5];

    v.push(adx);
    v.push(donchian_position(navs, idx, 20).unwrap_or(0.5));
    v.push(macd_hist(navs, idx).unwrap_or(0.0) / atr);
    v.push(dd_mag / atr);
    v.push(dist_ma20 / atr);
    v.push(rsi14 * adx_norm);
    v.push(ret20 * adx_norm);

    v
}

/// ATR（相对比例）
fn calc_atr_ratio(navs: &[(String, f64)], idx: usize, period: usize) -> Option<f64> {
    if idx < period || idx >= navs.len() {
        return None;
    }
    let mut sum = 0.0;
    let mut count = 0;
    for i in (idx + 1 - period)..=idx {
        let prev = navs[i - 1].1;
        let now = navs[i].1;
        if prev > 0.0 && now > 0.0 {
            sum += (now - prev).abs() / prev;
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    Some(sum / count as f64)
}

/// ADX代理：趋势强度 0-100
fn adx_proxy(navs: &[(String, f64)], idx: usize, period: usize) -> Option<f64> {
    if idx < period || idx >= navs.len() {
        return None;
    }
    let mut up = 0.0;
    let mut down = 0.0;
    for i in (idx + 1 - period)..=idx {
        let chg = navs[i].1 - navs[i - 1].1;
        if chg > 0.0 {
            up += chg;
        } else {
            down -= chg;
        }
    }
    let total = up + down;
    if total < 1e-12 {
        return Some(0.0);
    }
    Some((up - down).abs() / total * 100.0)
}

/// 唐奇安位置：价格在N日高低区间中的位置 [0,1]
fn donchian_position(navs: &[(String, f64)], idx: usize, period: usize) -> Option<f64> {
    if idx + 1 < period || idx >= navs.len() {
        return None;
    }
    let start = idx + 1 - period;
    let mut hi = f64::NEG_INFINITY;
    let mut lo = f64::INFINITY;
    for &(_, v) in navs.iter().take(idx + 1).skip(start) {
        if v > hi { hi = v; }
        if v < lo { lo = v; }
    }
    let now = navs[idx].1;
    if hi - lo < 1e-12 {
        return Some(0.5);
    }
    Some((now - lo) / (hi - lo))
}

/// MACD柱
fn macd_hist(navs: &[(String, f64)], idx: usize) -> Option<f64> {
    if idx < 35 || idx >= navs.len() {
        return None;
    }
    let closes: Vec<f64> = navs.iter().take(idx + 1).map(|&(_, v)| v).collect();
    let ema12 = ema(&closes, 12);
    let ema26 = ema(&closes, 26);
    let dif: Vec<f64> = ema12.iter().zip(ema26.iter()).map(|(a, b)| a - b).collect();
    let dea = ema(&dif, 9);
    let n = dif.len();
    if n == 0 || dea.len() != n {
        return None;
    }
    Some((dif[n - 1] - dea[n - 1]) * 2.0)
}

fn ema(values: &[f64], period: usize) -> Vec<f64> {
    let k = 2.0 / (period as f64 + 1.0);
    let mut out = Vec::with_capacity(values.len());
    let mut prev = values[0];
    out.push(prev);
    for &v in &values[1..] {
        prev = v * k + prev * (1.0 - k);
        out.push(prev);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v3_features_length() {
        let navs: Vec<(String, f64)> = (0..400)
            .map(|i| (format!("d{i}"), 1.0 + (i as f64 * 0.1).sin() * 0.03 + i as f64 * 0.0005))
            .collect();
        let f = build_features_v3(&navs, 399);
        assert_eq!(f.len(), 13 + 7);
        assert!(f.iter().all(|x| x.is_finite()), "所有特征应为有限值");
    }
}
