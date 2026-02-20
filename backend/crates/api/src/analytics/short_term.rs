#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombinedBucket {
    High,
    Medium,
    Low,
}

impl CombinedBucket {
    pub fn as_str(&self) -> &'static str {
        match self {
            CombinedBucket::High => "high",
            CombinedBucket::Medium => "medium",
            CombinedBucket::Low => "low",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrendSignal {
    pub direction: String,
    pub strength_0_1: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MeanReversionSignal {
    pub bucket: CombinedBucket,
    pub score_0_1: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CombinedSignal {
    pub bucket: CombinedBucket,
    pub action_hint: String,
    pub rationale: String,
}

#[derive(Debug, Clone)]
pub struct ShortTermSignals {
    pub trend: TrendSignal,
    pub mean_reversion: MeanReversionSignal,
    pub combined: CombinedSignal,
}

pub fn compute_short_term_signals(navs: &[f64]) -> Option<ShortTermSignals> {
    if navs.len() < 10 {
        return None;
    }

    let last = *navs.last()?;

    let ret20 = simple_return(navs, 20).unwrap_or(0.0);
    let ret60 = simple_return(navs, 60).unwrap_or(ret20);

    let direction = if ret60 > 0.0 {
        "up"
    } else if ret60 < 0.0 {
        "down"
    } else {
        "flat"
    }
    .to_string();

    // 趋势强度：用 60T 收益绝对值粗略刻画（0..1）
    let strength_0_1 = (ret60.abs() * 8.0).clamp(0.0, 1.0);
    let trend = TrendSignal {
        direction: direction.clone(),
        strength_0_1,
        reasons: vec![format!("ret20={:.4}", ret20), format!("ret60={:.4}", ret60)],
    };

    // 回归：用窗口内从峰值回撤深度刻画（越深越“偏低”）
    let peak = navs
        .iter()
        .copied()
        .fold(f64::MIN, |acc, x| if x > acc { x } else { acc });
    let dd = if peak > 0.0 { last / peak - 1.0 } else { 0.0 };
    let dd_mag = (-dd).max(0.0);

    let bucket = if dd_mag >= 0.12 {
        CombinedBucket::Low
    } else if dd_mag <= 0.03 {
        CombinedBucket::High
    } else {
        CombinedBucket::Medium
    };
    let score_0_1 = ((dd_mag - 0.03) / (0.12 - 0.03)).clamp(0.0, 1.0);
    let mean_reversion = MeanReversionSignal {
        bucket,
        score_0_1,
        reasons: vec![format!("drawdown={:.4}", dd)],
    };

    // 合成：趋势优先
    let combined = if strength_0_1 >= 0.7 {
        let (action_hint, rationale) = if direction == "up" {
            (
                "趋势优先：趋势向上，逢回撤分批加仓，破位减仓".to_string(),
                "trend strong up".to_string(),
            )
        } else if direction == "down" {
            (
                "趋势优先：趋势偏弱，控制仓位，等待企稳再考虑分批".to_string(),
                "trend strong down".to_string(),
            )
        } else {
            (
                "趋势优先：方向不明，保持观望".to_string(),
                "trend strong flat".to_string(),
            )
        };
        CombinedSignal {
            bucket: CombinedBucket::Medium,
            action_hint,
            rationale,
        }
    } else {
        match bucket {
            CombinedBucket::Low => CombinedSignal {
                bucket,
                action_hint: "回归优先：偏低区间，可分批布局，设置止损/止盈".to_string(),
                rationale: "mean-reversion drawdown deep".to_string(),
            },
            CombinedBucket::High => CombinedSignal {
                bucket,
                action_hint: "回归优先：偏高区间，谨慎追高，可考虑止盈/减仓".to_string(),
                rationale: "mean-reversion near peak".to_string(),
            },
            CombinedBucket::Medium => CombinedSignal {
                bucket,
                action_hint: "回归优先：中性区间，耐心等待更优价格或确认趋势".to_string(),
                rationale: "mean-reversion mid".to_string(),
            },
        }
    };

    Some(ShortTermSignals {
        trend,
        mean_reversion,
        combined,
    })
}

fn simple_return(navs: &[f64], lookback: usize) -> Option<f64> {
    if navs.len() <= lookback {
        return None;
    }
    let last = *navs.last()?;
    let base = navs.get(navs.len() - 1 - lookback).copied()?;
    if base <= 0.0 {
        return None;
    }
    Some(last / base - 1.0)
}
