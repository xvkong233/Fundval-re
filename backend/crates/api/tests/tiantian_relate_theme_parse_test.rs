use api::tiantian_h5::parse_fund_relate_themes_from_jjxqy1_2;

#[test]
fn parse_jjxqy1_2_extracts_relate_theme_sec_names() {
    let payload = r#"
    {
      "success": true,
      "errorCode": 0,
      "data": {
        "fundRelateTheme": [
          { "SEC_CODE": "BK000156", "SEC_NAME": "国防军工", "FCODE": "018939" },
          { "SEC_CODE": "BK000158", "SEC_NAME": "航空装备", "FCODE": "018939" }
        ]
      }
    }
    "#;

    let themes = parse_fund_relate_themes_from_jjxqy1_2(payload).expect("parse themes");
    assert_eq!(themes.len(), 2);
    assert_eq!(themes[0].sec_code, "BK000156");
    assert_eq!(themes[0].sec_name, "国防军工");
}
