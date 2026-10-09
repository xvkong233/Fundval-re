//! 交易信号生成：从 ML 模型概率到可执行的入场/出场信号。
//!
//! 核心设计：
//! - 使用校准后的概率
//! - 置信度分层：只有高置信度才入场（保证胜率）
//! - 信号包含方向、置信度、建议持有期

use serde::{Deserialize, Serialize};

use super::calibration::PlattCalibrator;
use crate::ml::logreg::LogRegModel;

/// 信号强度分层
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalStrength {
    /// P < 0.55：无信号
    None,
    /// 0.55 <= P < 0.70：弱信号（观察）
    Weak,
    /// 0.70 <= P < 0.80：中等信号（可小仓位）
    Medium,
    /// P >= 0.80：强信号（标准仓位，高胜率）
    Strong,
}

impl SignalStrength {
    pub fn from_prob(p: f64) -> Self {
        if p >= 0.80 {
            SignalStrength::Strong
        } else if p >= 0.70 {
            SignalStrength::Medium
        } else if p >= 0.55 {
            SignalStrength::Weak
        } else {
            SignalStrength::None
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SignalStrength::None => "none",
            SignalStrength::Weak => "weak",
            SignalStrength::Medium => "medium",
            SignalStrength::Strong => "strong",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSignal {
    pub date: String,
    pub fund_code: String,
    /// 校准后的抄底成功概率
    pub dip_buy_proba: f64,
    /// 校准后的反转概率
    pub magic_rebound_proba: f64,
    /// 综合信号强度
    pub strength: SignalStrength,
    /// 是否触发入场（强信号）
    pub enter: bool,
    /// 建议止盈目标（基于波动率动态）
    pub take_profit_pct: f64,
    /// 建议止损（基于波动率动态）
    pub stop_loss_pct: f64,
}

/// 信号生成器：持有模型 + 校准器，为每个时点生成信号
pub struct SignalGenerator {
    pub dip_buy_model: LogRegModel,
    pub magic_rebound_model: LogRegModel,
    pub dip_buy_calibrator: PlattCalibrator,
    pub magic_calibrator: PlattCalibrator,
    /// 入场概率阈值（默认 0.80，保证高胜率）
    pub enter_threshold: f64,
}

impl SignalGenerator {
    pub fn generate(
        &self,
        date: &str,
        fund_code: &str,
        features: &[f64],
        _vol20: f64,
    ) -> Option<TradingSignal> {
        let dip_raw = self.dip_buy_model.predict_proba(features)?;
        let magic_raw = self.magic_rebound_model.predict_proba(features)?;

        let dip_buy_proba = self.dip_buy_calibrator.calibrate(dip_raw);
        let magic_rebound_proba = self.magic_calibrator.calibrate(magic_raw);

        // 综合概率：两个模型的一致性加权
        // 当两个模型都看好时，置信度更高
        let combined = (dip_buy_proba * 0.6 + magic_rebound_proba * 0.4).clamp(0.0, 1.0);
        let strength = SignalStrength::from_prob(combined);
        let enter = combined >= self.enter_threshold;

        // 固定止盈止损（与训练标签一致，保证可预测性）
        // 动态调整会引入训练-回测不一致
        let take_profit_pct = 6.0;
        let stop_loss_pct = 3.0;

        Some(TradingSignal {
            date: date.to_string(),
            fund_code: fund_code.to_string(),
            dip_buy_proba,
            magic_rebound_proba,
            strength,
            enter,
            take_profit_pct,
            stop_loss_pct,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_strength_tiers() {
        assert_eq!(SignalStrength::from_prob(0.85), SignalStrength::Strong);
        assert_eq!(SignalStrength::from_prob(0.75), SignalStrength::Medium);
        assert_eq!(SignalStrength::from_prob(0.60), SignalStrength::Weak);
        assert_eq!(SignalStrength::from_prob(0.50), SignalStrength::None);
    }
}
