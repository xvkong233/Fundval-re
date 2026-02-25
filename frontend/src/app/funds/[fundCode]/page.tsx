"use client";

import dynamic from "next/dynamic";
import {
  Button,
  Card,
  Descriptions,
  Empty,
  Grid,
  Popover,
  Result,
  Select,
  Space,
  Spin,
  Statistic,
  Table,
  Tag,
  Tabs,
  Typography,
  message,
  theme,
} from "antd";
import { useEffect, useMemo, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { AuthedLayout } from "../../../components/AuthedLayout";
import {
  getFundDetail,
  computeFundAnalysisV2,
  getFundAnalysisV2,
  getFundEstimate,
  getFundSignals,
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
import { bucketForRangePosition, computeRangePositionPct } from "../../../lib/navPosition";
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
  const router = useRouter();
  const screens = Grid.useBreakpoint();
  const isMobile = !screens.md;

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
  const [navReloadKey, setNavReloadKey] = useState(0);
  const [timeRange, setTimeRange] = useState<TimeRange>("1M");
  const [compactChart, setCompactChart] = useState(false);
  const [showSwingPoints, setShowSwingPoints] = useState<boolean>(() => {
    if (typeof window === "undefined") return true;
    const raw = window.localStorage.getItem("fv_nav_swing_points");
    if (raw === "false") return false;
    if (raw === "true") return true;
    return true;
  });

  const [analysisV2Loading, setAnalysisV2Loading] = useState(false);
  const [analysisV2, setAnalysisV2] = useState<any | null>(null);
  const [analysisV2Error, setAnalysisV2Error] = useState<string | null>(null);
  const referIndexPresets = useMemo(
    () => [
      { value: "1.000001", label: "上证指数（000001）" },
      { value: "1.000300", label: "沪深300（000300）" },
      { value: "1.000905", label: "中证500（000905）" },
    ],
    []
  );
  const [referIndexCode, setReferIndexCode] = useState<string>(() => {
    if (typeof window === "undefined") return "1.000001";
    const raw = window.localStorage.getItem("fv_refer_index_code");
    return raw ? String(raw).trim() || "1.000001" : "1.000001";
  });
  useEffect(() => {
    if (typeof window === "undefined") return;
    window.localStorage.setItem("fv_refer_index_code", referIndexCode);
  }, [referIndexCode]);

  const [signalsLoading, setSignalsLoading] = useState(false);
  const [signals, setSignals] = useState<any | null>(null);
  const [signalsError, setSignalsError] = useState<string | null>(null);

  const [positionsLoading, setPositionsLoading] = useState(false);
  const [positionRows, setPositionRows] = useState<FundPositionRow[]>([]);

  const [operationsLoading, setOperationsLoading] = useState(false);
  const [operations, setOperations] = useState<OperationRow[]>([]);

  const { token } = theme.useToken();

  const title = useMemo(() => {
    if (!fund) return "基金详情";
    const type = String(fund.fund_type ?? "").trim();
    return `${fund.fund_name}（${fund.fund_code}）${type ? ` · ${type}` : ""}`;
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
      setNavReloadKey((v) => v + 1);
    } catch {
      message.error("加载历史净值失败");
      setNavHistory([]);
      setNavReloadKey((v) => v + 1);
    } finally {
      setNavLoading(false);
    }
  };

  useEffect(() => {
    void loadSources();
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
    if (navHistory.length < 2) {
      setSignals(null);
      return;
    }

    const load = async () => {
      setSignalsLoading(true);
      setSignalsError(null);
      try {
        const res = await getFundSignals(fundCode, { source });
        setSignals(res?.data ?? null);
      } catch (e: any) {
        const msg = e?.response?.data?.error || e?.response?.data?.detail || "加载预测信号失败";
        setSignals(null);
        setSignalsError(String(msg));
      } finally {
        setSignalsLoading(false);
      }
    };

    void load();
  }, [fundCode, source, navReloadKey, navHistory.length]);

  useEffect(() => {
    if (!fundCode) return;
    const load = async () => {
      setAnalysisV2Loading(true);
      setAnalysisV2Error(null);
      try {
        const res = await getFundAnalysisV2(fundCode, {
          source,
          profile: "default",
          refer_index_code: referIndexCode,
        });
        const data = res?.data ?? null;
        if (data && (data as any).missing) {
          setAnalysisV2(null);
          setAnalysisV2Error("尚未计算，请点击“重新计算”生成分析结果");
        } else {
          setAnalysisV2(data);
        }
      } catch (e: any) {
        const status = e?.response?.status;
        if (status === 404) {
          setAnalysisV2(null);
          setAnalysisV2Error("尚未计算，请点击“重新计算”生成分析结果");
        } else {
          const msg = e?.response?.data?.error || e?.response?.data?.detail || "加载基金分析失败";
          setAnalysisV2(null);
          setAnalysisV2Error(String(msg));
        }
      } finally {
        setAnalysisV2Loading(false);
      }
    };

    void load();
  }, [fundCode, source, referIndexCode, navReloadKey]);

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

  const bestPeerSignals = useMemo(() => {
    const peers = Array.isArray(signals?.peers) ? (signals.peers as any[]) : [];
    if (!peers.length) return null;
    const bestCode = String(signals?.best_peer_code ?? "");
    const best = peers.find((p) => String(p?.peer_code ?? "") === bestCode);
    return best ?? peers[0];
  }, [signals]);

  const forecastWindow = useMemo(() => {
    const wins = analysisV2?.result?.windows;
    if (!Array.isArray(wins) || wins.length === 0) return null;
    return wins[0];
  }, [analysisV2]);

  const [showForecastOverlay, setShowForecastOverlay] = useState<boolean>(() => {
    if (typeof window === "undefined") return true;
    const raw = window.localStorage.getItem("fv_nav_forecast_overlay");
    if (raw === "false") return false;
    if (raw === "true") return true;
    return true;
  });

  const [activeTab, setActiveTab] = useState<string>(() => {
    if (typeof window === "undefined") return "analysis_v2";
    const raw = window.localStorage.getItem("fv_fund_detail_tab");
    return raw ? String(raw) : "analysis_v2";
  });
  useEffect(() => {
    if (typeof window === "undefined") return;
    try {
      window.localStorage.setItem("fv_fund_detail_tab", activeTab);
    } catch {
      // ignore
    }
  }, [activeTab]);

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

  const rangePositionPct = computeRangePositionPct(navHistory);
  const rangePositionBucket = rangePositionPct === null ? null : bucketForRangePosition(rangePositionPct);
  const rangePositionLabel = rangePositionBucket === "low" ? "偏低" : rangePositionBucket === "high" ? "偏高" : "中等";
  const rangePositionColor = rangePositionBucket === "low" ? "green" : rangePositionBucket === "high" ? "red" : "gold";

  const toNumber = (v: any): number | null => {
    if (v === null || v === undefined || v === "") return null;
    const n = Number(v);
    return Number.isFinite(n) ? n : null;
  };

  const bucketLabel = (b: any) => {
    const s = String(b ?? "").toLowerCase();
    if (s === "low") return { text: "偏低", color: "green" };
    if (s === "high") return { text: "偏高", color: "red" };
    return { text: "中等", color: "gold" };
  };

  return (
    <AuthedLayout title={title}>
      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
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

          <div className="fv-kpiGrid" style={{ marginTop: 16 }}>
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

        <Tabs
          activeKey={activeTab}
          onChange={(k) => setActiveTab(String(k))}
          size={isMobile ? "small" : "middle"}
          items={[
            { key: "analysis_v2", label: "分析 v2" },
            { key: "signals", label: "预测信号" },
            { key: "nav", label: "净值曲线" },
            { key: "holdings", label: "持仓/操作" },
          ]}
        />

        {activeTab === "analysis_v2" ? (
        <Card
          title="基金分析 v2（Qbot/xalpha）"
          loading={analysisV2Loading}
          extra={
            <div className="fv-toolbarScroll">
              <Space size={8} wrap>
                <Select
                  size="small"
                  value={referIndexCode}
                  style={{ width: 180 }}
                  onChange={(v) => setReferIndexCode(String(v))}
                  options={referIndexPresets}
                />
                {analysisV2?.as_of_date ? <Tag color="default">as_of：{String(analysisV2.as_of_date)}</Tag> : null}
                {analysisV2?.updated_at ? (
                  <Tag color="default">更新：{String(analysisV2.updated_at).replace("T", " ").slice(0, 19)}</Tag>
                ) : null}
                {analysisV2?.last_task_id ? (
                  <Button
                    size="small"
                    onClick={() => void router.push(`/tasks/${encodeURIComponent(String(analysisV2.last_task_id))}`)}
                  >
                    查看任务日志
                  </Button>
                ) : null}
                <Button
                  type="primary"
                  size="small"
                  onClick={async () => {
                    try {
                      const r = await computeFundAnalysisV2(fundCode, {
                        source,
                        profile: "default",
                        windows: [60],
                        refer_index_code: referIndexCode,
                      });
                      const taskId = String(r?.data?.task_id ?? "");
                      if (taskId) {
                        message.success(`已入队：${taskId}`);
                        void router.push(`/tasks/${encodeURIComponent(taskId)}`);
                      } else {
                        message.success("已入队");
                      }
                    } catch (e: any) {
                      const msg = e?.response?.data?.error || e?.response?.data?.detail || "入队失败";
                      message.error(String(msg));
                    }
                  }}
                >
                  重新计算
                </Button>
              </Space>
            </div>
          }
        >
          {analysisV2Error ? <Result status="info" title="暂无分析快照" subTitle={analysisV2Error} /> : null}

          {analysisV2?.result?.windows ? (
            <Table
              size={isMobile ? "small" : "middle"}
              pagination={false}
              rowKey={(r: any) => String(r?.window ?? "")}
              dataSource={Array.isArray(analysisV2.result.windows) ? analysisV2.result.windows : []}
              scroll={{ x: "max-content" }}
              columns={[
                {
                  title: "窗口",
                  key: "window",
                  width: 90,
                  render: (_: any, r: any) => <Tag color="geekblue">{String(r?.window ?? "-")}T</Tag>,
                },
                {
                  title: "收益/回撤",
                  key: "return",
                  render: (_: any, r: any) => {
                    const m = r?.metrics?.metrics;
                    const tr = toNumber(m?.total_return);
                    const cagr = toNumber(m?.cagr);
                    const dd = toNumber(m?.max_drawdown);
                    const low = r?.forecast?.low;
                    const high = r?.forecast?.high;
                    const lowStep = typeof low?.step === "number" ? (low.step as number) : null;
                    const highStep = typeof high?.step === "number" ? (high.step as number) : null;
                    const lowNav = toNumber(low?.nav);
                    const highNav = toNumber(high?.nav);
                    return (
                      <Space wrap>
                        <Tag color="blue">TR：{tr === null ? "-" : `${(tr * 100).toFixed(2)}%`}</Tag>
                        <Tag color="purple">CAGR：{cagr === null ? "-" : `${(cagr * 100).toFixed(2)}%`}</Tag>
                        <Tag color={dd !== null && dd < 0 ? "red" : "default"}>
                          MDD：{dd === null ? "-" : `${(dd * 100).toFixed(2)}%`}
                        </Tag>
                        <Tag color="green">
                          低点：{lowStep === null ? "-" : `f+${lowStep}`}（{lowNav === null ? "-" : lowNav.toFixed(4)}）
                        </Tag>
                        <Tag color="red">
                          高点：{highStep === null ? "-" : `f+${highStep}`}（{highNav === null ? "-" : highNav.toFixed(4)}）
                        </Tag>
                      </Space>
                    );
                  },
                },
                {
                  title: "Sharpe/波动",
                  key: "risk",
                  width: 220,
                  render: (_: any, r: any) => {
                    const m = r?.metrics?.metrics;
                    const sharpe = toNumber(m?.sharpe);
                    const vol = toNumber(m?.vol_annual);
                    return (
                      <Space wrap>
                        <Tag color="default">S：{sharpe === null ? "-" : sharpe.toFixed(2)}</Tag>
                        <Tag color="default">Vol：{vol === null ? "-" : `${(vol * 100).toFixed(2)}%`}</Tag>
                      </Space>
                    );
                  },
                },
                {
                  title: "策略输出",
                  key: "rules",
                  render: (_: any, r: any) => {
                    const macdPts = Array.isArray(r?.macd?.points) ? r.macd.points.length : 0;
                    const tsActs = Array.isArray(r?.fund_strategies_ts?.actions)
                      ? r.fund_strategies_ts.actions.length
                      : null;
                    const gridActs = Array.isArray(r?.grid?.actions) ? r.grid.actions.length : 0;
                    const schedActs = Array.isArray(r?.scheduled?.actions) ? r.scheduled.actions.length : 0;
                    return (
                      <Space wrap>
                        <Tag>MACD：{macdPts}</Tag>
                        <Tag>TS：{tsActs === null ? "-" : tsActs}</Tag>
                        <Tag>Grid：{gridActs}</Tag>
                        <Tag>定投：{schedActs}</Tag>
                      </Space>
                    );
                  },
                },
              ]}
            />
          ) : null}
        </Card>
        ) : null}

        {activeTab === "signals" ? (
        <Card
          title="预测信号（ML）"
          loading={signalsLoading}
          extra={
            <div className="fv-toolbarScroll">
              <Space size={8} wrap>
                {!isMobile ? <Tag color="geekblue">两套窗口：5T + 20T（默认 20T）</Tag> : null}
                {bestPeerSignals ? (
                  <Popover
                    title="关联板块（同类）"
                    content={
                      <div style={{ width: isMobile ? 260 : 360 }}>
                        <Space direction="vertical" size={6} style={{ width: "100%" }}>
                          {(Array.isArray(signals?.peers) ? (signals?.peers as any[]) : []).map((p) => {
                            const name = String(p?.peer_name ?? "-");
                            const code = String(p?.peer_code ?? "-");
                            const dip20 =
                              typeof p?.dip_buy?.p_20t === "number" ? (p.dip_buy.p_20t as number) * 100 : null;
                            const dip5 =
                              typeof p?.dip_buy?.p_5t === "number" ? (p.dip_buy.p_5t as number) * 100 : null;
                            return (
                              <div
                                key={`${code}-${name}`}
                                style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12 }}
                              >
                                <Text style={{ maxWidth: isMobile ? 120 : 160 }} ellipsis={{ tooltip: name }}>
                                  {name}
                                </Text>
                                <Text type="secondary" style={{ fontSize: 12, whiteSpace: "nowrap" }}>
                                  抄底 {dip20 !== null ? dip20.toFixed(1) : "-"}%（20T）/ {dip5 !== null ? dip5.toFixed(1) : "-"}%（5T）
                                </Text>
                              </div>
                            );
                          })}
                        </Space>
                      </div>
                    }
                  >
                    <Tag color="blue" style={{ cursor: "pointer" }}>
                      同类：{String(bestPeerSignals?.peer_name ?? "-")}
                    </Tag>
                  </Popover>
                ) : (
                  <Tag>同类：-</Tag>
                )}
                {signals?.as_of_date ? <Tag color="default">as_of：{String(signals.as_of_date)}</Tag> : null}
              </Space>
            </div>
          }
        >
          {signalsError ? (
            <Result status="warning" title="预测信号暂不可用" subTitle={signalsError} />
          ) : bestPeerSignals ? (
            <div className="fv-kpiGrid4">
              <div>
                <div style={{ marginBottom: 8 }}>
                  <Text type="secondary">位置（同类分桶）</Text>
                </div>
                {(() => {
                  const b = bucketLabel(bestPeerSignals?.position_bucket);
                  const p =
                    typeof bestPeerSignals?.position_percentile_0_100 === "number"
                      ? (bestPeerSignals.position_percentile_0_100 as number)
                      : null;
                  return (
                    <Space wrap>
                      <Tag color={b.color}>{b.text}</Tag>
                      <Text type="secondary">{p !== null ? `分位 ${p.toFixed(0)}%` : "分位 -"}</Text>
                    </Space>
                  );
                })()}
              </div>

              <Statistic
                title="回撤抄底概率（20T）"
                value={
                  typeof bestPeerSignals?.dip_buy?.p_20t === "number" ? (bestPeerSignals.dip_buy.p_20t as number) * 100 : "-"
                }
                precision={typeof bestPeerSignals?.dip_buy?.p_20t === "number" ? 1 : 0}
                suffix={typeof bestPeerSignals?.dip_buy?.p_20t === "number" ? "%" : ""}
                valueStyle={{ color: token.colorPrimary }}
              />
              <Statistic
                title="回撤抄底概率（5T）"
                value={
                  typeof bestPeerSignals?.dip_buy?.p_5t === "number" ? (bestPeerSignals.dip_buy.p_5t as number) * 100 : "-"
                }
                precision={typeof bestPeerSignals?.dip_buy?.p_5t === "number" ? 1 : 0}
                suffix={typeof bestPeerSignals?.dip_buy?.p_5t === "number" ? "%" : ""}
              />
              <Statistic
                title="神奇反转概率（20T）"
                value={
                  typeof bestPeerSignals?.magic_rebound?.p_20t === "number"
                    ? (bestPeerSignals.magic_rebound.p_20t as number) * 100
                    : "-"
                }
                precision={typeof bestPeerSignals?.magic_rebound?.p_20t === "number" ? 1 : 0}
                suffix={typeof bestPeerSignals?.magic_rebound?.p_20t === "number" ? "%" : ""}
              />
              <Statistic
                title="神奇反转概率（5T）"
                value={
                  typeof bestPeerSignals?.magic_rebound?.p_5t === "number"
                    ? (bestPeerSignals.magic_rebound.p_5t as number) * 100
                    : "-"
                }
                precision={typeof bestPeerSignals?.magic_rebound?.p_5t === "number" ? 1 : 0}
                suffix={typeof bestPeerSignals?.magic_rebound?.p_5t === "number" ? "%" : ""}
              />
            </div>
          ) : (
            <Empty description="暂无信号（需要先同步净值与关联板块缓存）" />
          )}

          <div style={{ marginTop: 12 }}>
            <Text type="secondary" style={{ fontSize: 12 }}>
              说明：信号与概率为模型输出，仅用于辅助理解当前“位置/回撤后的反弹概率”，不构成投资建议；模型会随数据源、板块同类样本与训练数据变化而变化。
            </Text>
          </div>
        </Card>
        ) : null}

        {activeTab === "nav" ? (
        <Card
          title="历史净值"
          extra={
            <div className="fv-toolbarScroll">
              <Space style={{ whiteSpace: "nowrap" }} size={8}>
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
                  onClick={() => {
                    const next = !showSwingPoints;
                    setShowSwingPoints(next);
                    try {
                      window.localStorage.setItem("fv_nav_swing_points", next ? "true" : "false");
                    } catch {}
                  }}
                >
                  高低点{showSwingPoints ? "：开" : "：关"}
                </Button>
                <Button
                  size="small"
                  disabled={!forecastWindow?.forecast}
                  onClick={() => {
                    const next = !showForecastOverlay;
                    setShowForecastOverlay(next);
                    try {
                      window.localStorage.setItem("fv_nav_forecast_overlay", next ? "true" : "false");
                    } catch {}
                  }}
                >
                  预测{showForecastOverlay ? "：开" : "：关"}
                </Button>
                <Button size="small" loading={navLoading} onClick={() => void syncAndLoadNav(timeRange)}>
                  同步并加载
                </Button>
                {rangePositionPct !== null ? (
                  <Tag color={rangePositionColor}>
                    区间位置：{rangePositionPct.toFixed(0)}%（{rangePositionLabel}）
                  </Tag>
                ) : null}
                {!isMobile && forecastWindow?.as_of_date ? (
                  <Tag color="default">seed_as_of：{String(forecastWindow.as_of_date)}</Tag>
                ) : null}
                {!isMobile && forecastWindow?.seed_points ? (
                  <Tag color="default">seed_points：{String(forecastWindow.seed_points)}</Tag>
                ) : null}
              </Space>
            </div>
          }
        >
          {navHistory.length > 0 ? (
            <div style={{ marginBottom: 16 }}>
              <ReactECharts
                option={buildNavChartOption(navHistory, {
                  compact: compactChart,
                  color: token.colorPrimary,
                  swing: { enabled: showSwingPoints, window: compactChart ? 3 : 5, maxPointsPerKind: 6 },
                  forecast: showForecastOverlay ? (forecastWindow?.forecast as any) : undefined,
                })}
                style={{ height: compactChart ? 300 : 400 }}
              />
            </div>
          ) : null}
          <Table<NavRow>
            rowKey={(r) => `${r.nav_date ?? ""}`}
            loading={navLoading}
            dataSource={navHistory}
            pagination={{ pageSize: isMobile ? 10 : 20, simple: isMobile, showLessItems: isMobile }}
            locale={{ emptyText: "暂无数据（可点击右上角“同步并加载”）" }}
            columns={[
              { title: "日期", dataIndex: "nav_date", width: 140 },
              { title: "单位净值", dataIndex: "unit_nav", render: (v: any) => (v ? Number(v).toFixed(4) : "-") },
              {
                title: "累计净值",
                dataIndex: "accumulated_nav",
                responsive: ["md"],
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
        ) : null}

        {activeTab === "holdings" ? (
          <>
            {positionRows.length > 0 ? (
              <Card title="我的持仓" loading={positionsLoading}>
                <Table<FundPositionRow>
                  rowKey={(r) => r.account_name}
                  dataSource={positionRows}
                  pagination={false}
                  scroll={{ x: "max-content" }}
                  size={isMobile ? "small" : "middle"}
                  columns={[
                    { title: "账户", dataIndex: "account_name", key: "account_name" },
                    {
                      title: "持仓份额",
                      dataIndex: "holding_share",
                      key: "holding_share",
                      responsive: ["md"],
                      render: (v: any) => (Number.isFinite(Number(v)) ? Number(v).toFixed(2) : "-"),
                    },
                    {
                      title: "持仓成本",
                      dataIndex: "holding_cost",
                      key: "holding_cost",
                      responsive: ["md"],
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
                          <span style={{ color: positive ? "#cf1322" : "#3f8600", whiteSpace: "nowrap" }}>
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
                  pagination={{ pageSize: isMobile ? 10 : 20, simple: isMobile, showLessItems: isMobile }}
                  scroll={{ x: "max-content" }}
                  size={isMobile ? "small" : "middle"}
                  columns={[
                    { title: "日期", dataIndex: "operation_date", width: 120 },
                    { title: "账户", dataIndex: "account_name", width: 160, ellipsis: true, responsive: ["md"] },
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
                      responsive: ["md"],
                      render: (v: any) => (v ? Number(v).toFixed(4) : "-"),
                    },
                    {
                      title: "净值",
                      dataIndex: "nav",
                      width: 120,
                      responsive: ["md"],
                      render: (v: any) => (v ? Number(v).toFixed(4) : "-"),
                    },
                    {
                      title: "15点前",
                      dataIndex: "before_15",
                      width: 110,
                      responsive: ["lg"],
                      render: (v: any) => (v === true ? "是" : v === false ? "否" : "-"),
                    },
                  ]}
                />
              </Card>
            ) : null}

            {positionRows.length === 0 && operations.length === 0 ? <Empty description="暂无持仓/操作记录" /> : null}
          </>
        ) : null}
      </div>
    </AuthedLayout>
  );
}

