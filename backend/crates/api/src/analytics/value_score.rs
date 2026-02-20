#[derive(Debug, Clone)]
pub struct ValueScoreWeights {
    pub sharpe: f64,
    pub calmar: f64,
    pub ann_return: f64,
    pub max_drawdown: f64,
    pub ann_vol: f64,
}

impl Default for ValueScoreWeights {
    fn default() -> Self {
        Self {
            sharpe: 0.35,
            calmar: 0.25,
            ann_return: 0.20,
            max_drawdown: 0.10,
            ann_vol: 0.10,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SampleMetrics {
    pub fund_code: String,
    pub ann_return: Option<f64>,
    pub ann_vol: Option<f64>,
    pub max_drawdown: Option<f64>,
    pub sharpe: Option<f64>,
    pub calmar: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ValueScoreComponent {
    pub name: &'static str,
    pub percentile_0_100: f64,
    pub weight: f64,
    pub weighted: f64,
}

#[derive(Debug, Clone)]
pub struct ValueScoreResult {
    pub score_0_100: f64,
    pub percentile_0_100: f64,
    pub components: Vec<ValueScoreComponent>,
    pub sample_size: usize,
}

pub fn compute_value_score(
    samples: &[SampleMetrics],
    target_fund_code: &str,
    weights: &ValueScoreWeights,
) -> Option<ValueScoreResult> {
    let idx = samples
        .iter()
        .position(|s| s.fund_code == target_fund_code)?;
    let target = &samples[idx];

    let mut comps: Vec<ValueScoreComponent> = Vec::new();

    let mut total_weight = 0.0_f64;
    let mut total_weighted = 0.0_f64;

    // 越大越好：sharpe / calmar / ann_return
    if let Some(v) = target.sharpe {
        let p = percentile_high_better(samples.iter().filter_map(|s| s.sharpe), v);
        push_comp(
            &mut comps,
            "sharpe",
            p,
            weights.sharpe,
            &mut total_weight,
            &mut total_weighted,
        );
    }
    if let Some(v) = target.calmar {
        let p = percentile_high_better(samples.iter().filter_map(|s| s.calmar), v);
        push_comp(
            &mut comps,
            "calmar",
            p,
            weights.calmar,
            &mut total_weight,
            &mut total_weighted,
        );
    }
    if let Some(v) = target.ann_return {
        let p = percentile_high_better(samples.iter().filter_map(|s| s.ann_return), v);
        push_comp(
            &mut comps,
            "ann_return",
            p,
            weights.ann_return,
            &mut total_weight,
            &mut total_weighted,
        );
    }

    // 越小越好：max_drawdown / ann_vol（用反向 percentile）
    if let Some(v) = target.max_drawdown {
        let p = percentile_low_better(samples.iter().filter_map(|s| s.max_drawdown), v);
        push_comp(
            &mut comps,
            "max_drawdown",
            p,
            weights.max_drawdown,
            &mut total_weight,
            &mut total_weighted,
        );
    }
    if let Some(v) = target.ann_vol {
        let p = percentile_low_better(samples.iter().filter_map(|s| s.ann_vol), v);
        push_comp(
            &mut comps,
            "ann_vol",
            p,
            weights.ann_vol,
            &mut total_weight,
            &mut total_weighted,
        );
    }

    if total_weight <= 0.0 {
        return None;
    }

    let score = (total_weighted / total_weight).clamp(0.0, 100.0);

    // “总分分位”：在同类内把每个样本也算一遍分数（只用可用字段），做 rank percentile
    let mut peer_scores: Vec<f64> = Vec::with_capacity(samples.len());
    for s in samples {
        if let Some(ps) = compute_value_score_one(samples, s, weights) {
            peer_scores.push(ps);
        }
    }
    let percentile = if peer_scores.is_empty() {
        score
    } else {
        percentile_high_better(peer_scores.iter().copied(), score)
    };

    Some(ValueScoreResult {
        score_0_100: score,
        percentile_0_100: percentile,
        components: comps,
        sample_size: samples.len(),
    })
}

fn compute_value_score_one(
    samples: &[SampleMetrics],
    target: &SampleMetrics,
    weights: &ValueScoreWeights,
) -> Option<f64> {
    let mut total_weight = 0.0_f64;
    let mut total_weighted = 0.0_f64;

    if let Some(v) = target.sharpe {
        let p = percentile_high_better(samples.iter().filter_map(|s| s.sharpe), v);
        total_weight += weights.sharpe;
        total_weighted += weights.sharpe * p;
    }
    if let Some(v) = target.calmar {
        let p = percentile_high_better(samples.iter().filter_map(|s| s.calmar), v);
        total_weight += weights.calmar;
        total_weighted += weights.calmar * p;
    }
    if let Some(v) = target.ann_return {
        let p = percentile_high_better(samples.iter().filter_map(|s| s.ann_return), v);
        total_weight += weights.ann_return;
        total_weighted += weights.ann_return * p;
    }
    if let Some(v) = target.max_drawdown {
        let p = percentile_low_better(samples.iter().filter_map(|s| s.max_drawdown), v);
        total_weight += weights.max_drawdown;
        total_weighted += weights.max_drawdown * p;
    }
    if let Some(v) = target.ann_vol {
        let p = percentile_low_better(samples.iter().filter_map(|s| s.ann_vol), v);
        total_weight += weights.ann_vol;
        total_weighted += weights.ann_vol * p;
    }

    if total_weight <= 0.0 {
        None
    } else {
        Some((total_weighted / total_weight).clamp(0.0, 100.0))
    }
}

fn push_comp(
    out: &mut Vec<ValueScoreComponent>,
    name: &'static str,
    percentile_0_100: f64,
    weight: f64,
    total_weight: &mut f64,
    total_weighted: &mut f64,
) {
    if weight <= 0.0 {
        return;
    }
    let p = percentile_0_100.clamp(0.0, 100.0);
    let w = weight;
    *total_weight += w;
    *total_weighted += w * p;
    out.push(ValueScoreComponent {
        name,
        percentile_0_100: p,
        weight: w,
        weighted: w * p,
    });
}

fn percentile_high_better<I: Iterator<Item = f64>>(values: I, target: f64) -> f64 {
    let mut v: Vec<f64> = values.filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return 50.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len() as f64;
    let mut rank = 0usize;
    for (i, x) in v.iter().enumerate() {
        if *x <= target {
            rank = i;
        } else {
            break;
        }
    }
    (rank as f64) / (n - 1.0).max(1.0) * 100.0
}

fn percentile_low_better<I: Iterator<Item = f64>>(values: I, target: f64) -> f64 {
    100.0 - percentile_high_better(values, target)
}
