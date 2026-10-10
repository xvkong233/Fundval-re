//! 新策略大比武：Donchian突破 vs ADX趋势启动 vs 布林带 vs 买入持有。
//!
//! 学习来源：
//! - Donchian：Trader项目"大哥2.2"海龟交易法
//! - ADX：Qbot的adx_strategy.py（三均线+ADX+MACD）
//! - 布林带：Qbot的boll_strategy_bt.py

use api::quant::adx_trend::{backtest_adx, AdxParams};
use api::quant::boll::{backtest_boll, BollParams};
use api::quant::donchian::{backtest_donchian, DonchianParams};
use api::quant::trend::{backtest_buy_hold, backtest_trend, TrendParams};

mod common;

#[test]
fn new_strategies_showdown() {
    let funds = common::load_random_funds(150, 99, 800);
    println!("\n随机抽样 {} 只基金（seed=99）", funds.len());

    struct Agg {
        ret: f64,
        dd: f64,
        trades: usize,
        n: usize,
    }
    let mut agg_bh = Agg { ret: 0.0, dd: 0.0, trades: 0, n: 0 };
    let mut agg_trend = Agg { ret: 0.0, dd: 0.0, trades: 0, n: 0 };
    let mut agg_donchian = Agg { ret: 0.0, dd: 0.0, trades: 0, n: 0 };
    let mut agg_adx = Agg { ret: 0.0, dd: 0.0, trades: 0, n: 0 };
    let mut agg_boll = Agg { ret: 0.0, dd: 0.0, trades: 0, n: 0 };

    let trend_p = TrendParams::default();
    let donchian_p = DonchianParams::default();
    let adx_p = AdxParams::default();
    let boll_p = BollParams::default();

    for (_, navs, _) in &funds {
        let n = navs.len();
        let start = n * 70 / 100;
        if n - start < 100 {
            continue;
        }

        let (r, dd) = backtest_buy_hold(navs, start);
        agg_bh.ret += r;
        agg_bh.dd += dd;
        agg_bh.n += 1;

        let (r, t, dd) = backtest_trend(navs, &trend_p, start);
        agg_trend.ret += r;
        agg_trend.dd += dd;
        agg_trend.trades += t;
        agg_trend.n += 1;

        let (r, t, dd) = backtest_donchian(navs, &donchian_p, start);
        agg_donchian.ret += r;
        agg_donchian.dd += dd;
        agg_donchian.trades += t;
        agg_donchian.n += 1;

        let (r, t, dd) = backtest_adx(navs, &adx_p, start);
        agg_adx.ret += r;
        agg_adx.dd += dd;
        agg_adx.trades += t;
        agg_adx.n += 1;

        let (r, t, dd) = backtest_boll(navs, &boll_p, start);
        agg_boll.ret += r;
        agg_boll.dd += dd;
        agg_boll.trades += t;
        agg_boll.n += 1;
    }

    println!("\n=== 新策略大比武（{}只基金，后30%，含费用） ===", agg_bh.n);
    println!("{:<18} {:>10} {:>10} {:>10}", "策略", "平均收益", "平均回撤", "平均交易");
    for (name, a) in [
        ("买入持有(基准)", &agg_bh),
        ("MA趋势跟踪", &agg_trend),
        ("Donchian突破", &agg_donchian),
        ("ADX趋势启动", &agg_adx),
        ("布林带均值回归", &agg_boll),
    ] {
        println!(
            "{:<18} {:>9.1}% {:>9.1}% {:>10.1}",
            name,
            a.ret / a.n as f64 * 100.0,
            a.dd / a.n as f64 * 100.0,
            a.trades as f64 / a.n as f64,
        );
    }
}
