//! 量化交易策略引擎：定义何时进场、何时出场、仓位多大。
//!
//! 策略逻辑（追求 80%+ 胜率）：
//! - 入场：仅在强信号（综合概率 >= 0.80）时入场，宁可错过不做错
//! - 出场：止盈 / 止损 / 超时 / 信号反转，四者任一触发即出场
//! - 仓位：固定分数 + 凯利公式调整，单笔风险控制在 1-2%

use serde::{Deserialize, Serialize};

use super::signal::{SignalStrength, TradingSignal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// 入场概率阈值
    pub enter_threshold: f64,
    /// 最大持有天数
    pub max_hold_days: usize,
    /// 是否启用追踪止损
    pub use_trailing_stop: bool,
    /// 追踪止损回撤比例（从最高点回落）
    pub trailing_stop_pct: f64,
    /// 单笔风险占总资金比例
    pub risk_per_trade: f64,
    /// 交易成本（双边，如 0.0015 = 0.15%）
    pub commission_rate: f64,
    /// 最小建仓金额
    pub min_position_value: f64,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            enter_threshold: 0.80,
            max_hold_days: 20,
            use_trailing_stop: true,
            trailing_stop_pct: 3.0,
            risk_per_trade: 0.02,
            commission_rate: 0.0015,
            min_position_value: 1000.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub fund_code: String,
    pub entry_date: String,
    pub entry_nav: f64,
    pub shares: f64,
    pub entry_value: f64,
    pub highest_nav: f64,
    pub take_profit_nav: f64,
    pub stop_loss_nav: f64,
    pub days_held: usize,
    pub entry_signal_strength: SignalStrength,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub fund_code: String,
    pub entry_date: String,
    pub exit_date: String,
    pub entry_nav: f64,
    pub exit_nav: f64,
    pub shares: f64,
    pub entry_value: f64,
    pub exit_value: f64,
    pub gross_pnl: f64,
    pub commission: f64,
    pub net_pnl: f64,
    pub return_pct: f64,
    pub days_held: usize,
    pub exit_reason: ExitReason,
    pub entry_strength: SignalStrength,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExitReason {
    TakeProfit,
    StopLoss,
    TrailingStop,
    TimeExit,
    SignalReversal,
}

impl ExitReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExitReason::TakeProfit => "take_profit",
            ExitReason::StopLoss => "stop_loss",
            ExitReason::TrailingStop => "trailing_stop",
            ExitReason::TimeExit => "time_exit",
            ExitReason::SignalReversal => "signal_reversal",
        }
    }
}

/// 策略决策：给定当前信号和持仓，决定操作
pub enum StrategyAction {
    /// 建仓（返回建议仓位价值）
    Enter { position_value: f64, signal: TradingSignal },
    /// 持有
    Hold,
    /// 平仓
    Exit { reason: ExitReason },
}

pub struct Strategy {
    pub config: StrategyConfig,
}

impl Strategy {
    pub fn new(config: StrategyConfig) -> Self {
        Self { config }
    }

    /// 计算建议仓位：基于风险的固定分数
    /// position_value = (total_equity * risk_per_trade) / (stop_loss_pct / 100)
    pub fn position_size(
        &self,
        total_equity: f64,
        stop_loss_pct: f64,
    ) -> f64 {
        if stop_loss_pct <= 0.0 || total_equity <= 0.0 {
            return 0.0;
        }
        let risk_amount = total_equity * self.config.risk_per_trade;
        let size = risk_amount / (stop_loss_pct / 100.0);
        // 不超过总资金的 50%（分散风险）
        size.min(total_equity * 0.5).max(0.0)
    }

    /// 评估是否应该入场
    pub fn should_enter(
        &self,
        signal: &TradingSignal,
        has_position: bool,
        total_equity: f64,
    ) -> Option<StrategyAction> {
        if has_position {
            return None;
        }
        if !signal.enter {
            return None;
        }
        let size = self.position_size(total_equity, signal.stop_loss_pct);
        if size < self.config.min_position_value {
            return None;
        }
        Some(StrategyAction::Enter {
            position_value: size,
            signal: signal.clone(),
        })
    }

    /// 评估是否应该出场
    pub fn should_exit(
        &self,
        position: &Position,
        current_nav: f64,
        current_signal: Option<&TradingSignal>,
    ) -> Option<ExitReason> {
        // 止盈
        if current_nav >= position.take_profit_nav {
            return Some(ExitReason::TakeProfit);
        }
        // 止损
        if current_nav <= position.stop_loss_nav {
            return Some(ExitReason::StopLoss);
        }
        // 追踪止损
        if self.config.use_trailing_stop {
            let drawdown_from_high =
                (position.highest_nav - current_nav) / position.highest_nav * 100.0;
            if drawdown_from_high >= self.config.trailing_stop_pct && current_nav > position.entry_nav {
                return Some(ExitReason::TrailingStop);
            }
        }
        // 超时
        if position.days_held >= self.config.max_hold_days {
            return Some(ExitReason::TimeExit);
        }
        // 信号反转：综合概率大幅下降
        if let Some(sig) = current_signal {
            let combined = sig.dip_buy_proba * 0.6 + sig.magic_rebound_proba * 0.4;
            if combined < 0.40 {
                return Some(ExitReason::SignalReversal);
            }
        }
        None
    }

    /// 更新持仓状态（最高价、持有天数）
    pub fn update_position(&self, position: &mut Position, current_nav: f64) {
        if current_nav > position.highest_nav {
            position.highest_nav = current_nav;
        }
        position.days_held += 1;
    }

    /// 从持仓和平仓价结算一笔交易
    pub fn settle_trade(
        &self,
        position: &Position,
        exit_date: &str,
        exit_nav: f64,
        reason: ExitReason,
    ) -> Trade {
        let exit_value = position.shares * exit_nav;
        let gross_pnl = exit_value - position.entry_value;
        let commission =
            (position.entry_value + exit_value) * self.config.commission_rate;
        let net_pnl = gross_pnl - commission;
        let return_pct = if position.entry_value > 0.0 {
            net_pnl / position.entry_value * 100.0
        } else {
            0.0
        };
        Trade {
            fund_code: position.fund_code.clone(),
            entry_date: position.entry_date.clone(),
            exit_date: exit_date.to_string(),
            entry_nav: position.entry_nav,
            exit_nav,
            shares: position.shares,
            entry_value: position.entry_value,
            exit_value,
            gross_pnl,
            commission,
            net_pnl,
            return_pct,
            days_held: position.days_held,
            exit_reason: reason,
            entry_strength: position.entry_signal_strength,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_size_respects_risk() {
        let s = Strategy::new(StrategyConfig::default());
        // 10万资金，2%风险=2000元，止损3% → 理论仓位 66666，但上限50%资金=50000
        let size = s.position_size(100_000.0, 3.0);
        assert!((size - 50000.0).abs() < 1.0, "得到 {size}");
        // 1万资金，2%风险=200元，止损6% → 3333.33（不受50%上限影响）
        let size2 = s.position_size(10_000.0, 6.0);
        assert!((size2 - 3333.33).abs() < 1.0, "得到 {size2}");
    }

    #[test]
    fn trailing_stop_triggers() {
        let s = Strategy::new(StrategyConfig::default());
        let mut pos = Position {
            fund_code: "000001".to_string(),
            entry_date: "2024-01-01".to_string(),
            entry_nav: 1.0,
            shares: 1000.0,
            entry_value: 1000.0,
            highest_nav: 1.10,
            take_profit_nav: 1.08,
            stop_loss_nav: 0.97,
            days_held: 5,
            entry_signal_strength: SignalStrength::Strong,
        };
        // 从最高 1.10 回落到 1.06，回撤 3.6% > 3% 追踪止损
        s.update_position(&mut pos, 1.06);
        let reason = s.should_exit(&pos, 1.06, None);
        assert_eq!(reason, Some(ExitReason::TrailingStop));
    }
}
