use api::sources::normalize_source_name;

#[test]
fn normalize_source_name_maps_aliases_to_canonical() {
    assert_eq!(normalize_source_name("tiantian"), Some("tiantian"));
    assert_eq!(normalize_source_name("eastmoney"), Some("tiantian"));

    assert_eq!(normalize_source_name("danjuan"), Some("danjuan"));

    assert_eq!(normalize_source_name("ths"), Some("ths"));
    assert_eq!(normalize_source_name("tonghuashun"), Some("ths"));

    assert_eq!(normalize_source_name("unknown-source"), None);
}

