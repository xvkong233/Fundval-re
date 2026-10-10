//! 回测引擎：在历史数据上模拟量化交易，统计胜率与绩效。
//!
//! 设计原则：
//! - 无前视偏差：t 时刻只用 t 及之前的数据生成信号
//! - Walk-forward：滚动训练/测试，模拟实盘
//! - 包含交易成本
//! - 输出完整绩效指标

use serde::{Deserialize, Serialize};

use super::signal::TradingSignal;
use super::strategy::{ExitReason, Position, Strategy, Trade};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub initial_capital: f64,
    /// 训练窗口（天）
    pub train_window: usize,
    /// 测试窗口（天），滚动步长
    pub test_window: usize,
    /// 最小训练样本数
    pub min_train_samples: usize,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: 100_000.0,
            train_window: 252,
            test_window: 60,
            min_train_samples: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetrics {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    /// 胜率（核心指标，目标 80%+）
    pub win_rate: f64,
    pub total_net_pnl: f64,
    pub total_return_pct: f64,
    pub avg_win_pct: f64,
    pub avg_loss_pct: f64,
    /// 盈亏比
    pub profit_factor: f64,
    /// 最大回撤
    pub max_drawdown_pct: f64,
    /// 夏普比率（年化）
    pub sharpe_ratio: f64,
    pub avg_hold_days: f64,
    pub total_commission: f64,
}

impl BacktestMetrics {
    pub fn from_trades(trades: &[Trade], initial_capital: f64, equity_curve: &[f64]) -> Self {
        let total = trades.len();
        let wins = trades.iter().filter(|t| t.net_pnl > 0.0).count();
        let losses = total - wins;
        let win_rate = if total > 0 {
            wins as f64 / total as f64
        } else {
            0.0
        };

        let total_net_pnl: f64 = trades.iter().map(|t| t.net_pnl).sum();
        let total_return_pct = if initial_capital > 0.0 {
            total_net_pnl / initial_capital * 100.0
        } else {
            0.0
        };

        let win_pnls: Vec<f64> = trades
            .iter()
            .filter(|t| t.net_pnl > 0.0)
            .map(|t| t.return_pct)
            .collect();
        let loss_pnls: Vec<f64> = trades
            .iter()
            .filter(|t| t.net_pnl <= 0.0)
            .map(|t| t.return_pct)
            .collect();
        let avg_win_pct = if !win_pnls.is_empty() {
            win_pnls.iter().sum::<f64>() / win_pnls.len() as f64
        } else {
            0.0
        };
        let avg_loss_pct = if !loss_pnls.is_empty() {
            loss_pnls.iter().sum::<f64>() / loss_pnls.len() as f64
        } else {
            0.0
        };

        let gross_win: f64 = trades
            .iter()
            .filter(|t| t.net_pnl > 0.0)
            .map(|t| t.net_pnl)
            .sum();
        let gross_loss: f64 = trades
            .iter()
            .filter(|t| t.net_pnl <= 0.0)
            .map(|t| t.net_pnl.abs())
            .sum();
        let profit_factor = if gross_loss > 1e-9 {
            gross_win / gross_loss
        } else if gross_win > 0.0 {
            999.0
        } else {
            0.0
        };

        // 最大回撤
        let mut max_dd = 0.0_f64;
        let mut peak = f64::MIN;
        for &eq in equity_curve {
            if eq > peak {
                peak = eq;
            }
            if peak > 0.0 {
                let dd = (peak - eq) / peak * 100.0;
                if dd > max_dd {
                    max_dd = dd;
                }
            }
        }

        // 夏普（简化：日收益均值/标准差 * sqrt(252)）
        let sharpe_ratio = if equity_curve.len() > 2 {
            let mut rets: Vec<f64> = Vec::new();
            for w in equity_curve.windows(2) {
                if w[0] > 0.0 {
                    rets.push((w[1] - w[0]) / w[0]);
                }
            }
            if rets.len() > 1 {
                let mean = rets.iter().sum::<f64>() / rets.len() as f64;
                let var = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>()
                    / rets.len() as f64;
                let std = var.sqrt();
                if std > 1e-9 {
                    mean / std * (252.0_f64).sqrt()
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };

        let avg_hold_days = if total > 0 {
            trades.iter().map(|t| t.days_held as f64).sum::<f64>() / total as f64
        } else {
            0.0
        };
        let total_commission: f64 = trades.iter().map(|t| t.commission).sum();

        Self {
            total_trades: total,
            winning_trades: wins,
            losing_trades: losses,
            win_rate,
            total_net_pnl,
            total_return_pct,
            avg_win_pct,
            avg_loss_pct,
            profit_factor,
            max_drawdown_pct: max_dd,
            sharpe_ratio,
            avg_hold_days,
            total_commission,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub fund_code: String,
    pub metrics: BacktestMetrics,
    pub trades: Vec<Trade>,
    pub equity_curve: Vec<(String, f64)>,
}

/// 单基金回测：在给定净值序列上运行策略。
///
/// `navs`: 按日期升序的 (date, nav) 序列。
/// `signal_fn`: 给定日期索引，返回该日的交易信号（由外部模型生成，保证无前视）。
pub fn backtest_single_fund(
    fund_code: &str,
    navs: &[(String, f64)],
    signal_fn: &dyn Fn(usize) -> Option<TradingSignal>,
    strategy: &Strategy,
    initial_capital: f64,
) -> BacktestResult {
    let mut cash = initial_capital;
    let mut position: Option<Position> = None;
    let mut trades: Vec<Trade> = Vec::new();
    let mut equity_curve: Vec<(String, f64)> = Vec::with_capacity(navs.len());

    for (i, (date, nav)) in navs.iter().enumerate() {
        if *nav <= 0.0 {
            continue;
        }

        // 1. 更新持仓并检查出场
        if let Some(pos) = position.as_mut() {
            strategy.update_position(pos, *nav);
            let sig = signal_fn(i);
            if let Some(reason) =
                strategy.should_exit(pos, *nav, sig.as_ref())
            {
                let trade = strategy.settle_trade(pos, date, *nav, reason);
                cash += trade.exit_value - trade.commission;
                // 注意：买入时已扣 entry_value，这里加回 exit_value 减佣金
                // 为简化，cash 跟踪：买入时 cash -= entry_value + 买入佣金
                trades.push(trade);
                position = None;
            }
        }

        // 2. 检查入场（无持仓时）
        if position.is_none() {
            if let Some(sig) = signal_fn(i) {
                if let Some(action) =
                    strategy.should_enter(&sig, false, cash)
                {
                    match action {
                        super::strategy::StrategyAction::Enter {
                            position_value,
                            signal,
                        } => {
                            let buy_commission =
                                position_value * strategy.config.commission_rate;
                            let total_cost = position_value + buy_commission;
                            if total_cost <= cash && position_value >= strategy.config.min_position_value {
                                let shares = position_value / nav;
                                cash -= total_cost;
                                // ATR自适应止损（Trader海龟交易法思想）：波动大则止损宽，波动小则止损窄
                                let (take_profit_nav, stop_loss_nav) = if strategy.config.use_atr_stop {
                                    match crate::quant::strategy::calc_atr(navs, i, strategy.config.atr_period) {
                                        Some(atr) => (
                                            nav * (1.0 + strategy.config.atr_profit_mult * atr),
                                            nav * (1.0 - strategy.config.atr_stop_mult * atr),
                                        ),
                                        None => (
                                            nav * (1.0 + signal.take_profit_pct / 100.0),
                                            nav * (1.0 - signal.stop_loss_pct / 100.0),
                                        ),
                                    }
                                } else {
                                    (
                                        nav * (1.0 + signal.take_profit_pct / 100.0),
                                        nav * (1.0 - signal.stop_loss_pct / 100.0),
                                    )
                                };
                                position = Some(Position {
                                    fund_code: fund_code.to_string(),
                                    entry_date: date.clone(),
                                    entry_nav: *nav,
                                    shares,
                                    entry_value: position_value,
                                    highest_nav: *nav,
                                    take_profit_nav,
                                    stop_loss_nav,
                                    days_held: 0,
                                    entry_signal_strength: signal.strength,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // 3. 记录权益曲线
        let pos_value = position
            .as_ref()
            .map(|p| p.shares * nav)
            .unwrap_or(0.0);
        equity_curve.push((date.clone(), cash + pos_value));
    }

    // 期末强制平仓
    if let Some(pos) = position.take() {
        if let Some((date, nav)) = navs.last() {
            if *nav > 0.0 {
                let trade = strategy.settle_trade(
                    &pos,
                    date,
                    *nav,
                    ExitReason::TimeExit,
                );
                cash += trade.exit_value - trade.commission;
                trades.push(trade);
            }
        }
    }

    let equity_values: Vec<f64> = equity_curve.iter().map(|(_, v)| *v).collect();
    let metrics =
        BacktestMetrics::from_trades(&trades, initial_capital, &equity_values);

    // 修正 cash 计算：上面的逻辑中买入时扣了 total_cost，卖出时加了 exit_value - commission
    // 需要确保 equity_curve 最后一项反映真实权益
    if let Some(last) = equity_curve.last_mut() {
        // 已经通过 cash + pos_value 计算，无需调整
        let _ = last;
    }

    BacktestResult {
        fund_code: fund_code.to_string(),
        metrics,
        trades,
        equity_curve,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_win_rate() {
        let trades = vec![
            Trade {
                fund_code: "a".into(),
                entry_date: "d1".into(),
                exit_date: "d2".into(),
                entry_nav: 1.0,
                exit_nav: 1.05,
                shares: 100.0,
                entry_value: 100.0,
                exit_value: 105.0,
                gross_pnl: 5.0,
                commission: 0.3,
                net_pnl: 4.7,
                return_pct: 4.7,
                days_held: 5,
                exit_reason: ExitReason::TakeProfit,
                entry_strength: super::super::signal::SignalStrength::Strong,
            },
            Trade {
                fund_code: "a".into(),
                entry_date: "d3".into(),
                exit_date: "d4".into(),
                entry_nav: 1.0,
                exit_nav: 0.98,
                shares: 100.0,
                entry_value: 100.0,
                exit_value: 98.0,
                gross_pnl: -2.0,
                commission: 0.3,
                net_pnl: -2.3,
                return_pct: -2.3,
                days_held: 3,
                exit_reason: ExitReason::StopLoss,
                entry_strength: super::super::signal::SignalStrength::Strong,
            },
        ];
        let m = BacktestMetrics::from_trades(&trades, 10000.0, &[10000.0, 10002.0]);
        assert_eq!(m.total_trades, 2);
        assert!((m.win_rate - 0.5).abs() < 1e-9);
    }
}
