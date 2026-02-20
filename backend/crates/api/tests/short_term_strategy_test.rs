use api::analytics::short_term::{CombinedBucket, compute_short_term_signals};

#[test]
fn trend_priority_overrides_mean_reversion_when_trend_strong() {
    // 60T 强趋势上行：combined 应走趋势优先，并且 bucket 固定为 Medium（避免误判高/低位）
    let mut navs: Vec<f64> = Vec::new();
    for i in 0..100 {
        navs.push(1.0 + (i as f64) * 0.002);
    }

    let s = compute_short_term_signals(&navs).expect("signals");
    assert_eq!(s.combined.bucket, CombinedBucket::Medium);
    assert!(s.combined.action_hint.contains("趋势"));
}

#[test]
fn mean_reversion_applies_when_trend_weak_and_drawdown_deep() {
    // 走势横盘后出现明显回撤：combined 应输出偏低（可分批布局）
    let navs = vec![
        1.0, 1.01, 1.00, 1.02, 1.01, 0.95, 0.94, 0.92, 0.91, 0.90, 0.89, 0.895,
    ];

    let s = compute_short_term_signals(&navs).expect("signals");
    assert_eq!(s.combined.bucket, CombinedBucket::Low);
    assert!(s.combined.action_hint.contains("分批"));
}
