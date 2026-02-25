"use client";

import { ReloadOutlined } from "@ant-design/icons";
import { Button, Card, Col, Grid, Row, Space, Statistic, Table, Typography, message } from "antd";
import type { TableColumnsType } from "antd";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { AuthedLayout } from "../../components/AuthedLayout";
import { listAccounts, listPositions, listPositionOperations, listWatchlists } from "../../lib/api";

const { Text } = Typography;

type Account = Record<string, any> & { id: string; name?: string; parent: string | null; is_default?: boolean };
type Position = Record<string, any> & { id: string; fund_code: string; fund_name?: string; pnl?: string; holding_cost?: string; holding_share?: string };
type Operation = Record<string, any> & {
  id: string;
  fund: string;
  fund_name?: string;
  operation_type: "BUY" | "SELL";
  operation_date: string;
  amount?: string;
  share?: string;
  nav?: string;
  created_at: string;
};
type Watchlist = Record<string, any> & { id: string; name?: string; items?: any[] };

function toNumber(v: any): number | null {
  if (v === null || v === undefined || v === "") return null;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

function money(v: any): string {
  const n = toNumber(v);
  if (n === null) return "-";
  return n.toLocaleString("zh-CN", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function pnlColor(v: any): string | undefined {
  const n = toNumber(v);
  if (n === null) return undefined;
  return n >= 0 ? "#cf1322" : "#3f8600";
}

function pctFromRate(v: any): string {
  const n = toNumber(v);
  if (n === null) return "-";
  return `${(n * 100).toFixed(2)}%`;
}

function opTypeText(t: "BUY" | "SELL"): string {
  return t === "BUY" ? "买入" : "卖出";
}

export default function DashboardPage() {
  const screens = Grid.useBreakpoint();
  const isMobile = !screens.md;

  const [loading, setLoading] = useState(false);
  const [lastUpdateTime, setLastUpdateTime] = useState<Date | null>(null);

  const [accounts, setAccounts] = useState<Account[]>([]);
  const [positions, setPositions] = useState<Position[]>([]);
  const [operations, setOperations] = useState<Operation[]>([]);
  const [watchlists, setWatchlists] = useState<Watchlist[]>([]);

  const parentAccounts = useMemo(() => accounts.filter((a) => !a?.parent), [accounts]);

  const summary = useMemo(() => {
    const sum = (key: string) =>
      parentAccounts.reduce((acc, a) => acc + (toNumber((a as any)[key]) ?? 0), 0);

    const holding_cost = sum("holding_cost");
    const holding_value = sum("holding_value");
    const pnl = sum("pnl");
    const today_pnl = sum("today_pnl");
    const pnl_rate = holding_cost > 0 ? pnl / holding_cost : null;
    const today_pnl_rate = holding_value > 0 ? today_pnl / holding_value : null;

    return { holding_cost, holding_value, pnl, pnl_rate, today_pnl, today_pnl_rate };
  }, [parentAccounts]);

  const latestOperations = useMemo(() => {
    return [...operations]
      .sort((a, b) => {
        const d = new Date(b.operation_date).getTime() - new Date(a.operation_date).getTime();
        if (d !== 0) return d;
        return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
      })
      .slice(0, 10);
  }, [operations]);

  const topPositions = useMemo(() => {
    return [...positions]
      .sort((a, b) => (toNumber(b.pnl) ?? 0) - (toNumber(a.pnl) ?? 0))
      .slice(0, 10);
  }, [positions]);

  const loadAll = async () => {
    setLoading(true);
    try {
      const [a, p, o, w] = await Promise.allSettled([
        listAccounts(),
        listPositions(),
        listPositionOperations(),
        listWatchlists(),
      ]);

      if (a.status === "fulfilled") setAccounts(Array.isArray(a.value.data) ? (a.value.data as Account[]) : []);
      else message.error("加载账户失败");

      if (p.status === "fulfilled") setPositions(Array.isArray(p.value.data) ? (p.value.data as Position[]) : []);
      else message.error("加载持仓失败");

      if (o.status === "fulfilled") setOperations(Array.isArray(o.value.data) ? (o.value.data as Operation[]) : []);
      else message.error("加载操作流水失败");

      if (w.status === "fulfilled") setWatchlists(Array.isArray(w.value.data) ? (w.value.data as Watchlist[]) : []);
      else message.error("加载自选列表失败");

      setLastUpdateTime(new Date());
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void loadAll();
  }, []);

  const parentColumns: TableColumnsType<Account> = [
    {
      title: "账户",
      dataIndex: "name",
      key: "name",
      render: (v: any, record) => (
        <span style={{ whiteSpace: "nowrap" }}>
          {String(v ?? record.id)}
          {record.is_default ? (
            <Text type="secondary" style={{ marginLeft: 8, fontSize: 12 }}>
              (默认)
            </Text>
          ) : null}
        </span>
      ),
    },
    { title: "持仓成本", dataIndex: "holding_cost", key: "holding_cost", width: 140, render: money, responsive: ["md"] },
    { title: "持仓市值", dataIndex: "holding_value", key: "holding_value", width: 140, render: money, responsive: ["md"] },
    {
      title: "总盈亏",
      dataIndex: "pnl",
      key: "pnl",
      width: 120,
      render: (v: any) => <span style={{ color: pnlColor(v) }}>{money(v)}</span>,
    },
  ];

  const opColumns: TableColumnsType<Operation> = [
    { title: "日期", dataIndex: "operation_date", key: "operation_date", width: 120 },
    { title: "类型", dataIndex: "operation_type", key: "operation_type", width: 90, render: opTypeText },
    {
      title: "基金",
      key: "fund",
      render: (_: any, r) => r.fund_name ?? "-",
    },
    { title: "金额", dataIndex: "amount", key: "amount", width: 120, render: money },
    {
      title: "份额",
      dataIndex: "share",
      key: "share",
      width: 120,
      render: (v: any) => (v ? String(v) : "-"),
      responsive: ["md"],
    },
  ];

  const posColumns: TableColumnsType<Position> = [
    { title: "代码", dataIndex: "fund_code", key: "fund_code", width: 110 },
    { title: "基金名称", dataIndex: "fund_name", key: "fund_name", ellipsis: true },
    { title: "持仓成本", dataIndex: "holding_cost", key: "holding_cost", width: 140, render: money, responsive: ["lg"] },
    {
      title: "盈亏",
      dataIndex: "pnl",
      key: "pnl",
      width: 120,
      render: (v: any) => <span style={{ color: pnlColor(v) }}>{money(v)}</span>,
    },
  ];

  return (
    <AuthedLayout
      title="仪表盘"
      subtitle={lastUpdateTime ? `更新于 ${lastUpdateTime.toLocaleTimeString()}` : undefined}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <Card
          title="总览"
          extra={
            <div className="fv-toolbarScroll">
              <Button icon={<ReloadOutlined />} loading={loading} onClick={() => void loadAll()}>
                刷新
              </Button>
            </div>
          }
        >
          <div className="fv-kpiGrid">
            <Statistic title="持仓成本" value={money(summary.holding_cost)} />
            <Statistic title="持仓市值" value={money(summary.holding_value)} />
            <Statistic title="总盈亏" value={money(summary.pnl)} valueStyle={{ color: pnlColor(summary.pnl) }} />
            <Statistic title="收益率" value={pctFromRate(summary.pnl_rate)} valueStyle={{ color: pnlColor(summary.pnl_rate) }} />
            <Statistic
              title="今日盈亏(预估)"
              value={money(summary.today_pnl)}
              valueStyle={{ color: pnlColor(summary.today_pnl) }}
            />
            <Statistic
              title="今日收益率(预估)"
              value={pctFromRate(summary.today_pnl_rate)}
              valueStyle={{ color: pnlColor(summary.today_pnl_rate) }}
            />
            <Statistic title="持仓数量" value={positions.length} />
            <Statistic title="自选列表" value={watchlists.length} />
          </div>

          <div style={{ marginTop: 16 }}>
            <div className="fv-toolbarScroll">
              <Space wrap={false} style={{ whiteSpace: "nowrap" }}>
                <Link href="/funds">基金</Link>
                <Link href="/watchlists">自选</Link>
                <Link href="/accounts">账户</Link>
                <Link href="/positions">持仓</Link>
                <Link href="/sniffer">嗅探</Link>
                <Link href="/sim">模拟盘</Link>
                <Link href="/sources">数据源</Link>
              </Space>
            </div>
          </div>
        </Card>

        <Card title="父账户概览">
          <Table<Account>
            rowKey={(r) => r.id}
            loading={loading}
            columns={parentColumns}
            dataSource={parentAccounts}
            pagination={{
              pageSize: isMobile ? 10 : 20,
              simple: isMobile,
              showLessItems: isMobile,
              showSizeChanger: !isMobile,
            }}
            size={isMobile ? "small" : "middle"}
            scroll={isMobile ? undefined : { x: "max-content" }}
          />
        </Card>

        <Row gutter={16}>
          <Col xs={24} lg={12}>
            <Card title="最近操作流水">
              <Table<Operation>
                rowKey={(r) => r.id}
                loading={loading}
                columns={opColumns}
                dataSource={latestOperations}
                pagination={false}
                size="small"
                locale={{ emptyText: "暂无数据" }}
              />
            </Card>
          </Col>
          <Col xs={24} lg={12}>
            <Card title="盈亏靠前持仓">
              <Table<Position>
                rowKey={(r) => r.id}
                loading={loading}
                columns={posColumns}
                dataSource={topPositions}
                pagination={false}
                size="small"
                locale={{ emptyText: "暂无数据" }}
              />
            </Card>
          </Col>
        </Row>
      </div>
    </AuthedLayout>
  );
}
