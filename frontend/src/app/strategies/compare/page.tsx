"use client";

import { Button, Card, DatePicker, Form, Grid, Input, InputNumber, Select, Space, Statistic, Table, Tabs, Typography, message } from "antd";
import React, { useMemo, useState } from "react";
import dayjs from "dayjs";

import { AuthedLayout } from "../../../components/AuthedLayout";
import { compareFundStrategies, getIndexDaily, listNavHistory } from "../../../lib/api";

const { RangePicker } = DatePicker;
const { Text } = Typography;

type CompareRow = {
  date: string;
  total_amount: number;
  left_amount: number;
  fund_amount: number;
  position: number;
  accumulated_profit: number;
  total_profit_rate: number;
};

function mapStrategyRows(result: any, name: string): CompareRow[] {
  const series: any[] = result?.strategies?.[name]?.series ?? [];
  return series.map((p) => ({
    date: String(p.date),
    total_amount: Number(p.total_amount ?? 0),
    left_amount: Number(p.left_amount ?? 0),
    fund_amount: Number(p.fund_amount ?? 0),
    position: Number(p.position ?? 0),
    accumulated_profit: Number(p.accumulated_profit ?? 0),
    total_profit_rate: Number(p.total_profit_rate ?? 0),
  }));
}

export default function StrategiesComparePage() {
  const screens = Grid.useBreakpoint();
  const isMobile = !screens.md;

  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<any>(null);

  const rowsA = useMemo(() => mapStrategyRows(result, "策略A"), [result]);
  const rowsB = useMemo(() => mapStrategyRows(result, "策略B"), [result]);
  const lastA = rowsA.length ? rowsA[rowsA.length - 1] : null;
  const lastB = rowsB.length ? rowsB[rowsB.length - 1] : null;

  const tableColumns = useMemo(
    () => [
      { title: "日期", dataIndex: "date", width: 110 },
      { title: "总资产", dataIndex: "total_amount", responsive: ["sm"] as any },
      { title: "现金", dataIndex: "left_amount", responsive: ["md"] as any },
      { title: "持仓市值", dataIndex: "fund_amount", responsive: ["md"] as any },
      {
        title: "仓位",
        dataIndex: "position",
        width: 90,
        render: (v: any) => `${(Number(v) * 100).toFixed(1)}%`,
      },
      { title: "累计收益", dataIndex: "accumulated_profit", responsive: ["sm"] as any },
      {
        title: "累计收益率",
        dataIndex: "total_profit_rate",
        width: 120,
        render: (v: any) => `${(Number(v) * 100).toFixed(2)}%`,
      },
    ],
    []
  );

  return (
    <AuthedLayout
      title="策略对比（Qbot/fund-strategies）"
      subtitle="基于 fund-strategies TS 策略（服务端计算）"
    >
      <Card
        title="对比参数"
        extra={<Text type="secondary">参考指数用于 MACD 择时；上证指数用于“高位止盈”阈值判断</Text>}
      >
        <Form
          form={form}
          layout="vertical"
          initialValues={{
            fund_code: "000001",
            source: "tiantian",
            range: [dayjs().add(-365, "day"), dayjs()],
            total_amount: 10000,
            salary: 10000,
            fixed_amount: 1000,
            refer_index_code: "1.000300",
            a_buy_macd_point: 50,
            a_sell_macd_point: 75,
            b_buy_macd_point: null,
            b_sell_macd_point: null,
          }}
          onFinish={async (v) => {
            setLoading(true);
            try {
              const fundCode = String(v.fund_code || "").trim();
              const source = String(v.source || "").trim();
              const [start, end] = v.range ?? [];
              const startDate = dayjs(start).format("YYYY-MM-DD");
              const endDate = dayjs(end).format("YYYY-MM-DD");

              const nav = await listNavHistory(fundCode, { start_date: startDate, end_date: endDate, source });
              const items: any[] = Array.isArray(nav?.data?.items) ? nav.data.items : [];
              const fundSeries = items
                .map((it) => ({
                  date: String(it.nav_date),
                  val: Number(it.unit_nav),
                }))
                .reverse();

              const referIndexCode = String(v.refer_index_code || "").trim();
              const [szResp, riResp] = await Promise.all([
                getIndexDaily({ index_code: "1.000001", start_date: startDate, end_date: endDate }),
                getIndexDaily({ index_code: referIndexCode, start_date: startDate, end_date: endDate }),
              ]);
              const szPoints: any[] = Array.isArray(szResp?.data?.points) ? szResp.data.points : [];
              const riPoints: any[] = Array.isArray(riResp?.data?.points) ? riResp.data.points : [];
              const shangzhengSeries = szPoints.map((p) => ({ date: String(p.date), val: Number(p.close) }));
              const referIndexSeries = riPoints.map((p) => ({ date: String(p.date), val: Number(p.close) }));

              const payload = {
                fund_series: fundSeries,
                shangzheng_series: shangzhengSeries,
                refer_index_points: [],
                refer_index_series: referIndexSeries,
                strategies: [
                  {
                    name: "策略A",
                    cfg: {
                      total_amount: Number(v.total_amount ?? 10000),
                      salary: Number(v.salary ?? 10000),
                      fixed_amount: Number(v.fixed_amount ?? 1000),
                      period: ["monthly", 1],
                      buy_macd_point: v.a_buy_macd_point === null ? null : Number(v.a_buy_macd_point),
                      sell_macd_point: v.a_sell_macd_point === null ? null : Number(v.a_sell_macd_point),
                    },
                  },
                  {
                    name: "策略B",
                    cfg: {
                      total_amount: Number(v.total_amount ?? 10000),
                      salary: 0,
                      fixed_amount: Number(v.fixed_amount ?? 1000),
                      period: ["monthly", 1],
                      buy_macd_point: v.b_buy_macd_point === null ? null : Number(v.b_buy_macd_point),
                      sell_macd_point: v.b_sell_macd_point === null ? null : Number(v.b_sell_macd_point),
                    },
                  },
                ],
              };

              const resp = await compareFundStrategies(payload);
              setResult(resp?.data ?? null);
              message.success("对比完成");
            } catch (e: any) {
              message.error(e?.response?.data?.error || e?.message || "对比失败");
            } finally {
              setLoading(false);
            }
          }}
        >
          <div className="fv-toolbar">
            <div className="fv-toolbarScroll">
              <Space wrap={false}>
                <Button
                  type="primary"
                  htmlType="submit"
                  loading={loading}
                >
                  开始对比
                </Button>
                <Button onClick={() => form.resetFields()} disabled={loading}>
                  重置
                </Button>
              </Space>
            </div>
          </div>

          <div className="fv-grid" style={{ display: "grid", gridTemplateColumns: isMobile ? "1fr" : "repeat(4, minmax(0, 1fr))", gap: 12 }}>
            <Form.Item name="fund_code" label="基金代码" rules={[{ required: true }]}>
              <Input placeholder="例如 000001" />
            </Form.Item>
            <Form.Item name="source" label="数据源" rules={[{ required: true }]}>
              <Select
                options={[
                  { label: "天天基金 (tiantian)", value: "tiantian" },
                  { label: "东方财富 (eastmoney)", value: "eastmoney" },
                ]}
              />
            </Form.Item>
            <Form.Item name="refer_index_code" label="参考指数（MACD）" rules={[{ required: true }]}>
              <Select
                options={[
                  { value: "1.000001", label: "上证综指 (1.000001)" },
                  { value: "1.000300", label: "沪深300 (1.000300)" },
                  { value: "1.000905", label: "中证500 (1.000905)" },
                ]}
              />
            </Form.Item>
            <Form.Item name="range" label="时间范围" rules={[{ required: true }]}>
              <RangePicker style={{ width: "100%" }} />
            </Form.Item>
          </div>

          <div className="fv-grid" style={{ display: "grid", gridTemplateColumns: isMobile ? "1fr" : "repeat(4, minmax(0, 1fr))", gap: 12, marginTop: 8 }}>
            <Form.Item name="total_amount" label="初始资金">
              <InputNumber min={0} style={{ width: "100%" }} />
            </Form.Item>
            <Form.Item name="salary" label="每月工资（1号入金）">
              <InputNumber min={0} style={{ width: "100%" }} />
            </Form.Item>
            <Form.Item name="fixed_amount" label="每月定投（1号）">
              <InputNumber min={0} style={{ width: "100%" }} />
            </Form.Item>
            <div />
            <Form.Item name="a_buy_macd_point" label="策略A 买点分位(0-100)">
              <InputNumber min={0} max={100} style={{ width: "100%" }} />
            </Form.Item>
            <Form.Item name="a_sell_macd_point" label="策略A 卖点分位(0-100)">
              <InputNumber min={0} max={100} style={{ width: "100%" }} />
            </Form.Item>
            <Form.Item name="b_buy_macd_point" label="策略B 买点分位(0-100)">
              <InputNumber min={0} max={100} style={{ width: "100%" }} />
            </Form.Item>
            <Form.Item name="b_sell_macd_point" label="策略B 卖点分位(0-100)">
              <InputNumber min={0} max={100} style={{ width: "100%" }} />
            </Form.Item>
          </div>
        </Form>
      </Card>

      <Card style={{ marginTop: 16 }} title="结果预览" loading={loading}>
        <Tabs
          size={isMobile ? "small" : "middle"}
          items={[
            {
              key: "a",
              label: "策略A",
              children: (
                <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                  <div className="fv-kpiGrid4">
                    <Statistic title="期末总资产" value={lastA ? lastA.total_amount : "-"} />
                    <Statistic title="累计收益" value={lastA ? lastA.accumulated_profit : "-"} />
                    <Statistic title="累计收益率" value={lastA ? `${(lastA.total_profit_rate * 100).toFixed(2)}%` : "-"} />
                    <Statistic title="期末仓位" value={lastA ? `${(lastA.position * 100).toFixed(1)}%` : "-"} />
                  </div>
                  <Table
                    rowKey="date"
                    size={isMobile ? "small" : "middle"}
                    dataSource={rowsA}
                    pagination={{ pageSize: isMobile ? 10 : 12, simple: isMobile, showLessItems: isMobile }}
                    columns={tableColumns as any}
                    scroll={isMobile ? undefined : { x: "max-content" }}
                    locale={{ emptyText: "暂无结果（请先开始对比）" }}
                  />
                </div>
              ),
            },
            {
              key: "b",
              label: "策略B",
              children: (
                <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                  <div className="fv-kpiGrid4">
                    <Statistic title="期末总资产" value={lastB ? lastB.total_amount : "-"} />
                    <Statistic title="累计收益" value={lastB ? lastB.accumulated_profit : "-"} />
                    <Statistic title="累计收益率" value={lastB ? `${(lastB.total_profit_rate * 100).toFixed(2)}%` : "-"} />
                    <Statistic title="期末仓位" value={lastB ? `${(lastB.position * 100).toFixed(1)}%` : "-"} />
                  </div>
                  <Table
                    rowKey="date"
                    size={isMobile ? "small" : "middle"}
                    dataSource={rowsB}
                    pagination={{ pageSize: isMobile ? 10 : 12, simple: isMobile, showLessItems: isMobile }}
                    columns={tableColumns as any}
                    scroll={isMobile ? undefined : { x: "max-content" }}
                    locale={{ emptyText: "暂无结果（请先开始对比）" }}
                  />
                </div>
              ),
            },
          ]}
        />
      </Card>
    </AuthedLayout>
  );
}
