use std::collections::{BTreeMap, HashMap};

use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub fund_id: Uuid,
    pub operation_type: OperationType,
    pub operation_date: NaiveDate,
    pub amount: Decimal,
    pub share: Decimal,
}

#[derive(Debug, Clone)]
pub struct NavRecord {
    pub fund_id: Uuid,
    pub nav_date: NaiveDate,
    pub unit_nav: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryPoint {
    pub date: NaiveDate,
    pub value: Decimal,
    pub cost: Decimal,
}

pub fn calculate_account_history(
    operations: &[Operation],
    nav_records: &[NavRecord],
    latest_nav_by_fund: &HashMap<Uuid, Decimal>,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<HistoryPoint> {
    if operations.is_empty() {
        return Vec::new();
    }

    let mut nav_map: HashMap<Uuid, HashMap<NaiveDate, Decimal>> = HashMap::new();
    for r in nav_records {
        nav_map
            .entry(r.fund_id)
            .or_default()
            .insert(r.nav_date, r.unit_nav);
    }

    // {fund_id -> op_date -> (share, cost)}，只记录发生操作那天的“操作后”持仓快照
    let mut snapshots: HashMap<Uuid, BTreeMap<NaiveDate, (Decimal, Decimal)>> = HashMap::new();
    let mut current: HashMap<Uuid, (Decimal, Decimal)> = HashMap::new();

    for op in operations {
        let entry = current
            .entry(op.fund_id)
            .or_insert((Decimal::ZERO, Decimal::ZERO));
        match op.operation_type {
            OperationType::Buy => {
                entry.0 += op.share;
                entry.1 += op.amount;
            }
            OperationType::Sell => {
                if entry.0 > Decimal::ZERO {
                    let cost_per_share = entry.1 / entry.0;
                    entry.0 -= op.share;
                    entry.1 -= op.share * cost_per_share;
                    entry.1 = rescale(entry.1, 2);
                } else {
                    entry.0 -= op.share;
                }
            }
        }

        snapshots
            .entry(op.fund_id)
            .or_default()
            .insert(op.operation_date, *entry);
    }

    let mut out = Vec::new();
    let mut d = start_date;
    while d <= end_date {
        let mut total_value = Decimal::ZERO;
        let mut total_cost = Decimal::ZERO;

        for (fund_id, fund_snaps) in &snapshots {
            let Some((share, cost)) = fund_snaps.range(..=d).next_back().map(|(_, v)| *v) else {
                continue;
            };

            if share == Decimal::ZERO {
                continue;
            }

            total_cost += cost;

            let nav = nav_map
                .get(fund_id)
                .and_then(|m| m.get(&d))
                .copied()
                .or_else(|| latest_nav_by_fund.get(fund_id).copied());

            if let Some(nav) = nav {
                total_value += share * nav;
            } else if share > Decimal::ZERO {
                // 无净值时用持仓净值估算：holding_nav = cost / share
                total_value += share * (cost / share);
            }
        }

        out.push(HistoryPoint {
            date: d,
            value: total_value,
            cost: total_cost,
        });
        let Some(next) = d.succ_opt() else {
            break;
        };
        d = next;
    }

    out
}

fn rescale(value: Decimal, dp: u32) -> Decimal {
    let mut v = value.round_dp_with_strategy(dp, RoundingStrategy::MidpointNearestEven);
    v.rescale(dp);
    v
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    use super::{HistoryPoint, NavRecord, Operation, OperationType, calculate_account_history};

    fn dec(s: &str) -> Decimal {
        s.parse::<Decimal>().unwrap()
    }

    #[test]
    fn empty_operations_returns_empty() {
        let start = chrono::NaiveDate::from_ymd_opt(2026, 2, 7).unwrap();
        let end = chrono::NaiveDate::from_ymd_opt(2026, 2, 17).unwrap();
        let out = calculate_account_history(&[], &[], &Default::default(), start, end);
        assert!(out.is_empty());
    }

    #[test]
    fn buy_op_yields_days_plus_today_points_and_date_range() {
        let fund_id = Uuid::new_v4();
        let end = chrono::NaiveDate::from_ymd_opt(2026, 2, 17).unwrap();
        let start = end - Duration::days(10);
        let op_date = end - Duration::days(5);

        let ops = vec![Operation {
            fund_id,
            operation_type: OperationType::Buy,
            operation_date: op_date,
            amount: dec("1000.00"),
            share: dec("1000.0000"),
        }];

        let mut nav = Vec::new();
        for i in 0..10 {
            let nav_date = end - Duration::days(9 - i);
            nav.push(NavRecord {
                fund_id,
                nav_date,
                unit_nav: dec("1.0000") + dec(&(i as i64).to_string()) * dec("0.1000"),
            });
        }

        let mut latest = std::collections::HashMap::new();
        latest.insert(fund_id, dec("1.5000"));

        let out = calculate_account_history(&ops, &nav, &latest, start, end);

        assert_eq!(out.len(), 11);
        assert_eq!(out.first().unwrap().date, start);
        assert_eq!(out.last().unwrap().date, end);

        let before_buy = out.iter().find(|p| p.date == start).unwrap();
        assert_eq!(
            before_buy,
            &HistoryPoint {
                date: start,
                value: Decimal::ZERO,
                cost: Decimal::ZERO,
            }
        );

        let at_buy = out.iter().find(|p| p.date == op_date).unwrap();
        assert!(at_buy.value > Decimal::ZERO);
        assert_eq!(at_buy.cost, dec("1000.00"));
    }

    #[test]
    fn sell_reduces_cost_by_average_cost_per_share() {
        let fund_id = Uuid::new_v4();
        let end = chrono::NaiveDate::from_ymd_opt(2026, 2, 17).unwrap();
        let start = end - Duration::days(3);
        let buy1 = start;
        let buy2 = start + Duration::days(1);
        let sell = start + Duration::days(2);

        let ops = vec![
            Operation {
                fund_id,
                operation_type: OperationType::Buy,
                operation_date: buy1,
                amount: dec("100.00"),
                share: dec("100.0000"),
            },
            Operation {
                fund_id,
                operation_type: OperationType::Buy,
                operation_date: buy2,
                amount: dec("200.00"),
                share: dec("100.0000"),
            },
            Operation {
                fund_id,
                operation_type: OperationType::Sell,
                operation_date: sell,
                amount: dec("0.00"),
                share: dec("100.0000"),
            },
        ];

        let nav = vec![NavRecord {
            fund_id,
            nav_date: end,
            unit_nav: dec("2.0000"),
        }];

        let out = calculate_account_history(&ops, &nav, &Default::default(), start, end);
        let last = out.last().unwrap();

        // 平均每份成本 = 300 / 200 = 1.5，卖出 100 份后剩余成本应为 150
        assert_eq!(last.cost, dec("150.00"));
    }

    #[test]
    fn max_end_date_does_not_panic() {
        let fund_id = Uuid::new_v4();
        let start = chrono::NaiveDate::MAX;
        let end = chrono::NaiveDate::MAX;

        let ops = vec![Operation {
            fund_id,
            operation_type: OperationType::Buy,
            operation_date: start,
            amount: dec("100.00"),
            share: dec("100.0000"),
        }];

        let out = calculate_account_history(&ops, &[], &Default::default(), start, end);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].date, start);
    }
}
