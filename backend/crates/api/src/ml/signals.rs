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
