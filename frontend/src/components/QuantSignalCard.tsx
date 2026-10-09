"use client";

import { Alert, Card, Descriptions, Spin, Tag, Typography } from "antd";
import { useEffect, useState } from "react";
import {
  getCurrentSignal,
  type TradingSignal,
  STRENGTH_LABEL,
} from "../lib/quantTrading";

const { Text } = Typography;

const STRENGTH_COLOR: Record<string, string> = {
  none: "default",
  weak: "orange",
  medium: "blue",
  strong: "green",
};

export function QuantSignalCard({ fundCode }: { fundCode: string }) {
  const [loading, setLoading] = useState(true);
  const [signal, setSignal] = useState<TradingSignal | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    getCurrentSignal(fundCode)
      .then((res) => {
        if (!cancelled) {
          setSignal(res.data as TradingSignal);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(
            e?.response?.data?.error || "加载交易信号失败（需先运行 ML 训练）"
          );
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [fundCode]);

  return (
    <Card title="量化交易信号" size="small" style={{ marginBottom: 16 }}>
      <Spin spinning={loading}>
        {error ? (
          <Text type="secondary">{error}</Text>
        ) : signal ? (
          <>
            <div style={{ marginBottom: 12 }}>
              <Tag color={STRENGTH_COLOR[signal.strength] || "default"}>
                {STRENGTH_LABEL[signal.strength] || signal.strength}
              </Tag>
              {signal.enter ? (
                <Tag color="green">建议入场</Tag>
              ) : (
                <Tag>观望</Tag>
              )}
              <Text type="secondary" style={{ marginLeft: 8 }}>
                {signal.date} · 净值 {signal.nav?.toFixed(4)}
              </Text>
            </div>
            <Descriptions column={2} size="small">
              <Descriptions.Item label="交易盈利概率">
                <Text strong>
                  {(signal.dip_buy_proba * 100).toFixed(1)}%
                </Text>
              </Descriptions.Item>
              <Descriptions.Item label="反转概率">
                {(signal.magic_rebound_proba * 100).toFixed(1)}%
              </Descriptions.Item>
              <Descriptions.Item label="止盈目标">
                +{signal.take_profit_pct.toFixed(1)}%
              </Descriptions.Item>
              <Descriptions.Item label="止损线">
                -{signal.stop_loss_pct.toFixed(1)}%
              </Descriptions.Item>
            </Descriptions>
            {signal.enter && (
              <Alert
                message="高置信度入场信号"
                description={`模型预测该笔交易盈利概率为 ${(signal.dip_buy_proba * 100).toFixed(1)}%，超过 85% 阈值。建议按止盈 +${signal.take_profit_pct.toFixed(1)}% / 止损 -${signal.stop_loss_pct.toFixed(1)}% 执行。`}
                type="success"
                showIcon
                style={{ marginTop: 12 }}
              />
            )}
          </>
        ) : (
          <Text type="secondary">暂无信号</Text>
        )}
      </Spin>
    </Card>
  );
}
