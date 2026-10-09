"use client";

import dynamic from "next/dynamic";
import {
  Button,
  Empty,
  Grid,
  Popover,
  Result,
  Select,
  Space,
  Spin,
  Table,
  Tag,
  Tabs,
  Typography,
  message,
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
        <div className="fv-card">
          <div className="fv-empty">
            <Spin tip="加载中..." />
          </div>
        </div>
      </AuthedLayout>
    );
  }

  if (!fund) {
    return (
      <AuthedLayout title="基金详情">
        <div className="fv-card">
          <div className="fv-cardBody">
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
          </div>
        </div>
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

  const latestNavNum = toNumber(latestNav);
  const latestNavText = latestNavNum !== null ? latestNavNum.toFixed(4) : "-";
  const dailyGrowthNum = toNumber(latestRow?.daily_growth);
  const estimateNavRaw = estimate?.estimate_nav ?? estimate?.estimate_value;
  const estimateNavNum = toNumber(estimateNavRaw);
  const estimateNavText = estimateNavNum !== null ? estimateNavNum.toFixed(4) : "-";
  const estimateGrowthNum = toNumber(estimate?.estimate_growth ?? estimate?.estimate_growth_rate);
  const estimateTimeRaw = typeof estimate?.estimate_time === "string" ? (estimate.estimate_time as string) : "";
  const estimateTimeText =
    estimateTimeRaw && estimateTimeRaw.includes("T")
      ? estimateTimeRaw.slice(5, 16).replace("T", " ")
      : estimateTimeRaw;
  const fundType = String(fund.fund_type ?? "").trim();
  const winMetrics = (forecastWindow as any)?.metrics?.metrics;
  const trNum = toNumber(winMetrics?.total_return);
  const sharpeNum = toNumber(winMetrics?.sharpe);
  const mddNum = toNumber(winMetrics?.max_drawdown);
  const volNum = toNumber(winMetrics?.vol_annual);

  return (
    <AuthedLayout
      title={title}
      subtitle={fundType ? `${fundType} · 数据源 ${sourceDisplayName(source)}` : `数据源 ${sourceDisplayName(source)}`}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        {/* Hero：名称 / 代码 / 大字净值 / 日涨跌 */}
        <div
          className="fv-card fv-section"
          style={{
            border: "none",
            background: "linear-gradient(135deg, #4F46E5 0%, #7C3AED 100%)",
            color: "#fff",
            padding: isMobile ? 20 : 28,
            boxShadow: "0 12px 32px rgba(79, 70, 229, 0.28)",
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", gap: 16, flexWrap: "wrap" }}>
            <div style={{ minWidth: 0 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
                <span style={{ fontSize: 22, fontWeight: 750, color: "#fff", letterSpacing: "-0.01em" }}>
                  {fund.fund_name}
                </span>
                <span className="fv-pill fv-mono" style={{ background: "rgba(255,255,255,0.16)", color: "#fff" }}>
                  {fund.fund_code}
                </span>
                {fundType ? (
                  <span className="fv-pill" style={{ background: "rgba(255,255,255,0.16)", color: "#fff" }}>
                    {fundType}
                  </span>
                ) : null}
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 12, marginTop: 18, flexWrap: "wrap" }}>
                <span
                  className="fv-num"
                  style={{ fontSize: 42, fontWeight: 800, letterSpacing: "-0.02em", color: "#fff", lineHeight: 1 }}
                >
                  {latestNavText}
                </span>
                {dailyGrowthNum !== null ? (
                  <span
                    className="fv-pill fv-num"
                    style={{ background: "rgba(255,255,255,0.16)", color: "#fff", fontSize: 13 }}
                  >
                    {dailyGrowthNum >= 0 ? "+" : ""}
                    {dailyGrowthNum.toFixed(2)}%
                  </span>
                ) : null}
              </div>
              <div style={{ marginTop: 10, fontSize: 12.5, color: "rgba(255,255,255,0.72)" }}>
                最新净值{latestNavDate ? `（${String(latestNavDate).slice(0, 10)}）` : ""}
                {estimateNavNum !== null ? ` · 实时估值 ${estimateNavText}` : ""}
              </div>
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 8, alignItems: "flex-end" }}>
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
              <span className="fv-pill fv-pillPrimary">{sourceDisplayName(source)}</span>
            </div>
          </div>
        </div>

        {/* KPI 行 */}
        <div className="fv-kpiGrid">
          <div className="fv-kpi">
            <div className="fv-kpiLabel">最新净值</div>
            <div className="fv-kpiValue fv-num">{latestNavText}</div>
            <div className="fv-kpiFoot fv-muted">{latestNavDate ? String(latestNavDate).slice(0, 10) : "-"}</div>
          </div>
          <div className="fv-kpi">
            <div className="fv-kpiLabel">实时估值</div>
            <div className="fv-kpiValue fv-num">{estimateNavText}</div>
            <div className="fv-kpiFoot fv-muted">{estimateTimeText || "-"}</div>
          </div>
          <div className="fv-kpi">
            <div className="fv-kpiLabel">估算涨跌</div>
            <div
              className={`fv-kpiValue fv-num ${estimateGrowthNum === null ? "" : estimateGrowthNum >= 0 ? "fv-up" : "fv-down"}`}
            >
              {estimateGrowthNum === null ? "-" : `${estimateGrowthNum >= 0 ? "+" : ""}${estimateGrowthNum.toFixed(2)}%`}
            </div>
            <div className="fv-kpiFoot fv-muted">盘中估算</div>
          </div>
          <div className="fv-kpi">
            <div className="fv-kpiLabel">区间位置</div>
            <div className="fv-kpiValue fv-num">
              {rangePositionPct === null ? "-" : `${rangePositionPct.toFixed(0)}%`}
            </div>
            <div className="fv-kpiFoot">
              {rangePositionBucket ? (
                <span className={`fv-pill ${rangePositionBucket === "low" ? "fv-pillDown" : rangePositionBucket === "high" ? "fv-pillUp" : "fv-pillNeutral"}`}>
                  {rangePositionLabel}
                </span>
              ) : (
                <span className="fv-muted">-</span>
              )}
            </div>
          </div>
          {winMetrics ? (
            <div className="fv-kpi">
              <div className="fv-kpiLabel">区间收益</div>
              <div className={`fv-kpiValue fv-num ${trNum === null ? "" : trNum >= 0 ? "fv-up" : "fv-down"}`}>
                {trNum === null ? "-" : `${trNum >= 0 ? "+" : ""}${(trNum * 100).toFixed(2)}%`}
              </div>
              <div className="fv-kpiFoot fv-muted">分析窗口</div>
            </div>
          ) : null}
          {winMetrics ? (
            <div className="fv-kpi">
              <div className="fv-kpiLabel">夏普比率</div>
              <div className="fv-kpiValue fv-num">{sharpeNum === null ? "-" : sharpeNum.toFixed(2)}</div>
              <div className="fv-kpiFoot fv-muted">风险调整收益</div>
            </div>
          ) : null}
          {winMetrics ? (
            <div className="fv-kpi">
              <div className="fv-kpiLabel">最大回撤</div>
              <div className={`fv-kpiValue fv-num ${mddNum === null ? "" : mddNum >= 0 ? "fv-up" : "fv-down"}`}>
                {mddNum === null ? "-" : `${(mddNum * 100).toFixed(2)}%`}
              </div>
              <div className="fv-kpiFoot fv-muted">区间最大跌幅</div>
            </div>
          ) : null}
          {winMetrics ? (
            <div className="fv-kpi">
              <div className="fv-kpiLabel">年化波动</div>
              <div className="fv-kpiValue fv-num">{volNum === null ? "-" : `${(volNum * 100).toFixed(2)}%`}</div>
              <div className="fv-kpiFoot fv-muted">年化标准差</div>
            </div>
          ) : null}
        </div>

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
        <div className="fv-card fv-section">
          <div className="fv-cardHead">
            <div className="fv-cardTitle">基金分析 v2</div>
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
          </div>
          <div className="fv-cardBody">
            <Spin spinning={analysisV2Loading}>
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
                  render: (_: any, r: any) => (
                    <span className="fv-pill fv-pillPrimary">{String(r?.window ?? "-")}T</span>
                  ),
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
                      <Space wrap size={[4, 8]}>
                        <span className="fv-muted">TR</span>
                        <span className={`fv-num ${tr === null ? "" : tr >= 0 ? "fv-up" : "fv-down"}`}>
                          {tr === null ? "-" : `${tr >= 0 ? "+" : ""}${(tr * 100).toFixed(2)}%`}
                        </span>
                        <span className="fv-muted">CAGR</span>
                        <span className={`fv-num ${cagr === null ? "" : cagr >= 0 ? "fv-up" : "fv-down"}`}>
                          {cagr === null ? "-" : `${cagr >= 0 ? "+" : ""}${(cagr * 100).toFixed(2)}%`}
                        </span>
                        <span className="fv-muted">MDD</span>
                        <span className={`fv-num ${dd === null ? "" : dd >= 0 ? "fv-up" : "fv-down"}`}>
                          {dd === null ? "-" : `${(dd * 100).toFixed(2)}%`}
                        </span>
                        <span className="fv-pill fv-pillDown">
                          低点{lowStep === null ? " -" : ` f+${lowStep}`}（
                          <span className="fv-num">{lowNav === null ? "-" : lowNav.toFixed(4)}</span>）
                        </span>
                        <span className="fv-pill fv-pillUp">
                          高点{highStep === null ? " -" : ` f+${highStep}`}（
                          <span className="fv-num">{highNav === null ? "-" : highNav.toFixed(4)}</span>）
                        </span>
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
                      <Space wrap size={[4, 8]}>
                        <span className="fv-muted">S</span>
                        <span className="fv-num">{sharpe === null ? "-" : sharpe.toFixed(2)}</span>
                        <span className="fv-muted">Vol</span>
                        <span className="fv-num">{vol === null ? "-" : `${(vol * 100).toFixed(2)}%`}</span>
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
                      <Space wrap size={[4, 8]}>
                        <span className="fv-pill fv-pillNeutral">MACD {macdPts}</span>
                        <span className="fv-pill fv-pillNeutral">TS {tsActs === null ? "-" : tsActs}</span>
                        <span className="fv-pill fv-pillNeutral">Grid {gridActs}</span>
                        <span className="fv-pill fv-pillNeutral">定投 {schedActs}</span>
                      </Space>
                    );
                  },
                },
              ]}
            />
          ) : null}
            </Spin>
          </div>
        </div>
        ) : null}

        {activeTab === "signals" ? (
        <div className="fv-card fv-section">
          <div className="fv-cardHead">
            <div className="fv-cardTitle">预测信号</div>
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
          </div>
          <div className="fv-cardBody">
            <Spin spinning={signalsLoading}>
          {signalsError ? (
            <Result status="warning" title="预测信号暂不可用" subTitle={signalsError} />
          ) : bestPeerSignals ? (
            <div className="fv-kpiGrid">
              <div className="fv-kpi">
                <div className="fv-kpiLabel">位置（同类分桶）</div>
                {(() => {
                  const b = bucketLabel(bestPeerSignals?.position_bucket);
                  const p =
                    typeof bestPeerSignals?.position_percentile_0_100 === "number"
                      ? (bestPeerSignals.position_percentile_0_100 as number)
                      : null;
                  return (
                    <>
                      <div className="fv-kpiValue">
                        <span
                          className={`fv-pill ${b.color === "green" ? "fv-pillDown" : b.color === "red" ? "fv-pillUp" : "fv-pillNeutral"}`}
                        >
                          {b.text}
                        </span>
                      </div>
                      <div className="fv-kpiFoot fv-muted">
                        {p !== null ? `分位 ${p.toFixed(0)}%` : "分位 -"}
                      </div>
                    </>
                  );
                })()}
              </div>

              <div className="fv-kpi">
                <div className="fv-kpiLabel">回撤抄底概率（20T）</div>
                <div className="fv-kpiValue fv-num" style={{ color: "#4F46E5" }}>
                  {typeof bestPeerSignals?.dip_buy?.p_20t === "number"
                    ? `${((bestPeerSignals.dip_buy.p_20t as number) * 100).toFixed(1)}%`
                    : "-"}
                </div>
              </div>
              <div className="fv-kpi">
                <div className="fv-kpiLabel">回撤抄底概率（5T）</div>
                <div className="fv-kpiValue fv-num" style={{ color: "#4F46E5" }}>
                  {typeof bestPeerSignals?.dip_buy?.p_5t === "number"
                    ? `${((bestPeerSignals.dip_buy.p_5t as number) * 100).toFixed(1)}%`
                    : "-"}
                </div>
              </div>
              <div className="fv-kpi">
                <div className="fv-kpiLabel">神奇反转概率（20T）</div>
                <div className="fv-kpiValue fv-num" style={{ color: "#4F46E5" }}>
                  {typeof bestPeerSignals?.magic_rebound?.p_20t === "number"
                    ? `${((bestPeerSignals.magic_rebound.p_20t as number) * 100).toFixed(1)}%`
                    : "-"}
                </div>
              </div>
              <div className="fv-kpi">
                <div className="fv-kpiLabel">神奇反转概率（5T）</div>
                <div className="fv-kpiValue fv-num" style={{ color: "#4F46E5" }}>
                  {typeof bestPeerSignals?.magic_rebound?.p_5t === "number"
                    ? `${((bestPeerSignals.magic_rebound.p_5t as number) * 100).toFixed(1)}%`
                    : "-"}
                </div>
              </div>
            </div>
          ) : (
            <Empty description="暂无信号（需要先同步净值与关联板块缓存）" />
          )}

          <div style={{ marginTop: 12 }}>
            <Text type="secondary" style={{ fontSize: 12 }}>
              说明：信号与概率为模型输出，仅用于辅助理解当前“位置/回撤后的反弹概率”，不构成投资建议；模型会随数据源、板块同类样本与训练数据变化而变化。
            </Text>
          </div>
            </Spin>
          </div>
        </div>
        ) : null}

        {activeTab === "nav" ? (
        <div className="fv-card fv-section">
          <div className="fv-cardHead">
            <div className="fv-cardTitle">历史净值</div>
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
                  <span
                    className={`fv-pill ${rangePositionBucket === "low" ? "fv-pillDown" : rangePositionBucket === "high" ? "fv-pillUp" : "fv-pillNeutral"}`}
                  >
                    区间位置 {rangePositionPct.toFixed(0)}%（{rangePositionLabel}）
                  </span>
                ) : null}
                {!isMobile && forecastWindow?.as_of_date ? (
                  <Tag color="default">seed_as_of：{String(forecastWindow.as_of_date)}</Tag>
                ) : null}
                {!isMobile && forecastWindow?.seed_points ? (
                  <Tag color="default">seed_points：{String(forecastWindow.seed_points)}</Tag>
                ) : null}
              </Space>
            </div>
          </div>
          <div className="fv-cardBody">
          {navHistory.length > 0 ? (
            <div style={{ marginBottom: 16 }}>
              <ReactECharts
                option={buildNavChartOption(navHistory, {
                  compact: compactChart,
                  color: "#4F46E5",
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
              {
                title: "单位净值",
                dataIndex: "unit_nav",
                render: (v: any) => (v ? <span className="fv-num">{Number(v).toFixed(4)}</span> : "-"),
              },
              {
                title: "累计净值",
                dataIndex: "accumulated_nav",
                responsive: ["md"],
                render: (v: any) => (v ? <span className="fv-num">{Number(v).toFixed(4)}</span> : "-"),
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
                  return (
                    <span className={`fv-pill fv-num ${positive ? "fv-pillUp" : "fv-pillDown"}`}>{text}</span>
                  );
                },
              },
            ]}
          />
          </div>
        </div>
        ) : null}

        {activeTab === "holdings" ? (
          <>
            {positionRows.length > 0 ? (
              <div className="fv-card fv-section">
                <div className="fv-cardHead">
                  <div className="fv-cardTitle">我的持仓</div>
                </div>
                <div className="fv-cardBody">
                  <Spin spinning={positionsLoading}>
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
                      render: (v: any) => (
                        <span className="fv-num">{Number.isFinite(Number(v)) ? Number(v).toFixed(2) : "-"}</span>
                      ),
                    },
                    {
                      title: "持仓成本",
                      dataIndex: "holding_cost",
                      key: "holding_cost",
                      responsive: ["md"],
                      render: (v: any) => (
                        <span className="fv-num">
                          {Number.isFinite(Number(v)) ? `¥${Number(v).toFixed(2)}` : "-"}
                        </span>
                      ),
                    },
                    {
                      title: "市值",
                      dataIndex: "market_value",
                      key: "market_value",
                      render: (v: any) => (
                        <span className="fv-num">
                          {Number.isFinite(Number(v)) ? `¥${Number(v).toFixed(2)}` : "-"}
                        </span>
                      ),
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
                          <span
                            className={`fv-num ${positive ? "fv-up" : "fv-down"}`}
                            style={{ whiteSpace: "nowrap" }}
                          >
                            {positive ? "+" : ""}¥{pnl.toFixed(2)}
                            {rateText}
                          </span>
                        );
                      },
                    },
                  ]}
                />
                  </Spin>
                </div>
              </div>
            ) : null}

            {operations.length > 0 ? (
              <div className="fv-card fv-section">
                <div className="fv-cardHead">
                  <div className="fv-cardTitle">操作记录</div>
                </div>
                <div className="fv-cardBody">
                  <Spin spinning={operationsLoading}>
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
                      render: (v: any) =>
                        v === "BUY" ? (
                          <span className="fv-pill fv-pillDown">买入</span>
                        ) : v === "SELL" ? (
                          <span className="fv-pill fv-pillUp">卖出</span>
                        ) : (
                          String(v ?? "-")
                        ),
                    },
                    {
                      title: "金额",
                      dataIndex: "amount",
                      width: 140,
                      render: (v: any) => (
                        <span className="fv-num">{v ? `¥${Number(v).toFixed(2)}` : "-"}</span>
                      ),
                    },
                    {
                      title: "份额",
                      dataIndex: "share",
                      width: 140,
                      responsive: ["md"],
                      render: (v: any) => (
                        <span className="fv-num">{v ? Number(v).toFixed(4) : "-"}</span>
                      ),
                    },
                    {
                      title: "净值",
                      dataIndex: "nav",
                      width: 120,
                      responsive: ["md"],
                      render: (v: any) => (
                        <span className="fv-num">{v ? Number(v).toFixed(4) : "-"}</span>
                      ),
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
                  </Spin>
                </div>
              </div>
            ) : null}

            {positionRows.length === 0 && operations.length === 0 ? <Empty description="暂无持仓/操作记录" /> : null}
          </>
        ) : null}
      </div>
    </AuthedLayout>
  );
}

