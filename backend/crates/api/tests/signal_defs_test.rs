use api::ml::signals::{PositionBucket, bucket_for_percentile};

#[test]
fn position_bucket_uses_20_60_20_thresholds() {
    assert_eq!(bucket_for_percentile(0.0), PositionBucket::Low);
    assert_eq!(bucket_for_percentile(20.0), PositionBucket::Low);
    assert_eq!(bucket_for_percentile(20.0001), PositionBucket::Medium);
    assert_eq!(bucket_for_percentile(80.0), PositionBucket::Medium);
    assert_eq!(bucket_for_percentile(80.0001), PositionBucket::High);
    assert_eq!(bucket_for_percentile(100.0), PositionBucket::High);
}
