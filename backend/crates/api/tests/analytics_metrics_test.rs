#[test]
fn max_drawdown_is_computed_from_peak_to_trough() {
    let navs = vec![100.0, 110.0, 90.0, 95.0, 120.0];
    let out = api::analytics::metrics::compute_metrics_from_navs(&navs, 0.0).expect("has metrics");
    let expected = -20.0 / 110.0; // 110 -> 90
    assert!((out.max_drawdown - expected).abs() < 1e-9);
}

#[test]
fn constant_nav_yields_zero_vol_and_no_sharpe() {
    let navs = vec![100.0, 100.0, 100.0];
    let out = api::analytics::metrics::compute_metrics_from_navs(&navs, 1.5).expect("has metrics");
    assert!((out.ann_vol - 0.0).abs() < 1e-12);
    assert_eq!(out.sharpe, None);
}
