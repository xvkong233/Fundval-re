#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CeResult {
    /// 年化确定性等价（超额收益口径，单位：比例，比如 0.05 表示 5%）
    pub ce: f64,
    pub ann_excess: f64,
    pub ann_var: f64,
    pub gamma: f64,
}

pub fn compute_ce_from_navs(navs: &[f64], rf_annual_percent: f64, gamma: f64) -> Option<CeResult> {
    if navs.len() < 2 {
        return None;
    }
    let gamma = if gamma.is_finite() && gamma > 0.0 { gamma } else { 3.0 };

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
        return None;
    }

    let n = daily.len() as f64;
    let mean = daily.iter().sum::<f64>() / n;
    let var = daily.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n;

    let rf_ann = (rf_annual_percent / 100.0).max(0.0);
    let ann_excess = mean * 252.0 - rf_ann;
    let ann_var = var * 252.0;

    let ce = ann_excess - 0.5 * gamma * ann_var;

    Some(CeResult {
        ce,
        ann_excess,
        ann_var,
        gamma,
    })
}

