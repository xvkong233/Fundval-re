pub const SOURCE_TIANTIAN: &str = "tiantian";
pub const SOURCE_DANJUAN: &str = "danjuan";
pub const SOURCE_THS: &str = "ths";
pub const SOURCE_TUSHARE: &str = "tushare";

pub const BUILTIN_SOURCES: [&str; 4] =
    [SOURCE_TIANTIAN, SOURCE_DANJUAN, SOURCE_THS, SOURCE_TUSHARE];

pub mod danjuan;
pub mod ths;
pub mod tushare;

pub fn normalize_source_name(input: &str) -> Option<&'static str> {
    let s = input.trim().to_ascii_lowercase();
    if s.is_empty() {
        return None;
    }

    match s.as_str() {
        SOURCE_TIANTIAN | "eastmoney" => Some(SOURCE_TIANTIAN),
        SOURCE_DANJUAN => Some(SOURCE_DANJUAN),
        SOURCE_THS | "tonghuashun" | "10jqka" => Some(SOURCE_THS),
        SOURCE_TUSHARE => Some(SOURCE_TUSHARE),
        _ => None,
    }
}
