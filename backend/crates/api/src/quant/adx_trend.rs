//! ADX趋势启动策略（学习 Qbot 的 adx_strategy.py）。
//!
//! 核心逻辑（捕捉趋势刚启动的时刻）：
//! - 三均线多头排列：EMA13 > EMA55 > EMA89
//! - ADX 上升但 < 25（趋势正在形成，未过热）
//! - MACD 柱上升（动量确认）
//! - 持有 3 天后卖出（短线趋势跟踪）
//!
//! 针对基金适配：用收盘价序列计算，不用HLC（基金无OHLC）。

/// EMA 计算
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

/// 简化 ADX：用方向变动率近似趋势强度。
///
/// 标准ADX需要HLC，基金只有收盘价，用 |N日动量| / N日波动 近似。
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
    // DX = |DI+ - DI-| / (DI+ + DI-) * 100
    Some((up - down).abs() / total * 100.0)
}

/// MACD 柱（DIF - DEA）
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

/// ADX策略参数
#[derive(Debug, Clone)]
pub struct AdxParams {
    pub ema_short: usize,
    pub ema_mid: usize,
    pub ema_long: usize,
    pub adx_threshold: f64,
    pub hold_days: usize,
}

impl Default for AdxParams {
    fn default() -> Self {
        Self {
            ema_short: 13,
            ema_mid: 55,
            ema_long: 89,
            adx_threshold: 25.0,
            hold_days: 3,
        }
    }
}

/// 检查 idx 处是否触发买入信号
pub fn adx_buy_signal(navs: &[(String, f64)], idx: usize, params: &AdxParams) -> bool {
    if idx < 90 || idx >= navs.len() {
        return false;
    }
    let closes: Vec<f64> = navs.iter().take(idx + 1).map(|&(_, v)| v).collect();

    // 三均线多头排列
    let e1 = ema(&closes, params.ema_short);
    let e2 = ema(&closes, params.ema_mid);
    let e3 = ema(&closes, params.ema_long);
    let n = closes.len();
    if !(e1[n - 1] > e2[n - 1] && e2[n - 1] > e3[n - 1]) {
        return false;
    }

    // ADX 上升但 < 阈值
    let adx_now = match adx_proxy(navs, idx, 14) {
        Some(v) => v,
        None => return false,
    };
    let adx_prev = match adx_proxy(navs, idx - 1, 14) {
        Some(v) => v,
        None => return false,
    };
    if !(adx_now <= params.adx_threshold && adx_now > adx_prev) {
        return false;
    }

    // MACD 柱上升
    let hist_now = match macd_hist(navs, idx) {
        Some(v) => v,
        None => return false,
    };
    let hist_prev = match macd_hist(navs, idx - 1) {
        Some(v) => v,
        None => return false,
    };
    if hist_now <= hist_prev {
        return false;
    }

    true
}

/// 回测 ADX 策略：信号触发买入，持有 hold_days 天后卖出
pub fn backtest_adx(
    navs: &[(String, f64)],
    params: &AdxParams,
    start_idx: usize,
) -> (f64, usize, f64) {
    let mut cash = 1.0;
    let mut shares = 0.0;
    let mut entry_idx: Option<usize> = None;
    let mut trades = 0;
    let mut peak = 1.0;
    let mut max_dd = 0.0;

    for idx in start_idx..navs.len() {
        let price = navs[idx].1;
        let value = cash + shares * price;
        if value > peak {
            peak = value;
        }
        let dd = (peak - value) / peak;
        if dd > max_dd {
            max_dd = dd;
        }

        // 持有到期卖出
        if let Some(ei) = entry_idx {
            if idx >= ei + params.hold_days && shares > 0.0 {
                cash = shares * price * (1.0 - 0.005);
                shares = 0.0;
                entry_idx = None;
                trades += 1;
            }
        }

        // 无持仓时检查买入信号
        if shares == 0.0 && entry_idx.is_none() && adx_buy_signal(navs, idx, params) {
            shares = cash * (1.0 - 0.0015) / price;
            cash = 0.0;
            entry_idx = Some(idx);
        }
    }
    let final_value = cash + shares * navs[navs.len() - 1].1;
    (final_value - 1.0, trades, max_dd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adx_runs() {
        // 构造趋势启动：先横盘后加速上涨
        let mut navs: Vec<(String, f64)> = Vec::new();
        for i in 0..300 {
            let p = if i < 150 {
                1.0 + (i as f64 * 0.05).sin() * 0.01
            } else {
                1.0 + (i - 150) as f64 * 0.003
            };
            navs.push((format!("d{i}"), p));
        }
        let params = AdxParams::default();
        let (ret, trades, _) = backtest_adx(&navs, &params, 100);
        // 不强制要求交易（信号条件严格），只确保不panic
        let _ = (ret, trades);
    }
}
