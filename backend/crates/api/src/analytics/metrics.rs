#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FundMetrics {
    pub max_drawdown: f64,
    pub ann_vol: f64,
    pub sharpe: Option<f64>,
}

pub fn compute_metrics_from_navs(navs: &[f64], _rf_annual_percent: f64) -> Option<FundMetrics> {
    if navs.len() < 2 {
        return None;
    }

    let mut peak = navs[0];
    let mut mdd = 0.0_f64;
    for &v in navs {
        if v > peak {
            peak = v;
        }
        if peak > 0.0 {
            let dd = v / peak - 1.0;
            if dd < mdd {
                mdd = dd;
            }
        }
    }

    let mut daily: Vec<f64> = Vec::with_capacity(navs.len().saturating_sub(1));
    for i in 1..navs.len() {
        let prev = navs[i - 1];
        let cur = navs[i];
        if prev <= 0.0 {
            continue;
        }
        daily.push(cur / prev - 1.0);
    }

    if daily.is_empty() {
        return Some(FundMetrics {
            max_drawdown: mdd,
            ann_vol: 0.0,
            sharpe: None,
        });
    }

    let n = daily.len() as f64;
    let mean = daily.iter().sum::<f64>() / n;
    let var = daily.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n;
    let std = var.sqrt();
    let ann_vol = std * 252.0_f64.sqrt();

    let rf_daily = (_rf_annual_percent / 100.0) / 252.0;
    let excess_mean = daily.iter().map(|r| r - rf_daily).sum::<f64>() / n;
    let sharpe = if std > 0.0 {
        Some((excess_mean / std) * 252.0_f64.sqrt())
    } else {
        None
    };

    Some(FundMetrics {
        max_drawdown: mdd,
        ann_vol,
        sharpe,
    })
}
