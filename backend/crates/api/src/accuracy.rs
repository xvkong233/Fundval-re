use rust_decimal::Decimal;

pub fn compute_error_rate(estimate_nav: Decimal, actual_nav: Decimal) -> Option<Decimal> {
    if actual_nav <= Decimal::ZERO {
        return None;
    }
    let diff = (estimate_nav - actual_nav).abs();
    Some(diff / actual_nav)
}

