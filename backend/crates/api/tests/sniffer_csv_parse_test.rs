use std::str::FromStr;

use api::sniffer::parse_deepq_csv;
use rust_decimal::Decimal;

#[test]
fn parses_deepq_csv_with_bom_and_dynamic_year_header() {
    let csv = "\u{feff}板块,基金名称,基金代码,近1周涨幅,2025年涨幅,今年最大回撤,基金规模,机构持有,基金经理持有,内部人士持有,评分星级,特色标签,赎回手续费\n\
有色金属,天弘中证工业有色金属主题ETF联接C,017193,2.75%,112.77%,14.97%,26.8亿(+19.7亿),17%(+1%),0万份 (持平),5万份 (-1万份),★★★★★,强势中、涨得多、内部买,\n";

    let rows = parse_deepq_csv(csv).expect("parse ok");
    assert_eq!(rows.len(), 1);

    let r = &rows[0];
    assert_eq!(r.sector, "有色金属");
    assert_eq!(r.fund_code, "017193");
    assert_eq!(r.fund_name, "天弘中证工业有色金属主题ETF联接C");
    assert_eq!(r.star_count, Some(5));
    assert_eq!(r.tags, vec!["强势中", "涨得多", "内部买"]);
    assert_eq!(r.week_growth, Some(Decimal::from_str("2.75").unwrap()));
    assert_eq!(r.year_growth, Some(Decimal::from_str("112.77").unwrap()));
    assert_eq!(r.max_drawdown, Some(Decimal::from_str("14.97").unwrap()));
    assert_eq!(r.fund_size_text.as_deref(), Some("26.8亿(+19.7亿)"));
}

#[test]
fn parses_by_header_name_not_column_order() {
    let csv = "\u{feff}基金代码,基金名称,板块,2025年涨幅,近1周涨幅,评分星级,特色标签\n\
017193,天弘中证工业有色金属主题ETF联接C,有色金属,112.77%,2.75%,★★★★★,强势中、涨得多\n";

    let rows = parse_deepq_csv(csv).expect("parse ok");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].sector, "有色金属");
    assert_eq!(rows[0].fund_code, "017193");
    assert_eq!(rows[0].star_count, Some(5));
    assert_eq!(
        rows[0].week_growth,
        Some(Decimal::from_str("2.75").unwrap())
    );
    assert_eq!(
        rows[0].year_growth,
        Some(Decimal::from_str("112.77").unwrap())
    );
    assert_eq!(rows[0].tags, vec!["强势中", "涨得多"]);
}
