"use client";

import {
  Button,
  Card,
  Col,
  Descriptions,
  Form,
  Input,
  InputNumber,
  Row,
  Space,
  Statistic,
  Table,
  Tag,
  Typography,
  message,
} from "antd";
import type { TableColumnsType } from "antd";
import { useState } from "react";
import { AuthedLayout } from "../../components/AuthedLayout";
import {
  runBacktest,
  type BacktestResult,
  type TradeView,
  EXIT_REASON_LABEL,
} from "../../lib/quantTrading";

const { Title, Text, Paragraph } = Typography;

const tradeColumns: TableColumnsType<TradeView> = [
  {
    title: "入场日期",
    dataIndex: "entry_date",
    key: "entry_date",
    width: 110,
  },
  {
    title: "出场日期",
    dataIndex: "exit_date",
    key: "exit_date",
    width: 110,
  },
  {
    title: "入场净值",
    dataIndex: "entry_nav",
    key: "entry_nav",
    render: (v: number) => v.toFixed(4),
    width: 100,
  },
  {
    title: "出场净值",
    dataIndex: "exit_nav",
    key: "exit_nav",
    render: (v: number) => v.toFixed(4),
    width: 100,
  },
  {
    title: "收益率",
    dataIndex: "return_pct",
    key: "return_pct",
    render: (v: number) => (
      <Text type={v > 0 ? "success" : "danger"}>{v.toFixed(2)}%</Text>
    ),
    sorter: (a, b) => a.return_pct - b.return_pct,
    width: 100,
  },
  {
    title: "持有天数",
    dataIndex: "days_held",
    key: "days_held",
    width: 90,
  },
  {
    title: "出场原因",
    dataIndex: "exit_reason",
    key: "exit_reason",
    render: (v: string) => EXIT_REASON_LABEL[v] || v,
    width: 100,
  },
  {
    title: "结果",
    key: "profitable",
    render: (_, r) =>
      r.profitable ? (
        <Tag color="green">盈利</Tag>
      ) : (
        <Tag color="red">亏损</Tag>
      ),
    width: 80,
  },
];

export default function QuantTradingPage() {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<BacktestResult | null>(null);

  const onRun = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);
      setResult(null);
      const res = await runBacktest({
        fund_code: values.fund_code.trim(),
        source: values.source?.trim() || undefined,
        initial_capital: values.initial_capital || 100000,
        enter_threshold: values.enter_threshold ?? 0.55,
      });
      setResult(res.data as BacktestResult);
      message.success("回测完成");
    } catch (e: any) {
      if (e?.response?.data?.error) {
        message.error(e.response.data.error);
      } else if (e?.errorFields) {
        // 表单验证失败，不提示
      } else {
        message.error("回测失败");
      }
    } finally {
      setLoading(false);
    }
  };

  const winRate = result ? result.win_rate * 100 : 0;
  const winRateColor = winRate >= 80 ? "green" : winRate >= 60 ? "orange" : "red";

  return (
    <AuthedLayout>
      <Title level={3}>量化交易回测</Title>
      <Paragraph type="secondary">
        基于 ML 预测的模拟交易：模型预测每笔交易的盈利概率，仅在高置信度（默认
        85%）时入场，止盈 6% / 止损 3% / 追踪止损，目标胜率 80%+。
      </Paragraph>

      <Card title="回测参数" style={{ marginBottom: 16 }}>
        <Form
          form={form}
          layout="inline"
          initialValues={{
            initial_capital: 100000,
            enter_threshold: 0.55,
            source: "tiantian",
          }}
        >
          <Form.Item
            label="基金代码"
            name="fund_code"
            rules={[{ required: true, message: "请输入基金代码" }]}
          >
            <Input placeholder="如 000001" style={{ width: 140 }} />
          </Form.Item>
          <Form.Item label="数据源" name="source">
            <Input placeholder="tiantian" style={{ width: 120 }} />
          </Form.Item>
          <Form.Item label="初始资金" name="initial_capital">
            <InputNumber min={1000} step={10000} style={{ width: 140 }} />
          </Form.Item>
          <Form.Item
            label="入场阈值"
            name="enter_threshold"
            tooltip="模型预测盈利概率超过此值才入场，越高越保守"
          >
            <InputNumber min={0.5} max={0.95} step={0.05} style={{ width: 120 }} />
          </Form.Item>
          <Form.Item>
            <Button type="primary" onClick={onRun} loading={loading}>
              运行回测
            </Button>
          </Form.Item>
        </Form>
      </Card>

      {result && (
        <>
          <Row gutter={16} style={{ marginBottom: 16 }}>
            <Col span={6}>
              <Card>
                <Statistic
                  title="胜率"
                  value={winRate}
                  precision={1}
                  suffix="%"
                  valueStyle={{ color: `var(--ant-color-${winRateColor})` }}
                />
                <Text type="secondary">
                  {result.winning_trades}/{result.total_trades} 笔盈利
                </Text>
              </Card>
            </Col>
            <Col span={6}>
              <Card>
                <Statistic
                  title="总收益率"
                  value={result.total_return_pct}
                  precision={2}
                  suffix="%"
                  valueStyle={{
                    color:
                      result.total_return_pct >= 0
                        ? "var(--ant-color-green)"
                        : "var(--ant-color-red)",
                  }}
                />
              </Card>
            </Col>
            <Col span={6}>
              <Card>
                <Statistic
                  title="盈亏比"
                  value={result.profit_factor}
                  precision={2}
                />
                <Text type="secondary">
                  夏普 {result.sharpe_ratio.toFixed(2)}
                </Text>
              </Card>
            </Col>
            <Col span={6}>
              <Card>
                <Statistic
                  title="最大回撤"
                  value={result.max_drawdown_pct}
                  precision={2}
                  suffix="%"
                  valueStyle={{ color: "var(--ant-color-orange)" }}
                />
              </Card>
            </Col>
          </Row>

          <Card
            title={`交易记录（${result.trades.length} 笔）`}
            style={{ marginBottom: 16 }}
          >
            {result.trades.length === 0 ? (
              <Text type="secondary">
                无交易。模型在该基金上未找到高置信度入场机会，说明当前信号不足——不交易也是一种风控。
              </Text>
            ) : (
              <Table
                columns={tradeColumns}
                dataSource={result.trades.map((t, i) => ({ ...t, key: i }))}
                pagination={{ pageSize: 20 }}
                size="small"
                scroll={{ x: 900 }}
              />
            )}
          </Card>

          <Card title="策略说明">
            <Descriptions column={1} size="small">
              <Descriptions.Item label="入场">
                模型预测该笔交易盈利概率 ≥{" "}
                {(form.getFieldValue("enter_threshold") ?? 0.55) * 100}%{" "}
                时入场；低于此阈值不交易
              </Descriptions.Item>
              <Descriptions.Item label="出场">
                止盈 +6% / 止损 -3% / 从最高点回落 3% 追踪止损 / 最长持有 20 天 /
                信号反转
              </Descriptions.Item>
              <Descriptions.Item label="仓位">
                单笔风险 2% 资金，最大仓位 50% 资金
              </Descriptions.Item>
              <Descriptions.Item label="成本">
                双边佣金 0.15%
              </Descriptions.Item>
            </Descriptions>
          </Card>
        </>
      )}
    </AuthedLayout>
  );
}
