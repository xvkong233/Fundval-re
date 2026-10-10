//! 回测共享工具：随机抽样基金（代替固定基金池）。
//!
//! 用法：
//! ```ignore
//! mod common;
//! let funds = common::load_random_funds(200, 42, 800);
//! ```

use api::quant::lookthrough::LookThroughFactors;

/// 简单 xorshift RNG（测试用，避免引入 rand 依赖）
pub struct TestRng(pub u64);

impl TestRng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    /// Fisher-Yates 洗牌
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

pub const DATA_DIR: &str = "/home/hatch/workspace/fund_data";

/// 加载单只基金的净值数据
pub fn load_nav(code: &str) -> Option<Vec<(String, f64)>> {
    let path = format!("{DATA_DIR}/nav_{code}.json");
    let text = std::fs::read_to_string(&path).ok()?;
    let items: Vec<serde_json::Value> = serde_json::from_str(&text).ok()?;
    let mut navs: Vec<(String, f64)> = Vec::new();
    for item in items {
        let date = item.get("date").and_then(|d| d.as_str()).unwrap_or("").to_string();
        let nav: f64 = item
            .get("nav")
            .and_then(|n| n.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        if !date.is_empty() && nav > 0.0 {
            navs.push((date, nav));
        }
    }
    if navs.is_empty() {
        return None;
    }
    Some(navs)
}

/// 加载单只基金的透视因子
pub fn load_lookthrough(code: &str) -> [f64; 3] {
    let hpath = format!("{DATA_DIR}/holdings_{code}.json");
    let htext = std::fs::read_to_string(&hpath).unwrap_or_default();
    let hitems: Vec<serde_json::Value> = serde_json::from_str(&htext).unwrap_or_default();
    let weights: Vec<f64> = hitems
        .iter()
        .filter_map(|h| h.get("weight_pct").and_then(|w| w.as_f64()))
        .collect();
    if weights.is_empty() {
        [0.0, 0.0, 0.0]
    } else {
        let f = LookThroughFactors::from_weights(&weights);
        let v = f.as_features();
        [v[0], v[1], v[2]]
    }
}

/// 列出所有有净值数据的基金代码
pub fn list_available_funds() -> Vec<String> {
    let mut codes = Vec::new();
    let entries = std::fs::read_dir(DATA_DIR).unwrap();
    for entry in entries.flatten() {
        let name = entry.file_name().into_string().unwrap_or_default();
        if name.starts_with("nav_") && name.ends_with(".json") {
            codes.push(name[4..name.len() - 5].to_string());
        }
    }
    codes.sort();
    codes
}

/// 随机抽样 N 只基金（可复现：相同 seed 得到相同样本）。
///
/// - `n`: 抽样数量（超过可用数量时返回全部）
/// - `seed`: 随机种子
/// - `min_days`: 最少净值天数要求
/// - 返回：(代码, 净值序列, 透视因子)
pub fn load_random_funds(
    n: usize,
    seed: u64,
    min_days: usize,
) -> Vec<(String, Vec<(String, f64)>, [f64; 3])> {
    let mut codes = list_available_funds();
    let mut rng = TestRng(seed);
    rng.shuffle(&mut codes);

    let mut funds = Vec::new();
    for code in codes {
        if funds.len() >= n {
            break;
        }
        let navs = match load_nav(&code) {
            Some(v) if v.len() >= min_days => v,
            _ => continue,
        };
        let lt = load_lookthrough(&code);
        funds.push((code, navs, lt));
    }
    funds
}
