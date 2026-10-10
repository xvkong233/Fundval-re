//! 趋势跟踪策略（参考 Trader 项目"古老但是有效"的简单趋势跟踪）。
//!
//! 核心思想：只在上升趋势中持有，趋势破坏时离场。
//! 针对开放式基金（T+1、无杠杆、无做空）适配：
//! - 信号 = 是否持有（多空二值），不做空
//! - 入场：价格站上长期均线 + 短期均线多头排列
//! - 出场：价格跌破长期均线 或 止损触发
//! - 移动止盈：从高点回撤超过阈值时离场（保护利润）

use crate::quant::factors;

/// 趋势策略参数
#[derive(Debug, Clone)]
pub struct TrendParams {
    /// 长期均线周期（趋势判断）
    pub long_ma: usize,
    /// 短期均线周期（入场确认）
    pub short_ma: usize,
    /// 止损幅度（从入场价）
    pub stop_loss: f64,
    /// 移动止盈回撤（从持有期高点）
    pub trailing_stop: f64,
    /// 入场时要求价格高于长期均线的幅度（过滤假突破）
    pub entry_buffer: f64,
}

impl Default for TrendParams {
    fn default() -> Self {
        Self {
            long_ma: 120,
            short_ma: 20,
            stop_loss: 0.08,
            trailing_stop: 0.10,
            entry_buffer: 0.02,
        }
    }
}

/// 趋势策略状态机
pub struct TrendStrategy {
    params: TrendParams,
    /// 是否持有
    pub in_position: bool,
    /// 入场价
    entry_price: f64,
    /// 持有期最高价
    peak: f64,
}

impl TrendStrategy {
    pub fn new(params: TrendParams) -> Self {
        Self {
            params,
            in_position: false,
            entry_price: 0.0,
            peak: 0.0,
        }
    }

    /// 对 navs[idx] 给出信号：true = 持有/买入，false = 空仓/卖出。
    /// 返回 (should_hold, reason)
    pub fn signal(&mut self, navs: &[(String, f64)], idx: usize) -> (bool, &'static str) {
        let f = factors::compute_all(navs, idx);
        let fi = |name: &str| -> f64 {
            factors::FACTOR_NAMES
                .iter()
                .position(|&n| n == name)
                .map(|i| f[i])
                .unwrap_or(0.0)
        };
        let price = navs[idx].1;

        // 用 dist_ma 因子近似均线位置
        let long_key = match self.params.long_ma {
            60 => "dist_ma60",
            120 => "dist_ma120",
            250 => "dist_ma250",
            _ => "dist_ma120",
        };
        let dist_long = fi(long_key);
        let dist_short = fi("dist_ma20");
        let ma_align = fi("ma_align");

        if !self.in_position {
            // 入场条件：站上长期均线 + 缓冲 + 短期不处于下跌（动量确认）
            let above_long = dist_long > self.params.entry_buffer;
            let momentum_ok = dist_short > -0.01; // 短期均线未破位
            let align_ok = ma_align > -0.2; // 不要求完全多头，但不能严重空头
            if above_long && momentum_ok && align_ok {
                self.in_position = true;
                self.entry_price = price;
                self.peak = price;
                return (true, "趋势入场");
            }
            return (false, "空仓等待");
        }

        // 持有中：更新峰值
        if price > self.peak {
            self.peak = price;
        }

        // 出场条件1：止损
        if price < self.entry_price * (1.0 - self.params.stop_loss) {
            self.in_position = false;
            return (false, "止损离场");
        }
        // 出场条件2：移动止盈
        if price < self.peak * (1.0 - self.params.trailing_stop) {
            self.in_position = false;
            return (false, "移动止盈");
        }
        // 出场条件3：趋势破坏（跌破长期均线）
        if dist_long < -0.02 {
            self.in_position = false;
            return (false, "趋势破坏");
        }
        (true, "趋势持有")
    }

    pub fn reset(&mut self) {
        self.in_position = false;
        self.entry_price = 0.0;
        self.peak = 0.0;
    }
}

/// 在单只基金上运行趋势策略回测，返回 (总收益率, 交易次数, 最大回撤)。
///
/// 考虑申购费 0.15%、赎回费 0.5%（持有>7天假设）。
pub fn backtest_trend(
    navs: &[(String, f64)],
    params: &TrendParams,
    start_idx: usize,
) -> (f64, usize, f64) {
    let mut strat = TrendStrategy::new(params.clone());
    let mut cash = 1.0;
    let mut shares = 0.0;
    let mut trades = 0;
    let mut peak_value = 1.0;
    let mut max_dd = 0.0;

    for idx in start_idx..navs.len() {
        let price = navs[idx].1;
        let (hold, _) = strat.signal(navs, idx);
        let value = cash + shares * price;
        if value > peak_value {
            peak_value = value;
        }
        let dd = (peak_value - value) / peak_value;
        if dd > max_dd {
            max_dd = dd;
        }

        if hold && shares == 0.0 && cash > 0.0 {
            // 买入
            let fee = 0.0015;
            shares = cash * (1.0 - fee) / price;
            cash = 0.0;
            trades += 1;
        } else if !hold && shares > 0.0 {
            // 卖出
            let fee = 0.005;
            cash = shares * price * (1.0 - fee);
            shares = 0.0;
            trades += 1;
        }
    }
    let final_value = cash + shares * navs[navs.len() - 1].1;
    (final_value - 1.0, trades, max_dd)
}

/// 买入持有基准（同口径，含费用）
pub fn backtest_buy_hold(navs: &[(String, f64)], start_idx: usize) -> (f64, f64) {
    let buy_price = navs[start_idx].1;
    let sell_price = navs[navs.len() - 1].1;
    let ret = (sell_price / buy_price) * (1.0 - 0.0015) * (1.0 - 0.005) - 1.0;
    // 最大回撤
    let mut peak = buy_price;
    let mut mdd = 0.0;
    for (_, p) in &navs[start_idx..] {
        if *p > peak {
            peak = *p;
        }
        let dd = (peak - p) / peak;
        if dd > mdd {
            mdd = dd;
        }
    }
    (ret, mdd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trend_runs() {
        let navs: Vec<(String, f64)> = (0..600)
            .map(|i| {
                (
                    format!("d{i}"),
                    1.0 + i as f64 * 0.002 + (i as f64 * 0.2).sin() * 0.03,
                )
            })
            .collect();
        let params = TrendParams::default();
        let (ret, trades, mdd) = backtest_trend(&navs, &params, 300);
        assert!(trades > 0);
        assert!(mdd >= 0.0);
        let _ = ret;
    }
}
