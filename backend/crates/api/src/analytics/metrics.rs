#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FundMetrics {
    pub max_drawdown: f64,
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

    Some(FundMetrics { max_drawdown: mdd })
}

