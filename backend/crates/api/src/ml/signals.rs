#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionBucket {
    Low,
    Medium,
    High,
}

impl PositionBucket {
    pub fn as_str(&self) -> &'static str {
        match self {
            PositionBucket::Low => "low",
            PositionBucket::Medium => "medium",
            PositionBucket::High => "high",
        }
    }
}

pub fn bucket_for_percentile(percentile_0_100: f64) -> PositionBucket {
    let p = percentile_0_100.clamp(0.0, 100.0);
    if p <= 20.0 {
        PositionBucket::Low
    } else if p <= 80.0 {
        PositionBucket::Medium
    } else {
        PositionBucket::High
    }
}

pub const MAGIC_REBOUND_THRESHOLD_5T: f64 = 0.03;
pub const MAGIC_REBOUND_THRESHOLD_20T: f64 = 0.08;

/// 抄底成功阈值：未来 h 天涨幅需超过该值才算"抄底成功"。
///
/// 旧标签 `(ret > 0.0)` 在 h=20 时正样本率高达 98%，模型退化为恒预测正，
/// 信号毫无区分度。改用覆盖交易成本且有实际意义的涨幅门槛。
pub const DIP_BUY_THRESHOLD_5T: f64 = 0.01;
pub const DIP_BUY_THRESHOLD_20T: f64 = 0.05;
