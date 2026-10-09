//! 每日可操作建议引擎：结合模型信号、股票透视、T+1 交易规则，
//! 输出每天 15:00 前可执行的买卖建议。
//!
//! T+1 规则（中国开放式基金）：
//! - 交易日 15:00 前提交 → 按当日净值成交；15:00 后 → 按下一交易日净值
//! - 申购/赎回 T+1 确认份额（T日15:00前提交，T+1确认）
//! - 持有期按自然日：从申购确认日 → 赎回确认日前一日
//! - 持有 <7 天赎回费 1.5%（惩罚性），策略强制最短持有 7 天
//! - 股混基金赎回资金 T+4 工作日到账

use chrono::{Datelike, Local, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};

/// T+1 交易规则常量
pub mod t1_rules {
    use chrono::NaiveDate;
    use chrono::Datelike;
    /// 每日交易截止时间（15:00）
    pub const CUTOFF_HOUR: u32 = 15;
    /// 最短持有天数（避开 1.5% 惩罚性赎回费）
    pub const MIN_HOLD_DAYS: i64 = 7;
    /// 持有不足7天的赎回费率
    pub const SHORT_HOLD_FEE: f64 = 0.015;
    /// 股混基金赎回资金到账（工作日）
    pub const REDEMPTION_SETTLE_DAYS: i64 = 4;

    /// 判断是否为交易日（简化：周一至周五；节假日需外部日历）
    pub fn is_trading_day(date: NaiveDate) -> bool {
        !matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun)
    }

    /// 获取下一个交易日
    pub fn next_trading_day(mut date: NaiveDate) -> NaiveDate {
        loop {
            date = date.succ_opt().unwrap_or(date);
            if is_trading_day(date) {
                return date;
            }
        }
    }

    /// 计算赎回费率（按持有天数，自然日）
    /// 返回 (费率, 是否触发惩罚费率)
    pub fn redemption_fee_rate(hold_days: i64) -> (f64, bool) {
        if hold_days < MIN_HOLD_DAYS {
            (SHORT_HOLD_FEE, true)
        } else if hold_days < 30 {
            (0.0075, false)
        } else if hold_days < 365 {
            (0.005, false)
        } else if hold_days < 730 {
            (0.0025, false)
        } else {
            (0.0, false)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// 建议买入（15:00前下单按今日净值）
    Buy,
    /// 建议卖出
    Sell,
    /// 持有不动
    Hold,
    /// 暂不操作（信号不足）
    Wait,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyRecommendation {
    pub fund_code: String,
    pub fund_name: String,
    pub action: Action,
    /// 模型综合概率 [0,1]
    pub confidence: f64,
    /// 信号强度描述
    pub signal_desc: String,
    /// 股票透视：持仓加权今日涨跌（%），None 表示无数据
    pub lookthrough_change_pct: Option<f64>,
    /// 操作理由
    pub reason: String,
    /// T+1 提示（如 "15:00前下单按今日净值成交"）
    pub t1_note: String,
    /// 当前持仓天数（None 表示未持有）
    pub hold_days: Option<i64>,
    /// 预估赎回费率（仅卖出时）
    pub est_redemption_fee: Option<f64>,
}

/// 生成每日建议
///
/// - `now`: 当前时间（用于判断是否在 15:00 前）
/// - `signals`: 每只基金的 (code, name, proba, lookthrough_change)
/// - `positions`: 当前持仓 (code → 确认日期)
pub fn generate_daily_recommendations(
    now: chrono::DateTime<Local>,
    signals: &[(String, String, f64, Option<f64>)],
    positions: &std::collections::HashMap<String, NaiveDate>,
    buy_threshold: f64,
    sell_threshold: f64,
) -> Vec<DailyRecommendation> {
    let today = now.date_naive();
    let hour = now.hour();
    let before_cutoff = hour < t1_rules::CUTOFF_HOUR;
    let is_trading = t1_rules::is_trading_day(today);

    let nav_note = if !is_trading {
        "今日非交易日，建议顺延至下一交易日 15:00 前操作".to_string()
    } else if before_cutoff {
        format!("15:00前下单按今日({})净值成交", today)
    } else {
        let next = t1_rules::next_trading_day(today);
        format!("已过15:00，现在下单按 {next} 净值成交")
    };

    let mut out = Vec::new();
    for (code, name, proba, lt_change) in signals {
        let hold_days = positions.get(code).map(|confirm_date| {
            (today - *confirm_date).num_days().max(0)
        });

        let (action, reason) = match hold_days {
            // 持有中：检查是否触发卖出
            Some(hd) => {
                if *proba < sell_threshold {
                    // 检查最短持有期
                    if hd < t1_rules::MIN_HOLD_DAYS {
                        let (fee, _) = t1_rules::redemption_fee_rate(hd);
                        (
                            Action::Hold,
                            format!("模型转弱但持有仅{hd}天，赎回费高达{:.1}%（惩罚费率），建议持有满7天", fee * 100.0),
                        )
                    } else {
                        (Action::Sell, format!("模型概率降至{proba:.2}，触发卖出信号"))
                    }
                } else {
                    (Action::Hold, format!("持有{hd}天，模型概率{proba:.2}维持强势"))
                }
            }
            // 未持有：检查是否触发买入
            None => {
                if *proba >= buy_threshold {
                    (Action::Buy, format!("模型概率{proba:.2}达买入阈值"))
                } else {
                    (Action::Wait, format!("模型概率{proba:.2}未达买入阈值{buy_threshold:.2}"))
                }
            }
        };

        // 透视因子增强理由
        let mut full_reason = reason;
        if let Some(lt) = lt_change {
            full_reason.push_str(&format!("；持仓透视今日{:+.2}%", lt));
        }

        // 卖出时计算预估赎回费
        let est_fee = match (&action, hold_days) {
            (Action::Sell, Some(hd)) => {
                let (fee, _) = t1_rules::redemption_fee_rate(hd);
                Some(fee)
            }
            _ => None,
        };

        let signal_desc = if *proba >= 0.70 {
            "强信号"
        } else if *proba >= buy_threshold {
            "中等信号"
        } else if *proba >= 0.50 {
            "弱信号"
        } else {
            "无信号"
        }
        .to_string();

        out.push(DailyRecommendation {
            fund_code: code.clone(),
            fund_name: name.clone(),
            action,
            confidence: *proba,
            signal_desc,
            lookthrough_change_pct: *lt_change,
            reason: full_reason,
            t1_note: nav_note.clone(),
            hold_days,
            est_redemption_fee: est_fee,
        });
    }

    // 按 action 优先级排序：Buy > Sell > Hold > Wait
    out.sort_by_key(|r| match r.action {
        Action::Buy => 0,
        Action::Sell => 1,
        Action::Hold => 2,
        Action::Wait => 3,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn t1_redemption_fee_tiers() {
        assert_eq!(t1_rules::redemption_fee_rate(3).0, 0.015);
        assert_eq!(t1_rules::redemption_fee_rate(10).0, 0.0075);
        assert_eq!(t1_rules::redemption_fee_rate(100).0, 0.005);
        assert_eq!(t1_rules::redemption_fee_rate(800).0, 0.0);
    }

    #[test]
    fn min_hold_protection() {
        // 持有3天但模型转弱 → 应 Hold 而非 Sell（避开1.5%惩罚费）
        let now = Local::now();
        let today = now.date_naive();
        let mut positions = HashMap::new();
        positions.insert("000001".to_string(), today - chrono::Duration::days(3));

        let signals = vec![("000001".to_string(), "测试基金".to_string(), 0.40, None)];
        let recs = generate_daily_recommendations(now, &signals, &positions, 0.60, 0.50);
        assert_eq!(recs[0].action, Action::Hold);
        assert!(recs[0].reason.contains("1.5%"));
    }

    #[test]
    fn buy_signal_no_position() {
        let now = Local::now();
        let positions = HashMap::new();
        let signals = vec![("000001".to_string(), "测试基金".to_string(), 0.65, Some(1.2))];
        let recs = generate_daily_recommendations(now, &signals, &positions, 0.60, 0.50);
        assert_eq!(recs[0].action, Action::Buy);
        assert!(recs[0].reason.contains("透视"));
    }
}
