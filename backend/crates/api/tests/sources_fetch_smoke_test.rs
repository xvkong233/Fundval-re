use api::sources;

#[test]
fn danjuan_parses_nav_history_items() {
    let sample = r#"{"data":{"items":[{"date":"2026-02-13","nav":"0.7037","percentage":"0.03","value":"0.7037"},{"date":"2026-02-12","nav":"0.7035","percentage":"-1.58","value":"0.7035"}],"current_page":1,"size":2,"total_items":2613,"total_pages":523},"result_code":0}"#;

    let rows = sources::danjuan::parse_nav_history_json(sample).expect("parse");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].nav_date.to_string(), "2026-02-13");
    assert_eq!(rows[0].unit_nav.to_string(), "0.7037");
    assert_eq!(rows[0].daily_growth.map(|v| v.to_string()), Some("0.03".to_string()));
}

#[test]
fn ths_parses_js_var_array() {
    let sample = r#"var dwjz_000001=[["2026-02-13","1.2345"],["2026-02-12","1.2000"]];"#;

    let rows = sources::ths::parse_nav_series_js(sample).expect("parse");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].nav_date.to_string(), "2026-02-13");
    assert_eq!(rows[0].unit_nav.to_string(), "1.2345");
}

#[test]
fn danjuan_builds_nav_history_url() {
    let url = sources::danjuan::nav_history_url("161725", 1, 5);
    assert!(url.contains("danjuanapp.com"));
    assert!(url.contains("/djapi/fund/nav/history/161725"));
    assert!(url.contains("page=1"));
    assert!(url.contains("size=5"));
}

#[test]
fn ths_builds_dwjz_url() {
    let url = sources::ths::dwjz_url("000001");
    assert_eq!(url, "https://fund.10jqka.com.cn/000001/json/jsondwjz.json");
}

#[test]
fn ths_selects_latest_nav_by_max_date() {
    let sample = r#"var dwjz_000001=[["2026-02-12","1.2000"],["2026-02-13","1.2345"]];"#;
    let rows = sources::ths::parse_nav_series_js(sample).expect("parse");
    let latest = sources::ths::latest_nav(&rows).expect("latest");
    assert_eq!(latest.nav_date.to_string(), "2026-02-13");
    assert_eq!(latest.nav.to_string(), "1.2345");
}
