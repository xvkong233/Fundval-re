//! 唐奇安通道突破策略（学习 Trader 项目"大哥2.2"海龟交易法）。
//!
//! 核心逻辑：
//! - 入场：收盘价突破 N 日最高价（做多突破）
//! - 出场：收盘价跌破 M 日最低价（M < N，反向突破）
//! - 止损：ATR 追踪止损（最高价 - k*ATR）
//! - 趋势过滤：短期均线 > 长期均线才做多
//!
//! 与我们的均值回归策略互补：趋势市中突破策略有效，
//! 震荡市中均值回归有效。

use crate::quant::strategy::calc_atr;

/// 唐奇安策略参数
#[derive(Debug, Clone)]
pub struct DonchianParams {
    /// 入场突破周期（N日最高）
    pub entry_period: usize,
    /// 出场突破周期（M日最低）
    pub exit_period: usize,
    /// 短期均线（趋势过滤）
    pub short_ma: usize,
    /// 长期均线（趋势过滤）
    pub long_ma: usize,
    /// ATR止损倍数
    pub atr_stop_mult: f64,
    /// ATR计算周期
    pub atr_period: usize,
}

impl Default for DonchianParams {
    fn default() -> Self {
        Self {
            entry_period: 20,
            exit_period: 10,
            short_ma: 20,
            long_ma: 60,
            atr_stop_mult: 2.0,
            atr_period: 14,
        }
    }
}

fn highest(navs: &[(String, f64)], idx: usize, period: usize) -> Option<f64> {
    if idx + 1 < period || idx >= navs.len() {
        return None;
    }
    let start = idx + 1 - period;
    Some(
        navs.iter().take(idx + 1).skip(start)
            .map(|&(_, v)| v)
            .fold(f64::NEG_INFINITY, f64::max),
    )
}

fn lowest(navs: &[(String, f64)], idx: usize, period: usize) -> Option<f64> {
    if idx + 1 < period || idx >= navs.len() {
        return None;
    }
    let start = idx + 1 - period;
    Some(
        navs.iter().take(idx + 1).skip(start)
            .map(|&(_, v)| v)
            .fold(f64::INFINITY, f64::min),
    )
}

fn sma(navs: &[(String, f64)], idx: usize, period: usize) -> Option<f64> {
    if idx + 1 < period || idx >= navs.len() {
        return None;
    }
    let start = idx + 1 - period;
    let sum: f64 = navs.iter().take(idx + 1).skip(start).map(|&(_, v)| v).sum();
    Some(sum / period as f64)
}

/// 唐奇安策略状态机
pub struct DonchianStrategy {
    params: DonchianParams,
    in_position: bool,
    entry_price: f64,
    highest_since_entry: f64,
}

impl DonchianStrategy {
    pub fn new(params: DonchianParams) -> Self {
        Self {
            params,
            in_position: false,
            entry_price: 0.0,
            highest_since_entry: 0.0,
        }
    }

    /// 返回 (持有?, 原因)
    pub fn signal(&mut self, navs: &[(String, f64)], idx: usize) -> (bool, &'static str) {
        let price = navs[idx].1;

        if !self.in_position {
            // 趋势过滤
            let short = sma(navs, idx, self.params.short_ma);
            let long = sma(navs, idx, self.params.long_ma);
            let trend_ok = match (short, long) {
                (Some(s), Some(l)) => s > l,
                _ => false,
            };
            if !trend_ok {
                return (false, "均线空头，不突破");
            }
            // 突破入场：收盘价 >= 前N日最高（不含当日，避免未来函数用前一日最高）
            if idx > 0 {
                if let Some(prev_high) = highest(navs, idx - 1, self.params.entry_period) {
                    if price >= prev_high {
                        self.in_position = true;
                        self.entry_price = price;
                        self.highest_since_entry = price;
                        return (true, "唐奇安突破入场");
                    }
                }
            }
            return (false, "等待突破");
        }

        // 持有中
        if price > self.highest_since_entry {
            self.highest_since_entry = price;
        }

        // ATR追踪止损
        if let Some(atr) = calc_atr(navs, idx, self.params.atr_period) {
            let stop = self.highest_since_entry - self.params.atr_stop_mult * atr * self.highest_since_entry;
            if price <= stop {
                self.in_position = false;
                return (false, "ATR追踪止损");
            }
        }

        // 反向突破出场
        if idx > 0 {
            if let Some(prev_low) = lowest(navs, idx - 1, self.params.exit_period) {
                if price <= prev_low {
                    self.in_position = false;
                    return (false, "跌破退出通道");
                }
            }
        }

        (true, "通道内持有")
    }

    pub fn reset(&mut self) {
        self.in_position = false;
        self.entry_price = 0.0;
        self.highest_since_entry = 0.0;
    }
}

/// 回测唐奇安策略
pub fn backtest_donchian(
    navs: &[(String, f64)],
    params: &DonchianParams,
    start_idx: usize,
) -> (f64, usize, f64) {
    let mut strat = DonchianStrategy::new(params.clone());
    let mut cash = 1.0;
    let mut shares = 0.0;
    let mut trades = 0;
    let mut peak = 1.0;
    let mut max_dd = 0.0;

    for idx in start_idx..navs.len() {
        let price = navs[idx].1;
        let (hold, _) = strat.signal(navs, idx);
        let value = cash + shares * price;
        if value > peak {
            peak = value;
        }
        let dd = (peak - value) / peak;
        if dd > max_dd {
            max_dd = dd;
        }

        if hold && shares == 0.0 && cash > 0.0 {
            shares = cash * (1.0 - 0.0015) / price;
            cash = 0.0;
            trades += 1;
        } else if !hold && shares > 0.0 {
            cash = shares * price * (1.0 - 0.005);
            shares = 0.0;
            trades += 1;
        }
    }
    let final_value = cash + shares * navs[navs.len() - 1].1;
    (final_value - 1.0, trades, max_dd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn donchian_runs() {
        // 先横盘后突破
        let mut navs: Vec<(String, f64)> = Vec::new();
        for i in 0..300 {
            let p = if i < 200 {
                1.0 + (i as f64 * 0.1).sin() * 0.02 // 横盘
            } else {
                1.0 + (i - 200) as f64 * 0.005 // 突破上涨
            };
            navs.push((format!("d{i}"), p));
        }
        let params = DonchianParams::default();
        let (ret, trades, _) = backtest_donchian(&navs, &params, 100);
        assert!(trades > 0, "应有突破交易");
        let _ = ret;
    }
}
