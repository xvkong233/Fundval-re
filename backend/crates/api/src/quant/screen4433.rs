//! 4433 法则基金筛选器（参考 Qbot/pyfunds 的 fund-strategies）。
//!
//! 4433 法则（晨星经典选基方法）：
//! - 第1个4：近1年收益率同类排名前 1/4
//! - 第2个4：近2年、3年、5年及今年以来收益率同类排名前 1/4
//! - 第1个3：近6个月收益率同类排名前 1/3
//! - 第2个3：近3个月收益率同类排名前 1/3
//!
//! 通过 4433 的基金 = 长短期业绩都稳定的"常胜基金"，
//! 再叠加 ML 择时模型，形成"选基+择时"双层策略。

use std::collections::HashMap;

/// 单只基金的各周期收益率
#[derive(Debug, Clone, Default)]
pub struct FundReturns {
    pub code: String,
    /// (周期名, 收益率)
    pub periods: HashMap<String, f64>,
}

impl FundReturns {
    /// 从净值序列计算各周期收益率
    /// navs: 按日期升序的 (date, nav)
    pub fn from_navs(code: &str, navs: &[(String, f64)]) -> Self {
        let mut periods = HashMap::new();
        let n = navs.len();
        if n < 60 {
            return Self { code: code.to_string(), periods };
        }
        // 交易日近似：1年=252天，2年=504，3年=756，5年=1260，6月=126，3月=63
        let defs = [
            ("1y", 252),
            ("2y", 504),
            ("3y", 756),
            ("5y", 1260),
            ("6m", 126),
            ("3m", 63),
            ("ytd", 0), // 今年以来（简化：近60天）
        ];
        let last_nav = navs[n - 1].1;
        for (name, days) in defs {
            let lookback = if name == "ytd" { 60.min(n - 1) } else { days.min(n - 1) };
            if lookback < 20 {
                continue;
            }
            let base_nav = navs[n - 1 - lookback].1;
            if base_nav > 0.0 {
                periods.insert(name.to_string(), last_nav / base_nav - 1.0);
            }
        }
        Self { code: code.to_string(), periods }
    }

    pub fn get(&self, period: &str) -> Option<f64> {
        self.periods.get(period).copied()
    }
}

/// 4433 筛选结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Screen4433Result {
    pub code: String,
    /// 各周期是否达标
    pub pass_1y: bool,
    pub pass_2y: bool,
    pub pass_3y: bool,
    pub pass_5y: bool,
    pub pass_ytd: bool,
    pub pass_6m: bool,
    pub pass_3m: bool,
    /// 总分（达标周期数）
    pub score: u8,
    /// 是否通过完整4433
    pub pass_all: bool,
}

/// 运行 4433 筛选
///
/// - `funds`: 所有候选基金的收益率数据
/// - 返回每只基金的筛选结果
pub fn screen_4433(funds: &[FundReturns]) -> Vec<Screen4433Result> {
    // 对每个周期计算排名阈值
    let periods_4 = ["1y", "2y", "3y", "5y", "ytd"]; // 要求前1/4
    let periods_3 = ["6m", "3m"]; // 要求前1/3

    // 收集每个周期的有效值
    let mut thresholds_4: HashMap<String, f64> = HashMap::new();
    let mut thresholds_3: HashMap<String, f64> = HashMap::new();

    for p in periods_4 {
        let mut vals: Vec<f64> = funds.iter().filter_map(|f| f.get(p)).collect();
        if vals.len() < 4 {
            continue;
        }
        vals.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let idx = vals.len() / 4; // 前1/4的分位点
        thresholds_4.insert(p.to_string(), vals[idx.min(vals.len() - 1)]);
    }
    for p in periods_3 {
        let mut vals: Vec<f64> = funds.iter().filter_map(|f| f.get(p)).collect();
        if vals.len() < 3 {
            continue;
        }
        vals.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let idx = vals.len() / 3; // 前1/3的分位点
        thresholds_3.insert(p.to_string(), vals[idx.min(vals.len() - 1)]);
    }

    funds
        .iter()
        .map(|f| {
            let check = |p: &str, map: &HashMap<String, f64>| -> bool {
                match (f.get(p), map.get(p)) {
                    (Some(v), Some(&t)) => v >= t,
                    _ => false,
                }
            };
            let pass_1y = check("1y", &thresholds_4);
            let pass_2y = check("2y", &thresholds_4);
            let pass_3y = check("3y", &thresholds_4);
            let pass_5y = check("5y", &thresholds_4);
            let pass_ytd = check("ytd", &thresholds_4);
            let pass_6m = check("6m", &thresholds_3);
            let pass_3m = check("3m", &thresholds_3);

            let score = [pass_1y, pass_2y, pass_3y, pass_5y, pass_ytd, pass_6m, pass_3m]
                .iter()
                .filter(|&&b| b)
                .count() as u8;
            // 完整4433：1y + (2y/3y/5y/ytd至少3个) + 6m + 3m
            let long_pass = [pass_2y, pass_3y, pass_5y, pass_ytd].iter().filter(|&&b| b).count();
            let pass_all = pass_1y && long_pass >= 3 && pass_6m && pass_3m;

            Screen4433Result {
                code: f.code.clone(),
                pass_1y,
                pass_2y,
                pass_3y,
                pass_5y,
                pass_ytd,
                pass_6m,
                pass_3m,
                score,
                pass_all,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_basic() {
        // 构造4只基金，A最强
        let mk = |code: &str, rets: &[(&str, f64)]| {
            let mut periods = HashMap::new();
            for (p, v) in rets {
                periods.insert(p.to_string(), *v);
            }
            FundReturns { code: code.to_string(), periods }
        };
        let funds = vec![
            mk("A", &[("1y", 0.30), ("2y", 0.50), ("3y", 0.80), ("5y", 1.20), ("ytd", 0.10), ("6m", 0.15), ("3m", 0.08)]),
            mk("B", &[("1y", 0.10), ("2y", 0.20), ("3y", 0.30), ("5y", 0.50), ("ytd", 0.05), ("6m", 0.06), ("3m", 0.03)]),
            mk("C", &[("1y", 0.05), ("2y", 0.10), ("3y", 0.15), ("5y", 0.25), ("ytd", 0.02), ("6m", 0.03), ("3m", 0.01)]),
            mk("D", &[("1y", 0.01), ("2y", 0.02), ("3y", 0.03), ("5y", 0.05), ("ytd", 0.01), ("6m", 0.01), ("3m", 0.005)]),
        ];
        let results = screen_4433(&funds);
        let a = results.iter().find(|r| r.code == "A").unwrap();
        assert!(a.pass_all, "最强基金应通过4433");
        assert_eq!(a.score, 7);
        let d = results.iter().find(|r| r.code == "D").unwrap();
        assert!(!d.pass_all);
    }
}
