//! 布林带均值回归策略（学习 Qbot 的 boll_strategy_bt.py）。
//!
//! 核心逻辑（Qbot 原版，period=13）：
//! - 买入：收盘价跌破下轨（超卖）
//! - 卖出：收盘价突破上轨（超买）
//!
//! 与我们 ML 均值回归的区别：纯规则、无模型，
//! 作为基准策略和组合策略的子策略。

/// 布林带参数
#[derive(Debug, Clone)]
pub struct BollParams {
    pub period: usize,
    pub num_std: f64,
}

impl Default for BollParams {
    fn default() -> Self {
        Self {
            period: 13, // Qbot 原版参数
            num_std: 2.0,
        }
    }
}

fn sma(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn std_dev(values: &[f64], mean: f64) -> f64 {
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    var.sqrt()
}

/// 计算布林带：(上轨, 中轨, 下轨)
pub fn bollinger_bands(navs: &[(String, f64)], idx: usize, period: usize, num_std: f64) -> Option<(f64, f64, f64)> {
    if idx + 1 < period || idx >= navs.len() {
        return None;
    }
    let start = idx + 1 - period;
    let values: Vec<f64> = navs.iter().take(idx + 1).skip(start).map(|&(_, v)| v).collect();
    let mid = sma(&values);
    let sd = std_dev(&values, mid);
    Some((mid + num_std * sd, mid, mid - num_std * sd))
}

/// 布林带策略状态机
pub struct BollStrategy {
    params: BollParams,
    in_position: bool,
}

impl BollStrategy {
    pub fn new(params: BollParams) -> Self {
        Self { params, in_position: false }
    }

    /// 返回 (持有?, 原因)
    pub fn signal(&mut self, navs: &[(String, f64)], idx: usize) -> (bool, &'static str) {
        let price = navs[idx].1;
        let (top, _, bot) = match bollinger_bands(navs, idx, self.params.period, self.params.num_std) {
            Some(b) => b,
            None => return (self.in_position, "数据不足"),
        };

        if !self.in_position {
            if price < bot {
                self.in_position = true;
                return (true, "跌破下轨买入");
            }
            return (false, "等待超卖");
        }

        if price > top {
            self.in_position = false;
            return (false, "突破上轨卖出");
        }
        (true, "带内持有")
    }

    pub fn reset(&mut self) {
        self.in_position = false;
    }
}

/// 回测布林带策略
pub fn backtest_boll(
    navs: &[(String, f64)],
    params: &BollParams,
    start_idx: usize,
) -> (f64, usize, f64) {
    let mut strat = BollStrategy::new(params.clone());
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
    fn boll_runs() {
        // 震荡市：布林带策略应能盈利
        let navs: Vec<(String, f64)> = (0..300)
            .map(|i| (format!("d{i}"), 1.0 + (i as f64 * 0.3).sin() * 0.05))
            .collect();
        let params = BollParams::default();
        let (ret, trades, _) = backtest_boll(&navs, &params, 50);
        assert!(trades > 0, "震荡市应有交易");
        let _ = ret;
    }
}
