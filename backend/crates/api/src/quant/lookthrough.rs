//! 股票透视因子：穿透基金持仓，看底层股票的表现。
//!
//! 移植上游 FundVal-Live 的穿透估算思想，但用于因子工程而非实时估值：
//! - 静态因子：持仓集中度（HHI）、第一大重仓占比、有效持仓数
//! - 动态因子：持仓加权日涨跌（盘中/盘后估算今日净值变动）
//!
//! 这些因子作为 ML 模型的额外特征，帮助判断基金的真实强弱。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookThroughFactors {
    /// 持仓集中度 HHI = Σ(weight²)，越高越集中
    pub hhi: f64,
    /// 第一大重仓股占比（%）
    pub top_weight_pct: f64,
    /// 有效持仓数 = 1/HHI_normalized，越小越集中
    pub effective_n: f64,
    /// 持仓加权日涨跌幅（%），基于成分股实时/日行情
    pub weighted_daily_change_pct: f64,
    /// 有行情的持仓权重覆盖率（0-1）
    pub coverage_ratio: f64,
}

impl LookThroughFactors {
    /// 从持仓权重计算静态因子（无需行情）
    /// `weights`: 持仓占比（%，如 6.45 表示 6.45%）
    pub fn from_weights(weights: &[f64]) -> Self {
        if weights.is_empty() {
            return Self {
                hhi: 0.0,
                top_weight_pct: 0.0,
                effective_n: 0.0,
                weighted_daily_change_pct: 0.0,
                coverage_ratio: 0.0,
            };
        }
        // 归一化为小数
        let total: f64 = weights.iter().sum();
        let norm: Vec<f64> = if total > 1e-9 {
            weights.iter().map(|w| w / total).collect()
        } else {
            vec![0.0; weights.len()]
        };
        let hhi: f64 = norm.iter().map(|w| w * w).sum();
        let top_weight_pct = weights.iter().cloned().fold(0.0_f64, f64::max);
        let effective_n = if hhi > 1e-9 { 1.0 / hhi } else { 0.0 };

        Self {
            hhi,
            top_weight_pct,
            effective_n,
            weighted_daily_change_pct: 0.0,
            coverage_ratio: 0.0,
        }
    }

    /// 加入成分股日涨跌幅，计算加权变动（穿透估算）
    /// `changes`: 与 weights 对应的成分股日涨跌幅（%，None 表示无行情）
    pub fn with_quotes(mut self, weights: &[f64], changes: &[Option<f64>]) -> Self {
        let total_w: f64 = weights.iter().sum();
        if total_w <= 1e-9 {
            return self;
        }
        let mut weighted_sum = 0.0_f64;
        let mut covered_w = 0.0_f64;
        for (w, c) in weights.iter().zip(changes.iter()) {
            if let Some(chg) = c {
                weighted_sum += w * chg;
                covered_w += w;
            }
        }
        self.weighted_daily_change_pct = weighted_sum / total_w;
        self.coverage_ratio = covered_w / total_w;
        self
    }

    /// 转为 ML 特征向量（3 维静态因子）
    pub fn as_features(&self) -> Vec<f64> {
        vec![
            self.hhi.clamp(0.0, 1.0),
            (self.top_weight_pct / 100.0).clamp(0.0, 1.0),
            (self.effective_n / 10.0).clamp(0.0, 1.0), // 归一化
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hhi_concentration() {
        // 单一持仓 100% → HHI=1
        let f = LookThroughFactors::from_weights(&[100.0]);
        assert!((f.hhi - 1.0).abs() < 1e-9);
        assert!((f.effective_n - 1.0).abs() < 1e-9);

        // 10只等权 → HHI=0.1, effective_n=10
        let f2 = LookThroughFactors::from_weights(&[10.0; 10]);
        assert!((f2.hhi - 0.1).abs() < 1e-9);
        assert!((f2.effective_n - 10.0).abs() < 1e-9);
    }

    #[test]
    fn weighted_change() {
        let f = LookThroughFactors::from_weights(&[60.0, 40.0])
            .with_quotes(&[60.0, 40.0], &[Some(2.0), Some(-1.0)]);
        // 0.6*2 + 0.4*(-1) = 0.8
        assert!((f.weighted_daily_change_pct - 0.8).abs() < 1e-9);
        assert!((f.coverage_ratio - 1.0).abs() < 1e-9);
    }
}
