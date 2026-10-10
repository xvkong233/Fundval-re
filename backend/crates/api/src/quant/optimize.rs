//! 参数优化框架（参考 Abu 的网格搜索 + VNPy 的遗传算法/穷举思想）。
//!
//! 提供：
//! - 网格搜索：在参数空间中穷举，支持自定义评分函数
//! - 评分函数：综合收益、回撤、夏普、交易次数的复合评分
//! - Walk-forward 验证：防止过拟合的参数选择

use std::collections::HashMap;

/// 参数值
#[derive(Debug, Clone)]
pub enum ParamValue {
    Int(i64),
    Float(f64),
}

impl ParamValue {
    pub fn as_f64(&self) -> f64 {
        match self {
            ParamValue::Int(i) => *i as f64,
            ParamValue::Float(f) => *f,
        }
    }
}

/// 参数空间：一组候选值
#[derive(Debug, Clone)]
pub struct ParamSpace {
    pub name: String,
    pub values: Vec<ParamValue>,
}

impl ParamSpace {
    pub fn ints(name: &str, values: Vec<i64>) -> Self {
        Self {
            name: name.to_string(),
            values: values.into_iter().map(ParamValue::Int).collect(),
        }
    }

    pub fn floats(name: &str, values: Vec<f64>) -> Self {
        Self {
            name: name.to_string(),
            values: values.into_iter().map(ParamValue::Float).collect(),
        }
    }
}

/// 回测结果摘要（供评分）
#[derive(Debug, Clone, Default)]
pub struct BacktestSummary {
    /// 总收益率
    pub total_return: f64,
    /// 年化收益率
    pub annual_return: f64,
    /// 最大回撤
    pub max_drawdown: f64,
    /// 夏普率
    pub sharpe: f64,
    /// 交易次数
    pub trades: usize,
    /// 胜率
    pub win_rate: f64,
}

/// 优化结果
#[derive(Debug, Clone)]
pub struct OptimizeResult {
    pub params: HashMap<String, ParamValue>,
    pub score: f64,
    pub summary: BacktestSummary,
}

/// 默认复合评分：收益 - 回撤惩罚 + 夏普奖励 - 低交易惩罚。
///
/// 借鉴 Abu 的"自定义评分机制"思想：不只看收益，兼顾风险与稳健性。
pub fn default_score(s: &BacktestSummary) -> f64 {
    if s.trades < 3 {
        return f64::NEG_INFINITY; // 交易太少不可信
    }
    // 年化收益（主项）
    let ret_score = s.annual_return * 100.0;
    // 回撤惩罚：回撤每 1% 扣 2 分
    let dd_penalty = s.max_drawdown * 100.0 * 2.0;
    // 夏普奖励
    let sharpe_bonus = s.sharpe * 10.0;
    // 胜率修正（避免纯赌方向）
    let wr_adj = (s.win_rate - 0.5) * 20.0;
    ret_score - dd_penalty + sharpe_bonus + wr_adj
}

/// 网格搜索。
///
/// - `spaces`: 参数空间
/// - `evaluate`: 给定参数 -> 回测摘要
/// - `score_fn`: 评分函数
/// - 返回按分数排序的全部结果（最好在前）
pub fn grid_search<F, S>(
    spaces: &[ParamSpace],
    evaluate: F,
    score_fn: S,
) -> Vec<OptimizeResult>
where
    F: Fn(&HashMap<String, ParamValue>) -> BacktestSummary,
    S: Fn(&BacktestSummary) -> f64,
{
    let combos = cartesian(spaces);
    let mut results: Vec<OptimizeResult> = combos
        .into_iter()
        .map(|params| {
            let summary = evaluate(&params);
            let score = score_fn(&summary);
            OptimizeResult { params, score, summary }
        })
        .collect();
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

/// 笛卡尔积
fn cartesian(spaces: &[ParamSpace]) -> Vec<HashMap<String, ParamValue>> {
    let mut out: Vec<HashMap<String, ParamValue>> = vec![HashMap::new()];
    for space in spaces {
        let mut next = Vec::new();
        for combo in &out {
            for v in &space.values {
                let mut c = combo.clone();
                c.insert(space.name.clone(), v.clone());
                next.push(c);
            }
        }
        out = next;
    }
    out
}

/// Walk-forward 参数稳健性检查：将数据分成 N 段，检查最优参数在各段是否稳定。
///
/// 返回每段的最优参数 key（用于判断参数是否漂移）。
pub fn walk_forward_stability<F, S>(
    spaces: &[ParamSpace],
    segments: &[BacktestSummary],
    evaluate_on: F,
    score_fn: S,
) -> Vec<HashMap<String, ParamValue>>
where
    F: Fn(&HashMap<String, ParamValue>, usize) -> BacktestSummary,
    S: Fn(&BacktestSummary) -> f64,
{
    let _ = segments;
    (0..segments.len())
        .map(|seg| {
            let results = grid_search(spaces, |p| evaluate_on(p, seg), &score_fn);
            results.into_iter().next().map(|r| r.params).unwrap_or_default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_search_finds_best() {
        let spaces = vec![
            ParamSpace::ints("a", vec![1, 2, 3]),
            ParamSpace::floats("b", vec![0.1, 0.2]),
        ];
        let results = grid_search(
            &spaces,
            |p| BacktestSummary {
                annual_return: p["a"].as_f64() * 0.1 + p["b"].as_f64(),
                max_drawdown: 0.05,
                sharpe: 1.0,
                trades: 10,
                win_rate: 0.6,
                total_return: 0.0,
            },
            default_score,
        );
        assert_eq!(results.len(), 6);
        // a=3,b=0.2 应该最好
        assert_eq!(results[0].params["a"].as_f64(), 3.0);
    }

    #[test]
    fn default_score_penalizes_dd() {
        let good = BacktestSummary {
            annual_return: 0.15,
            max_drawdown: 0.05,
            sharpe: 1.2,
            trades: 20,
            win_rate: 0.6,
            total_return: 0.5,
        };
        let bad = BacktestSummary {
            max_drawdown: 0.40,
            ..good.clone()
        };
        assert!(default_score(&good) > default_score(&bad));
    }
}
