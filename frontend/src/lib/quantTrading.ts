"use client";

import api from "./http";

export type TradeView = {
  entry_date: string;
  exit_date: string;
  entry_nav: number;
  exit_nav: number;
  return_pct: number;
  days_held: number;
  exit_reason: string;
  profitable: boolean;
};

export type BacktestResult = {
  fund_code: string;
  win_rate: number;
  total_trades: number;
  winning_trades: number;
  total_return_pct: number;
  profit_factor: number;
  max_drawdown_pct: number;
  sharpe_ratio: number;
  trades: TradeView[];
};

export type TradingSignal = {
  fund_code: string;
  date: string;
  nav: number;
  dip_buy_proba: number;
  magic_rebound_proba: number;
  strength: "none" | "weak" | "medium" | "strong";
  enter: boolean;
  take_profit_pct: number;
  stop_loss_pct: number;
};

export const runBacktest = (params: {
  fund_code: string;
  source?: string;
  initial_capital?: number;
  enter_threshold?: number;
}) => api.post("/quant-trading/backtest", params);

export const getCurrentSignal = (fundCode: string, source?: string) =>
  api.get(`/quant-trading/signals/${encodeURIComponent(fundCode)}`, {
    params: source ? { source } : {},
  });

export const EXIT_REASON_LABEL: Record<string, string> = {
  take_profit: "止盈",
  stop_loss: "止损",
  trailing_stop: "追踪止损",
  time_exit: "超时退出",
  signal_reversal: "信号反转",
};

export const STRENGTH_LABEL: Record<string, string> = {
  none: "无信号",
  weak: "弱信号",
  medium: "中等信号",
  strong: "强信号",
};
