//! 多策略组合框架（参考 VNPy 的多策略管理思想）。
//!
//! 单一策略在某些市场状态下会失效，多策略组合通过投票/加权
//! 降低单一策略风险。针对基金（只能做多、T+1）设计：
//!
//! 策略池：
//! 1. 趋势跟踪（trend）
//! 2. 均值回归（mean_reversion）：超卖反弹
//! 3. 形态突破（pattern）：形态方向性
//! 4. ML 信号（ml）：现有逻辑回归模型
//! 5. 买入持有（buy_hold）：基准
//!
//! 组合方式：
//! - 投票制：多数策略看多才持有
//! - 加权制：按策略历史表现加权

use crate::quant::{factors, patterns, trend};

/// 子策略信号
#[derive(Debug, Clone)]
pub struct StrategyVote {
    pub name: &'static str,
    /// -1 看空/卖出，0 中性，+1 看多/买入
    pub vote: i8,
    /// 置信度 [0,1]
    pub confidence: f64,
    pub reason: String,
}

/// 组合模式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CombineMode {
    /// 多数投票：总票数 > 0 则持有
    Majority,
    /// 加权投票：按权重加权，总分 > 阈值则持有
    Weighted { threshold: f64 },
}

/// 多策略组合
pub struct StrategyPortfolio {
    pub mode: CombineMode,
    /// 策略权重（用于加权模式）
    pub weights: [f64; 4], // trend, mean_reversion, pattern, ml
    trend_strat: trend::TrendStrategy,
}

impl StrategyPortfolio {
    pub fn new(mode: CombineMode) -> Self {
        Self {
            mode,
            weights: [0.3, 0.25, 0.15, 0.3],
            trend_strat: trend::TrendStrategy::new(trend::TrendParams::default()),
        }
    }

    /// 收集各子策略投票
    pub fn collect_votes(
        &mut self,
        navs: &[(String, f64)],
        idx: usize,
        ml_proba: Option<f64>,
    ) -> Vec<StrategyVote> {
        let f = factors::compute_all(navs, idx);
        let fi = |name: &str| -> f64 {
            factors::FACTOR_NAMES
                .iter()
                .position(|&n| n == name)
                .map(|i| f[i])
                .unwrap_or(0.0)
        };

        let mut votes = Vec::new();

        // 1. 趋势跟踪
        let (hold, reason) = self.trend_strat.signal(navs, idx);
        votes.push(StrategyVote {
            name: "trend",
            vote: if hold { 1 } else { -1 },
            confidence: (fi("ma_align").abs() * 0.5 + 0.5).min(1.0),
            reason: reason.to_string(),
        });

        // 2. 均值回归：RSI 超卖 + 远离均线 -> 看多；RSI 超买 -> 看空
        let rsi = fi("rsi_14");
        let pctb = fi("boll_pctb");
        let mr_vote = if rsi < 0.3 && pctb < 0.2 {
            1
        } else if rsi > 0.7 && pctb > 0.8 {
            -1
        } else {
            0
        };
        votes.push(StrategyVote {
            name: "mean_reversion",
            vote: mr_vote,
            confidence: if mr_vote != 0 { 0.7 } else { 0.3 },
            reason: format!("RSI={rsi:.2}, %b={pctb:.2}"),
        });

        // 3. 形态识别
        let pat = patterns::detect_pattern(navs, idx);
        let ma_st = patterns::ma_state(navs, idx);
        let pat_vote = pat.bias();
        votes.push(StrategyVote {
            name: "pattern",
            vote: pat_vote,
            confidence: if pat_vote != 0 { 0.6 } else { 0.2 },
            reason: format!("{} / {ma_st:?}", pat.name()),
        });

        // 4. ML 信号
        let ml_vote = match ml_proba {
            Some(p) if p >= 0.65 => 1,
            Some(p) if p <= 0.35 => -1,
            _ => 0,
        };
        votes.push(StrategyVote {
            name: "ml",
            vote: ml_vote,
            confidence: ml_proba.map(|p| (p - 0.5).abs() * 2.0).unwrap_or(0.0),
            reason: format!("ML proba={:.2}", ml_proba.unwrap_or(0.5)),
        });

        votes
    }

    /// 综合决策：true = 持有/买入
    pub fn decide(&mut self, navs: &[(String, f64)], idx: usize, ml_proba: Option<f64>) -> (bool, Vec<StrategyVote>) {
        let votes = self.collect_votes(navs, idx, ml_proba);
        let hold = match self.mode {
            CombineMode::Majority => {
                let sum: i32 = votes.iter().map(|v| v.vote as i32).sum();
                sum > 0
            }
            CombineMode::Weighted { threshold } => {
                let score: f64 = votes
                    .iter()
                    .zip(self.weights.iter())
                    .map(|(v, w)| v.vote as f64 * v.confidence * w)
                    .sum();
                score > threshold
            }
        };
        (hold, votes)
    }

    pub fn reset(&mut self) {
        self.trend_strat.reset();
    }
}

/// 在单只基金上回测组合策略
pub fn backtest_portfolio(
    navs: &[(String, f64)],
    mode: CombineMode,
    ml_probas: &[Option<f64>],
    start_idx: usize,
) -> (f64, usize, f64) {
    let mut pf = StrategyPortfolio::new(mode);
    let mut cash = 1.0;
    let mut shares = 0.0;
    let mut trades = 0;
    let mut peak_value = 1.0;
    let mut max_dd = 0.0;

    for idx in start_idx..navs.len() {
        let price = navs[idx].1;
        let ml_p = ml_probas.get(idx).copied().flatten();
        let (hold, _) = pf.decide(navs, idx, ml_p);
        let value = cash + shares * price;
        if value > peak_value {
            peak_value = value;
        }
        let dd = (peak_value - value) / peak_value;
        if dd > max_dd {
            max_dd = dd;
        }

        if hold && shares == 0.0 && cash > 0.0 {
            let fee = 0.0015;
            shares = cash * (1.0 - fee) / price;
            cash = 0.0;
            trades += 1;
        } else if !hold && shares > 0.0 {
            let fee = 0.005;
            cash = shares * price * (1.0 - fee);
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
    fn portfolio_runs() {
        let navs: Vec<(String, f64)> = (0..400)
            .map(|i| {
                (
                    format!("d{i}"),
                    1.0 + i as f64 * 0.001 + (i as f64 * 0.15).sin() * 0.04,
                )
            })
            .collect();
        let ml: Vec<Option<f64>> = vec![None; 400];
        let (ret, trades, mdd) = backtest_portfolio(&navs, CombineMode::Majority, &ml, 250);
        assert!(mdd >= 0.0);
        let _ = (ret, trades);
    }
}
