"use client";

import { ReloadOutlined } from "@ant-design/icons";
import { Button, Col, Grid, Row, Table, message } from "antd";
import type { TableColumnsType } from "antd";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { AuthedLayout } from "../../components/AuthedLayout";
import { listAccounts, listPositions, listPositionOperations, listWatchlists } from "../../lib/api";

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

/** 盈亏配色：正为红（fv-up）、负为绿（fv-down），与 A 股惯例一致 */
function pnlClass(v: any): string {
  const n = toNumber(v);
  if (n === null) return "";
  return n >= 0 ? "fv-up" : "fv-down";
}

function opTypeText(t: "BUY" | "SELL"): string {
  return t === "BUY" ? "买入" : "卖出";
}

/** 收益率 pill：正红 / 负绿 / 空值中性 */
function RatePill({ value }: { value: any }) {
  const n = toNumber(value);
  if (n === null) return <span className="fv-pill fv-pillNeutral">-</span>;
  const cls = n >= 0 ? "fv-pillUp" : "fv-pillDown";
  const sign = n >= 0 ? "+" : "";
  return <span className={`fv-pill ${cls}`}>{`${sign}${(n * 100).toFixed(2)}%`}</span>;
}

/** 买卖方向 pill：买入红 / 卖出绿 */
function OpTypePill({ type }: { type: "BUY" | "SELL" }) {
  return <span className={`fv-pill ${type === "BUY" ? "fv-pillUp" : "fv-pillDown"}`}>{opTypeText(type)}</span>;
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

  const refreshBtn = (
    <Button icon={<ReloadOutlined />} loading={loading} onClick={() => void loadAll()}>
      刷新
    </Button>
  );

  const parentColumns: TableColumnsType<Account> = [
    {
      title: "账户",
      dataIndex: "name",
      key: "name",
      render: (v: any, record) => (
        <span style={{ whiteSpace: "nowrap" }}>
          {String(v ?? record.id)}
          {record.is_default ? (
            <span className="fv-pill fv-pillPrimary" style={{ marginLeft: 8 }}>
              默认
            </span>
          ) : null}
        </span>
      ),
    },
    {
      title: "持仓成本",
      dataIndex: "holding_cost",
      key: "holding_cost",
      width: 140,
      render: (v: any) => <span className="fv-num">{money(v)}</span>,
      responsive: ["md"],
    },
    {
      title: "持仓市值",
      dataIndex: "holding_value",
      key: "holding_value",
      width: 140,
      render: (v: any) => <span className="fv-num">{money(v)}</span>,
      responsive: ["md"],
    },
    {
      title: "总盈亏",
      dataIndex: "pnl",
      key: "pnl",
      width: 120,
      render: (v: any) => <span className={`fv-num ${pnlClass(v)}`}>{money(v)}</span>,
    },
  ];

  const opColumns: TableColumnsType<Operation> = [
    { title: "日期", dataIndex: "operation_date", key: "operation_date", width: 120 },
    {
      title: "类型",
      dataIndex: "operation_type",
      key: "operation_type",
      width: 90,
      render: (t: "BUY" | "SELL") => <OpTypePill type={t} />,
    },
    {
      title: "基金",
      key: "fund",
      render: (_: any, r) => r.fund_name ?? "-",
    },
    {
      title: "金额",
      dataIndex: "amount",
      key: "amount",
      width: 120,
      render: (v: any) => <span className="fv-num">{money(v)}</span>,
    },
    {
      title: "份额",
      dataIndex: "share",
      key: "share",
      width: 120,
      render: (v: any) => (v ? <span className="fv-num">{String(v)}</span> : "-"),
      responsive: ["md"],
    },
  ];

  const posColumns: TableColumnsType<Position> = [
    {
      title: "代码",
      dataIndex: "fund_code",
      key: "fund_code",
      width: 110,
      render: (v: any) => <span className="fv-num">{String(v ?? "-")}</span>,
    },
    { title: "基金名称", dataIndex: "fund_name", key: "fund_name", ellipsis: true },
    {
      title: "持仓成本",
      dataIndex: "holding_cost",
      key: "holding_cost",
      width: 140,
      render: (v: any) => <span className="fv-num">{money(v)}</span>,
      responsive: ["lg"],
    },
    {
      title: "盈亏",
      dataIndex: "pnl",
      key: "pnl",
      width: 120,
      render: (v: any) => <span className={`fv-num ${pnlClass(v)}`}>{money(v)}</span>,
    },
  ];

  return (
    <AuthedLayout
      title="仪表盘"
      subtitle={lastUpdateTime ? `更新于 ${lastUpdateTime.toLocaleTimeString()}` : undefined}
      extra={refreshBtn}
    >
      <div className="fv-pageHead">
        <div>
          <h1 className="fv-pageTitle">仪表盘</h1>
          <div className="fv-pageDesc">
            {parentAccounts.length} 个账户 · {positions.length} 只持仓
            {lastUpdateTime ? ` · 更新于 ${lastUpdateTime.toLocaleTimeString()}` : ""}
          </div>
        </div>
        <div className="fv-pageActions">{refreshBtn}</div>
      </div>

      <div className="fv-kpiGrid">
        <div className="fv-kpi">
          <div className="fv-kpiLabel">持仓成本</div>
          <div className="fv-kpiValue fv-num">{money(summary.holding_cost)}</div>
          <div className="fv-kpiFoot">
            <span className="fv-muted">累计投入本金</span>
          </div>
        </div>
        <div className="fv-kpi">
          <div className="fv-kpiAccent" style={{ background: "var(--fv-primary)" }} />
          <div className="fv-kpiLabel">持仓市值</div>
          <div className="fv-kpiValue fv-num">{money(summary.holding_value)}</div>
          <div className="fv-kpiFoot">
            <span className="fv-muted">按最新净值估算</span>
          </div>
        </div>
        <div className="fv-kpi">
          <div className="fv-kpiLabel">总盈亏</div>
          <div className={`fv-kpiValue fv-num ${pnlClass(summary.pnl)}`}>{money(summary.pnl)}</div>
          <div className="fv-kpiFoot">
            <RatePill value={summary.pnl_rate} />
            <span className="fv-muted">收益率</span>
          </div>
        </div>
        <div className="fv-kpi">
          <div className="fv-kpiLabel">今日盈亏（预估）</div>
          <div className={`fv-kpiValue fv-num ${pnlClass(summary.today_pnl)}`}>
            {money(summary.today_pnl)}
          </div>
          <div className="fv-kpiFoot">
            <RatePill value={summary.today_pnl_rate} />
            <span className="fv-muted">今日收益率</span>
          </div>
        </div>
        <div className="fv-kpi">
          <div className="fv-kpiLabel">持仓数量</div>
          <div className="fv-kpiValue fv-num">{positions.length}</div>
          <div className="fv-kpiFoot">
            <Link href="/positions">查看持仓 →</Link>
          </div>
        </div>
        <div className="fv-kpi">
          <div className="fv-kpiLabel">自选列表</div>
          <div className="fv-kpiValue fv-num">{watchlists.length}</div>
          <div className="fv-kpiFoot">
            <Link href="/watchlists">查看自选 →</Link>
          </div>
        </div>
      </div>

      <div className="fv-section">
        <div className="fv-card">
          <div className="fv-cardHead">
            <div className="fv-cardTitle">父账户概览</div>
          </div>
          <div className="fv-cardBody">
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
              locale={{ emptyText: <div className="fv-empty">暂无数据</div> }}
            />
          </div>
        </div>
      </div>

      <div className="fv-section">
        <Row gutter={[16, 16]}>
          <Col xs={24} lg={12}>
            <div className="fv-card">
              <div className="fv-cardHead">
                <div className="fv-cardTitle">最近操作流水</div>
              </div>
              <div className="fv-cardBody">
                <Table<Operation>
                  rowKey={(r) => r.id}
                  loading={loading}
                  columns={opColumns}
                  dataSource={latestOperations}
                  pagination={false}
                  size="small"
                  locale={{ emptyText: <div className="fv-empty">暂无数据</div> }}
                />
              </div>
            </div>
          </Col>
          <Col xs={24} lg={12}>
            <div className="fv-card">
              <div className="fv-cardHead">
                <div className="fv-cardTitle">盈亏靠前持仓</div>
              </div>
              <div className="fv-cardBody">
                <Table<Position>
                  rowKey={(r) => r.id}
                  loading={loading}
                  columns={posColumns}
                  dataSource={topPositions}
                  pagination={false}
                  size="small"
                  locale={{ emptyText: <div className="fv-empty">暂无数据</div> }}
                />
              </div>
            </div>
          </Col>
        </Row>
      </div>
    </AuthedLayout>
  );
}
