use rust_decimal::Decimal;

#[test]
fn compute_error_rate_matches_django_formula() {
    // error_rate = abs(estimate - actual) / actual
    let estimate = Decimal::from_str_exact("1.10").unwrap();
    let actual = Decimal::from_str_exact("1.00").unwrap();
    let rate = api::accuracy::compute_error_rate(estimate, actual).unwrap();
    assert_eq!(rate.round_dp(6), Decimal::from_str_exact("0.1").unwrap());
}

#[test]
fn compute_error_rate_returns_none_when_actual_non_positive() {
    let estimate = Decimal::from_str_exact("1.10").unwrap();
    let actual = Decimal::from_str_exact("0").unwrap();
    assert!(api::accuracy::compute_error_rate(estimate, actual).is_none());
}
