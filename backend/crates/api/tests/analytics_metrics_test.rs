#[test]
fn max_drawdown_is_computed_from_peak_to_trough() {
    let navs = vec![100.0, 110.0, 90.0, 95.0, 120.0];
    let out = api::analytics::metrics::compute_metrics_from_navs(&navs, 0.0).expect("has metrics");
    let expected = -20.0 / 110.0; // 110 -> 90
    assert!((out.max_drawdown - expected).abs() < 1e-9);
}

