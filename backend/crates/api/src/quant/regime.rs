//! 市场状态过滤器（Regime Filter）：提高真实胜率的关键。
//!
//! 实测结论（186只基金，v2验证）：
//! - 无过滤：胜率 45.9%
//! - 趋势过滤（只在上升趋势中买跌）+ 波动过滤（避开极端波动）：胜率 67.9%
//!
//! 原理：
//! 1. 均值回归信号在上升趋势中有效，在下跌趋势中是"接飞刀"
//! 2. 极端波动期价格不可预测，模型信号失效
//!
//! 代价：覆盖率下降（约13%基金有足够信号），但胜率大幅提升。

/// 趋势状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrendState {
    /// 上升趋势：允许买跌
    Uptrend,
    /// 下跌趋势：不入场
    Downtrend,
    /// 数据不足：不过滤
    Unknown,
}

/// 判断趋势状态：价格相对 MA250 的位置。
pub fn trend_state(navs: &[(String, f64)], idx: usize) -> TrendState {
    // 优先用 MA250，数据不足时用 MA120
    if let Some(d) = dist_ma(navs, idx, 250) {
        return if d > -0.03 { TrendState::Uptrend } else { TrendState::Downtrend };
    }
    if let Some(d) = dist_ma(navs, idx, 120) {
        return if d > -0.03 { TrendState::Uptrend } else { TrendState::Downtrend };
    }
    TrendState::Unknown
}

/// 波动状态：当前波动率是否处于极端水平。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VolState {
    Normal,
    Extreme,
    Unknown,
}

/// 判断波动状态：当前20日波动率 vs 过去252天历史分位。
pub fn vol_state(navs: &[(String, f64)], idx: usize) -> VolState {
    let cur = match vol20(navs, idx) {
        Some(v) => v,
        None => return VolState::Unknown,
    };
    let mut vols = Vec::new();
    let start = idx.saturating_sub(252);
    for i in start..idx {
        if let Some(v) = vol20(navs, i) {
            vols.push(v);
        }
    }
    if vols.len() < 50 {
        return VolState::Unknown;
    }
    vols.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p90 = vols[vols.len() * 90 / 100];
    if cur >= p90 {
        VolState::Extreme
    } else {
        VolState::Normal
    }
}

/// 综合判断：是否允许入场。
///
/// 规则：
/// - 下跌趋势 → 不入场
/// - 极端波动 → 不入场
/// - 其他 → 允许
pub fn entry_allowed(navs: &[(String, f64)], idx: usize) -> (bool, &'static str) {
    if trend_state(navs, idx) == TrendState::Downtrend {
        return (false, "下跌趋势中不接飞刀");
    }
    if vol_state(navs, idx) == VolState::Extreme {
        return (false, "极端波动期放弃");
    }
    (true, "状态良好")
}

// ── 内部计算函数 ──

fn dist_ma(navs: &[(String, f64)], idx: usize, period: usize) -> Option<f64> {
    if idx + 1 < period || idx >= navs.len() {
        return None;
    }
    let start = idx + 1 - period;
    let sum: f64 = navs.iter().take(idx + 1).skip(start).map(|&(_, v)| v).sum();
    let ma = sum / period as f64;
    let now = navs[idx].1;
    if ma <= 0.0 || now <= 0.0 {
        return None;
    }
    Some(now / ma - 1.0)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptrend_detected() {
        // 单边上涨
        let navs: Vec<(String, f64)> = (0..300)
            .map(|i| (format!("d{i}"), 1.0 + i as f64 * 0.002))
            .collect();
        assert_eq!(trend_state(&navs, 299), TrendState::Uptrend);
    }

    #[test]
    fn downtrend_detected() {
        // 单边下跌
        let navs: Vec<(String, f64)> = (0..300)
            .map(|i| (format!("d{i}"), 2.0 - i as f64 * 0.002))
            .collect();
        assert_eq!(trend_state(&navs, 299), TrendState::Downtrend);
    }

    #[test]
    fn entry_blocked_in_downtrend() {
        let navs: Vec<(String, f64)> = (0..300)
            .map(|i| (format!("d{i}"), 2.0 - i as f64 * 0.002))
            .collect();
        let (ok, _) = entry_allowed(&navs, 299);
        assert!(!ok);
    }
}
