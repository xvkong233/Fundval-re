use api::analytics::value_score::{compute_value_score, SampleMetrics, ValueScoreWeights};

#[test]
fn value_score_ranks_better_sharpe_and_lower_risk_higher() {
    let weights = ValueScoreWeights::default();

    // 同类三只基金：A 明显更好，C 更差
    let samples = vec![
        SampleMetrics {
            fund_code: "A".to_string(),
            ann_return: Some(0.20),
            ann_vol: Some(0.15),
            max_drawdown: Some(0.10),
            sharpe: Some(1.20),
            calmar: Some(1.80),
        },
        SampleMetrics {
            fund_code: "B".to_string(),
            ann_return: Some(0.12),
            ann_vol: Some(0.18),
            max_drawdown: Some(0.18),
            sharpe: Some(0.70),
            calmar: Some(0.80),
        },
        SampleMetrics {
            fund_code: "C".to_string(),
            ann_return: Some(0.05),
            ann_vol: Some(0.25),
            max_drawdown: Some(0.30),
            sharpe: Some(0.10),
            calmar: Some(0.15),
        },
    ];

    let a = compute_value_score(&samples, "A", &weights).expect("score A");
    let b = compute_value_score(&samples, "B", &weights).expect("score B");
    let c = compute_value_score(&samples, "C", &weights).expect("score C");

    assert!(a.score_0_100 > b.score_0_100);
    assert!(b.score_0_100 > c.score_0_100);

    // percentile 越大越好
    assert!(a.percentile_0_100 > b.percentile_0_100);
    assert!(b.percentile_0_100 > c.percentile_0_100);
}

