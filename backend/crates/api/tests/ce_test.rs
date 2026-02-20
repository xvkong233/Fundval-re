use api::analytics::ce::compute_ce_from_navs;

#[test]
fn ce_decreases_when_gamma_increases() {
    // 构造带波动的 NAV 序列
    let navs = vec![1.0, 1.02, 0.99, 1.03, 1.01, 1.04, 1.00];
    let rf_annual_percent = 2.0;

    let ce_low_gamma = compute_ce_from_navs(&navs, rf_annual_percent, 1.0).expect("ce");
    let ce_high_gamma = compute_ce_from_navs(&navs, rf_annual_percent, 8.0).expect("ce");

    assert!(ce_high_gamma.ce < ce_low_gamma.ce);
}

#[test]
fn ce_increases_when_returns_improve() {
    let rf_annual_percent = 2.0;
    let ce_bad =
        compute_ce_from_navs(&[1.0, 1.00, 0.99, 1.00, 0.98], rf_annual_percent, 3.0).unwrap();
    let ce_good =
        compute_ce_from_navs(&[1.0, 1.01, 1.02, 1.03, 1.04], rf_annual_percent, 3.0).unwrap();
    assert!(ce_good.ce > ce_bad.ce);
}
