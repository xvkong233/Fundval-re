#[test]
fn parse_chinabond_curve_json_extracts_3m_rate_and_date() {
    let json = r#"{
      "ycDefName": "中债国债收益率曲线",
      "worktime": "2026-02-14",
      "seriesData": [[0.25, 1.3428], [1.0, 1.3145]]
    }"#;

    let got = api::rates::treasury_3m::parse_chinabond_curve_json(json).expect("parse ok");
    assert_eq!(got.rate_date, "2026-02-14");
    assert!((got.rate_percent - 1.3428).abs() < 1e-9);
}

