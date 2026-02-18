"use client";

import dynamic from "next/dynamic";
import {
  Button,
  Card,
  Descriptions,
  Empty,
  Result,
  Select,
  Space,
  Spin,
  Statistic,
  Table,
  Tag,
  Typography,
  message,
  theme,
} from "antd";
import { useEffect, useMemo, useState } from "react";
import { useParams } from "next/navigation";
import { AuthedLayout } from "../../../components/AuthedLayout";
import {
  getFundDetail,
  getFundEstimate,
  listNavHistory,
  listPositionOperations,
  listPositions,
  listSources,
  syncNavHistory,
} from "../../../lib/api";
import { getDateRange, type TimeRange } from "../../../lib/dateRange";
import { buildNavChartOption } from "../../../lib/navChart";
import { buildFundPositionRows, sortOperationsDesc, type FundPositionRow } from "../../../lib/fundDetail";
import { normalizeNavHistoryRows } from "../../../lib/navHistoryNormalize";
import { sourceDisplayName, type SourceItem } from "../../../lib/sources";

const { Text } = Typography;

type NavRow = Record<string, any> & {
  nav_date?: string;
  unit_nav?: string | number;
  accumulated_nav?: string | number | null;
  daily_growth?: string | number | null;
};
type OperationRow = Record<string, any> & {
  id?: string;
  account_name?: string;
  operation_type?: string;
  operation_date?: string;
  before_15?: boolean;
  amount?: string;
  share?: string;
  nav?: string;
  created_at?: string;
};

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });

export default function FundDetailPage() {
  const params = useParams<{ fundCode: string }>();
  const fundCode = decodeURIComponent(params?.fundCode ?? "");

  const [loading, setLoading] = useState(true);
  const [fund, setFund] = useState<any | null>(null);
  const [fundNotFound, setFundNotFound] = useState(false);
  const [fundLoadError, setFundLoadError] = useState<string | null>(null);
  const [estimate, setEstimate] = useState<any | null>(null);

  const [sourcesLoading, setSourcesLoading] = useState(false);
  const [sources, setSources] = useState<SourceItem[]>([]);
  const [source, setSource] = useState<string>("tiantian");

  const [navLoading, setNavLoading] = useState(false);
  const [navHistory, setNavHistory] = useState<NavRow[]>([]);
  const [timeRange, setTimeRange] = useState<TimeRange>("1M");
  const [compactChart, setCompactChart] = useState(false);

  const [positionsLoading, setPositionsLoading] = useState(false);
  const [positionRows, setPositionRows] = useState<FundPositionRow[]>([]);

  const [operationsLoading, setOperationsLoading] = useState(false);
  const [operations, setOperations] = useState<OperationRow[]>([]);

  const { token } = theme.useToken();

  const title = useMemo(() => {
    if (!fund) return "基金详情";
    return (
      <Space direction="vertical" size={0}>
        <span>
          {fund.fund_name}（{fund.fund_code}）
        </span>
        <Text type="secondary" style={{ fontSize: 12 }}>
          {fund.fund_type || ""}
        </Text>
      </Space>
    );
  }, [fund]);

  const loadSources = async () => {
    setSourcesLoading(true);
    try {
      const res = await listSources();
      const list = Array.isArray(res.data) ? (res.data as SourceItem[]) : [];
      setSources(list);
    } catch {
      setSources([]);
    } finally {
      setSourcesLoading(false);
    }
  };

  const loadFund = async () => {
    setLoading(true);
    setFundLoadError(null);
    setFundNotFound(false);
    try {
      const detailRes = await getFundDetail(fundCode);
      setFund(detailRes.data);
    } catch (e: any) {
      const status = e?.response?.status as number | undefined;
      if (status === 404) {
        setFundNotFound(true);
      } else {
        const msg = e?.response?.data?.error || e?.response?.data?.detail || "加载基金详情失败";
        setFundLoadError(String(msg));
        message.error(String(msg));
      }
      setFund(null);
    } finally {
      setLoading(false);
    }
  };

  const loadEstimate = async (sourceName: string) => {
    try {
      const estimateRes = await getFundEstimate(fundCode, sourceName).catch(() => null);
      setEstimate(estimateRes?.data ?? null);
    } catch {
      setEstimate(null);
    }
  };

  const loadPositionsAndOperations = async (latestNav?: string | number | null) => {
    setPositionsLoading(true);
    setOperationsLoading(true);

    try {
      const [posRes, opRes] = await Promise.all([
        listPositions().catch(() => null),
        listPositionOperations({ fund_code: fundCode }).catch(() => null),
      ]);

      const positions = Array.isArray(posRes?.data) ? (posRes?.data as any[]) : [];
      setPositionRows(buildFundPositionRows(positions, fundCode, latestNav));

      const ops = Array.isArray(opRes?.data) ? (opRes?.data as OperationRow[]) : [];
      setOperations(sortOperationsDesc(ops));
    } catch {
      setPositionRows([]);
      setOperations([]);
    } finally {
      setPositionsLoading(false);
      setOperationsLoading(false);
    }
  };

  const syncAndLoadNav = async (range: TimeRange) => {
    setNavLoading(true);
    try {
      const now = new Date();
      const { startDate, endDate } = getDateRange(range, now);

      // 同步失败不阻断展示（与旧前端一致）
      try {
        await syncNavHistory([fundCode], startDate, endDate, source);
      } catch {
        // ignore
      }

      const params = { start_date: startDate, end_date: endDate, source };
      const res = await listNavHistory(fundCode, params);
      const rows = Array.isArray(res.data) ? (res.data as NavRow[]) : [];
      const normalized = normalizeNavHistoryRows(rows);
      normalized.sort((a, b) => String(a.nav_date).localeCompare(String(b.nav_date)));
      setNavHistory(normalized as NavRow[]);
    } catch {
      message.error("加载历史净值失败");
      setNavHistory([]);
    } finally {
      setNavLoading(false);
    }
  };

  useEffect(() => {
    void loadSources();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const saved = window.localStorage.getItem("fundval_source");
    if (saved && saved.trim()) setSource(saved.trim());
  }, []);

  useEffect(() => {
    if (!sources.length) return;
    const has = sources.some((s) => String(s?.name ?? "") === source);
    if (!has) setSource(String(sources[0]?.name ?? "tiantian"));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sources]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    window.localStorage.setItem("fundval_source", source);
  }, [source]);

  useEffect(() => {
    if (!fundCode) return;
    void loadFund();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fundCode]);

  useEffect(() => {
    if (!fundCode) return;
    void loadEstimate(source);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fundCode, source]);

  useEffect(() => {
    if (!fundCode) return;
    void syncAndLoadNav(timeRange);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fundCode, timeRange, source]);

  useEffect(() => {
    if (!fundCode) return;
    const latestFromHistory = navHistory.length ? navHistory[navHistory.length - 1]?.unit_nav : null;
    const latestNav = latestFromHistory ?? fund?.latest_nav ?? fund?.yesterday_nav ?? null;
    void loadPositionsAndOperations(latestNav);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fundCode, navHistory, fund?.latest_nav, fund?.yesterday_nav]);

  useEffect(() => {
    const update = () => setCompactChart(window.innerWidth < 768);
    update();
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

  if (loading) {
    return (
      <AuthedLayout title="基金详情">
        <Card>
          <div style={{ textAlign: "center", padding: "50px 0" }}>
            <Spin tip="加载中..." />
          </div>
        </Card>
      </AuthedLayout>
    );
  }

  if (!fund) {
    return (
      <AuthedLayout title="基金详情">
        <Card>
          {fundLoadError ? (
            <Result
              status="error"
              title="加载失败"
              subTitle={fundLoadError}
              extra={
                <Button type="primary" onClick={() => void loadFund()}>
                  重试
                </Button>
              }
            />
          ) : (
            <Empty description={fundNotFound ? "基金不存在" : "暂无数据"} />
          )}
        </Card>
      </AuthedLayout>
    );
  }

  const latestRow = navHistory.length ? navHistory[navHistory.length - 1] : null;
  const latestNav = latestRow?.unit_nav ?? fund.latest_nav ?? fund.yesterday_nav;
  const latestNavDate = latestRow?.nav_date ?? fund.latest_nav_date ?? fund.yesterday_nav_date;

  return (
    <AuthedLayout title={title}>
      <Space direction="vertical" size="large" style={{ width: "100%" }}>
        <Card title="基金信息">
          <Descriptions column={{ xs: 1, sm: 2, md: 3 }}>
            <Descriptions.Item label="基金代码">{fund.fund_code}</Descriptions.Item>
            <Descriptions.Item label="基金名称">{fund.fund_name}</Descriptions.Item>
            <Descriptions.Item label="基金类型">{fund.fund_type || "-"}</Descriptions.Item>
            <Descriptions.Item label="数据源">
              <Space wrap size={8}>
                <Select
                  style={{ minWidth: 160 }}
                  loading={sourcesLoading}
                  value={source}
                  onChange={(v) => setSource(String(v))}
                  options={(sources.length ? sources : [{ name: "tiantian" }]).map((s) => ({
                    label: `${sourceDisplayName(s.name)} (${s.name})`,
                    value: s.name,
                  }))}
                />
                <Tag color="blue">{sourceDisplayName(source)}</Tag>
              </Space>
            </Descriptions.Item>
          </Descriptions>

          <div style={{ marginTop: 16, display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 16 }}>
            <Statistic
              title="最新净值"
              value={latestNav || "-"}
              precision={latestNav ? 4 : 0}
              prefix={latestNav ? "¥" : ""}
              suffix={latestNavDate ? ` (${String(latestNavDate).slice(5)})` : ""}
            />
            <Statistic
              title="实时估值"
              value={estimate?.estimate_nav || estimate?.estimate_value || "-"}
              precision={estimate?.estimate_nav || estimate?.estimate_value ? 4 : 0}
              prefix={estimate?.estimate_nav || estimate?.estimate_value ? "¥" : ""}
            />
            <Statistic
              title="估算涨跌"
              value={estimate?.estimate_growth || estimate?.estimate_growth_rate || "-"}
              precision={estimate?.estimate_growth || estimate?.estimate_growth_rate ? 2 : 0}
              suffix={estimate?.estimate_growth || estimate?.estimate_growth_rate ? "%" : ""}
              valueStyle={{
                color:
                  Number(estimate?.estimate_growth ?? estimate?.estimate_growth_rate) >= 0 ? "#cf1322" : "#3f8600",
              }}
              prefix={
                Number(estimate?.estimate_growth ?? estimate?.estimate_growth_rate) >= 0 ? "+" : ""
              }
            />
          </div>
        </Card>

        <Card
          title="历史净值"
          extra={
            <Space wrap>
              {(["1W", "1M", "3M", "6M", "1Y", "ALL"] as TimeRange[]).map((range) => (
                <Button
                  key={range}
                  size="small"
                  type={timeRange === range ? "primary" : "default"}
                  onClick={() => setTimeRange(range)}
                >
                  {range === "ALL" ? "全部" : range === "1W" ? "1周" : range}
                </Button>
              ))}
              <Button
                size="small"
                loading={navLoading}
                onClick={() => void syncAndLoadNav(timeRange)}
              >
                同步并加载
              </Button>
            </Space>
          }
        >
          {navHistory.length > 0 ? (
            <div style={{ marginBottom: 16 }}>
              <ReactECharts
                option={buildNavChartOption(navHistory, { compact: compactChart, color: token.colorPrimary })}
                style={{ height: compactChart ? 300 : 400 }}
              />
            </div>
          ) : null}
          <Table<NavRow>
            rowKey={(r) => `${r.nav_date ?? ""}`}
            loading={navLoading}
            dataSource={navHistory}
            pagination={{ pageSize: 20 }}
            locale={{ emptyText: "暂无数据（可点击右上角“同步并加载”）" }}
            columns={[
              { title: "日期", dataIndex: "nav_date", width: 140 },
              { title: "单位净值", dataIndex: "unit_nav", render: (v: any) => (v ? Number(v).toFixed(4) : "-") },
              {
                title: "累计净值",
                dataIndex: "accumulated_nav",
                render: (v: any) => (v ? Number(v).toFixed(4) : "-"),
              },
              {
                title: "日涨跌(%)",
                dataIndex: "daily_growth",
                render: (v: any) => {
                  if (v === null || v === undefined || v === "") return "-";
                  const n = Number(v);
                  if (!Number.isFinite(n)) return String(v);
                  const positive = n >= 0;
                  const text = `${positive ? "+" : ""}${n.toFixed(2)}`;
                  return <span style={{ color: positive ? "#cf1322" : "#3f8600" }}>{text}</span>;
                },
              },
            ]}
          />
        </Card>

        {positionRows.length > 0 ? (
          <Card title="我的持仓" loading={positionsLoading}>
            <Table<FundPositionRow>
              rowKey={(r) => r.account_name}
              dataSource={positionRows}
              pagination={false}
              scroll={{ x: "max-content" }}
              columns={[
                { title: "账户", dataIndex: "account_name", key: "account_name" },
                {
                  title: "持仓份额",
                  dataIndex: "holding_share",
                  key: "holding_share",
                  render: (v: any) => (Number.isFinite(Number(v)) ? Number(v).toFixed(2) : "-"),
                },
                {
                  title: "持仓成本",
                  dataIndex: "holding_cost",
                  key: "holding_cost",
                  render: (v: any) => (Number.isFinite(Number(v)) ? `¥${Number(v).toFixed(2)}` : "-"),
                },
                {
                  title: "市值",
                  dataIndex: "market_value",
                  key: "market_value",
                  render: (v: any) => (Number.isFinite(Number(v)) ? `¥${Number(v).toFixed(2)}` : "-"),
                },
                {
                  title: "盈亏",
                  dataIndex: "pnl",
                  key: "pnl",
                  render: (_: any, record) => {
                    const pnl = record.pnl;
                    const pnlRate = record.pnl_rate;
                    if (pnl === null || pnl === undefined) return "-";
                    const positive = pnl >= 0;
                    const rateText =
                      pnlRate === null || pnlRate === undefined
                        ? ""
                        : ` (${pnlRate >= 0 ? "+" : ""}${pnlRate.toFixed(2)}%)`;
                    return (
                      <span style={{ color: positive ? "#cf1322" : "#3f8600" }}>
                        {positive ? "+" : ""}¥{pnl.toFixed(2)}
                        {rateText}
                      </span>
                    );
                  },
                },
              ]}
            />
          </Card>
        ) : null}

        {operations.length > 0 ? (
          <Card title="操作记录" loading={operationsLoading}>
            <Table<OperationRow>
              rowKey={(r) => String(r.id ?? `${r.operation_date ?? ""}-${r.created_at ?? ""}`)}
              dataSource={operations}
              pagination={{ pageSize: 20 }}
              scroll={{ x: "max-content" }}
              columns={[
                { title: "日期", dataIndex: "operation_date", width: 120 },
                { title: "账户", dataIndex: "account_name", width: 160, ellipsis: true },
                {
                  title: "类型",
                  dataIndex: "operation_type",
                  width: 120,
                  render: (v: any) => (v === "BUY" ? "买入" : v === "SELL" ? "卖出" : String(v ?? "-")),
                },
                {
                  title: "金额",
                  dataIndex: "amount",
                  width: 140,
                  render: (v: any) => (v ? `¥${Number(v).toFixed(2)}` : "-"),
                },
                {
                  title: "份额",
                  dataIndex: "share",
                  width: 140,
                  render: (v: any) => (v ? Number(v).toFixed(4) : "-"),
                },
                {
                  title: "净值",
                  dataIndex: "nav",
                  width: 120,
                  render: (v: any) => (v ? Number(v).toFixed(4) : "-"),
                },
                {
                  title: "15点前",
                  dataIndex: "before_15",
                  width: 110,
                  render: (v: any) => (v === true ? "是" : v === false ? "否" : "-"),
                },
              ]}
            />
          </Card>
        ) : null}
      </Space>
    </AuthedLayout>
  );
}

