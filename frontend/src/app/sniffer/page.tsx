"use client";

import {
  Badge,
  Button,
  Card,
  Col,
  Divider,
  Grid,
  Input,
  List,
  Result,
  Row,
  Select,
  Space,
  Spin,
  Statistic,
  Table,
  Tag,
  Tabs,
  Tooltip,
  Typography,
  message,
} from "antd";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { ReloadOutlined, SyncOutlined } from "@ant-design/icons";
import { AuthedLayout } from "../../components/AuthedLayout";
import {
  adminSnifferSync,
  enqueueBatchFundSignals,
  getBatchFundSignalsPage,
  getSnifferItems,
  getSnifferStatus,
} from "../../lib/api";
import { useAuth } from "../../contexts/AuthContext";
import { buildSnifferAdvice, type SnifferSignalsSummary } from "../../lib/snifferAdvice";
import { selectSnifferSignalCandidateCodes } from "../../lib/snifferSignalCandidates";
import { liteListToSignalsSummaryByFund } from "../../lib/snifferSignals";

const { Paragraph, Text, Title } = Typography;

type SnifferItem = {
  fund_code: string;
  fund_name: string;
  sector: string;
  star_count?: number | null;
  tags: string[];
  week_growth?: string | null;
  year_growth?: string | null;
  max_drawdown?: string | null;
  fund_size_text?: string | null;
};

type SnifferItemsResponse = {
  has_snapshot: boolean;
  source_url?: string | null;
  fetched_at?: string | null;
  item_count: number;
  sectors: string[];
  tags: string[];
  items: SnifferItem[];
};

type SnifferStatusResponse = {
  last_run?: any | null;
  last_snapshot?: any | null;
};

type SignalsBatchTaskCacheV1 = {
  v: 1;
  candidate_key: string;
  task_id: string;
  source: string;
  created_at_ms: number;
};

const SIGNALS_BATCH_TASK_CACHE_KEY = "fv.sniffer.signals_batch_task.v1";
const SIGNALS_BATCH_TASK_CACHE_TTL_MS = 60 * 60 * 1000;

function readSignalsBatchTaskCache(): SignalsBatchTaskCacheV1 | null {
  try {
    const raw = sessionStorage.getItem(SIGNALS_BATCH_TASK_CACHE_KEY);
    if (!raw) return null;
    const v = JSON.parse(raw) as SignalsBatchTaskCacheV1;
    if (!v || v.v !== 1) return null;
    if (!v.task_id || !v.candidate_key) return null;
    if (typeof v.created_at_ms !== "number" || !Number.isFinite(v.created_at_ms)) return null;
    return v;
  } catch {
    return null;
  }
}

function writeSignalsBatchTaskCache(v: SignalsBatchTaskCacheV1) {
  try {
    sessionStorage.setItem(SIGNALS_BATCH_TASK_CACHE_KEY, JSON.stringify(v));
  } catch {
    // ignore
  }
}

function clearSignalsBatchTaskCache() {
  try {
    sessionStorage.removeItem(SIGNALS_BATCH_TASK_CACHE_KEY);
  } catch {
    // ignore
  }
}

function toNumber(value: string | null | undefined): number | null {
  if (!value) return null;
  const n = Number.parseFloat(String(value));
  return Number.isFinite(n) ? n : null;
}

function starsText(count: number | null | undefined) {
  const n = typeof count === "number" && Number.isFinite(count) ? Math.max(0, Math.min(5, count)) : 0;
  return n ? "★".repeat(n) : "-";
}

function bucketLabel(bucket: any) {
  const s = String(bucket ?? "").toLowerCase();
  if (s === "low") return { text: "偏低", color: "green" as const };
  if (s === "high") return { text: "偏高", color: "red" as const };
  return { text: "中等", color: "gold" as const };
}

export default function SnifferPage() {
  const { user } = useAuth();
  const isAdmin = String(user?.role ?? "") === "admin";
  const screens = Grid.useBreakpoint();
  const isMobile = !screens.md;
  const isDesktop = Boolean(screens.lg);

  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [itemsResp, setItemsResp] = useState<SnifferItemsResponse | null>(null);
  const [statusResp, setStatusResp] = useState<SnifferStatusResponse | null>(null);

  const [sector, setSector] = useState<string | null>(null);
  const [tags, setTags] = useState<string[]>([]);
  const [search, setSearch] = useState("");

  const [signalsLoading, setSignalsLoading] = useState(false);
  const [signalsByFund, setSignalsByFund] = useState<Record<string, SnifferSignalsSummary | null>>({});
  const [signalsTaskId, setSignalsTaskId] = useState<string | null>(null);
  const [signalsTaskStatus, setSignalsTaskStatus] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const [itemsRes, statusRes] = await Promise.all([getSnifferItems(), getSnifferStatus()]);
      setItemsResp((itemsRes.data ?? null) as SnifferItemsResponse | null);
      setStatusResp((statusRes.data ?? null) as SnifferStatusResponse | null);
    } catch (e: any) {
      setItemsResp(null);
      setStatusResp(null);
      setError(e?.response?.data?.error || "加载嗅探数据失败");
    } finally {
      setLoading(false);
    }
  };

  const triggerAdminSync = async () => {
    setSyncing(true);
    try {
      const r = await adminSnifferSync();
      const taskId = String(r?.data?.task_id ?? "").trim();
      message.success(taskId ? `已入队（task_id=${taskId}）` : "已触发同步");
      await load();
    } catch (e: any) {
      message.error(e?.response?.data?.error || "触发同步失败（需要管理员权限）");
    } finally {
      setSyncing(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const filteredItemsByFacet = useMemo(() => {
    const base = itemsResp?.items ?? [];
    return base.filter((it) => {
      if (sector && it.sector !== sector) return false;
      if (tags.length > 0) {
        const set = new Set(it.tags ?? []);
        if (!tags.every((t) => set.has(t))) return false;
      }
      return true;
    });
  }, [itemsResp, sector, tags]);

  const filteredItems = useMemo(() => {
    const q = search.trim().toLowerCase();
    return filteredItemsByFacet.filter((it) => {
      if (q) {
        const name = String(it.fund_name ?? "").toLowerCase();
        const code = String(it.fund_code ?? "").toLowerCase();
        if (!name.includes(q) && !code.includes(q)) return false;
      }
      return true;
    });
  }, [filteredItemsByFacet, search]);

  const advice = useMemo(() => buildSnifferAdvice(filteredItems, signalsByFund), [filteredItems, signalsByFund]);
  const buyList = useMemo(() => advice.buy.slice(0, 10), [advice.buy]);
  const watchList = useMemo(() => advice.watch.slice(0, 10), [advice.watch]);
  const avoidList = useMemo(() => advice.avoid.slice(0, 10), [advice.avoid]);

  const signalCandidateCodes = useMemo(
    () => selectSnifferSignalCandidateCodes(filteredItemsByFacet, 50),
    [filteredItemsByFacet]
  );
  const signalCandidateKey = useMemo(() => [...signalCandidateCodes].sort().join(","), [signalCandidateCodes]);
  const signalCandidateCount = useMemo(() => signalCandidateCodes.length, [signalCandidateCodes]);
  const signalsLoadedCount = useMemo(() => Object.keys(signalsByFund).length, [signalsByFund]);
  const signalsNonNullCount = useMemo(
    () => Object.values(signalsByFund).filter((x) => x !== null).length,
    [signalsByFund]
  );

  useEffect(() => {
    const codes = signalCandidateCodes;
    if (!codes.length) {
      setSignalsByFund({});
      setSignalsTaskId(null);
      setSignalsTaskStatus(null);
      return;
    }

    let cancelled = false;
    const run = async () => {
      setSignalsLoading(true);
      setSignalsTaskStatus("queued");
      try {
        const source = "tiantian";
        const now = Date.now();
        const cached = readSignalsBatchTaskCache();
        const cachedOk =
          cached &&
          cached.v === 1 &&
          cached.source === source &&
          cached.candidate_key === signalCandidateKey &&
          now - cached.created_at_ms >= 0 &&
          now - cached.created_at_ms <= SIGNALS_BATCH_TASK_CACHE_TTL_MS;

        let taskId = cachedOk ? String(cached!.task_id ?? "").trim() : "";
        let resumed = Boolean(taskId);

        const enqueueNew = async () => {
          const enqueueRes = await enqueueBatchFundSignals({ fund_codes: codes, source });
          const newId = String(enqueueRes?.data?.task_id ?? "").trim();
          if (!newId) throw new Error("信号任务入队失败：缺少 task_id");
          taskId = newId;
          resumed = false;
          writeSignalsBatchTaskCache({
            v: 1,
            candidate_key: signalCandidateKey,
            task_id: taskId,
            source,
            created_at_ms: Date.now(),
          });
          if (!cancelled) {
            setSignalsTaskId(taskId);
            message.info("信号任务已入队：可在「任务队列」查看进度与日志");
          }
        };

        if (!taskId) {
          await enqueueNew();
        } else if (!cancelled) {
          setSignalsTaskId(taskId);
        }

        const pageSize = 200;
        const fetchedPages = new Set<number>();
        while (!cancelled) {
          let first: any = null;
          try {
            first = await getBatchFundSignalsPage(taskId, { page: 1, page_size: pageSize });
          } catch (e: any) {
            const status = Number(e?.response?.status ?? 0);
            const retryable = status === 404 || status === 400;
            if (resumed && retryable) {
              clearSignalsBatchTaskCache();
              fetchedPages.clear();
              await enqueueNew();
              continue;
            }
            throw e;
          }
          if (cancelled) return;

          const status = String(first?.data?.status ?? "");
          const done = Number(first?.data?.done ?? 0);
          setSignalsTaskStatus(status || null);

          const pages = Math.max(1, Math.ceil(Math.max(0, done) / pageSize));
          const all: any[] = [];

          const page1Items = Array.isArray(first?.data?.items) ? (first?.data?.items as any[]) : [];
          all.push(...page1Items);
          fetchedPages.add(1);

          for (let p = 2; p <= pages; p++) {
            if (cancelled) return;
            if (fetchedPages.has(p) && status === "done") continue;
            const r = await getBatchFundSignalsPage(taskId, { page: p, page_size: pageSize });
            const items = Array.isArray(r?.data?.items) ? (r?.data?.items as any[]) : [];
            all.push(...items);
            fetchedPages.add(p);
          }

          setSignalsByFund(liteListToSignalsSummaryByFund(all));

          if (status === "done" || status === "error") {
            break;
          }
          await new Promise((r) => setTimeout(r, 1500));
        }
      } finally {
        if (!cancelled) setSignalsLoading(false);
      }
    };

    void run();
    return () => {
      cancelled = true;
    };
  }, [signalCandidateCodes, signalCandidateKey]);

  const lastRun = statusResp?.last_run ?? null;
  const lastRunOk = lastRun ? Boolean(lastRun.ok) : null;
  const lastRunError = lastRun?.error ? String(lastRun.error) : null;

  return (
    <AuthedLayout title="嗅探">
      <Space direction="vertical" size="large" style={{ width: "100%" }}>
        <Card
          styles={{ body: { padding: 16 } }}
          title={
            <Space size={10} wrap>
              <Title level={3} style={{ margin: 0 }}>
                嗅探
              </Title>
              <Tag>自动</Tag>
              <Badge
                status={signalsLoading ? "processing" : "default"}
                text={signalsLoading ? "信号加载中" : `信号 ${signalsLoadedCount}/${signalCandidateCount}`}
              />
              {signalsNonNullCount > 0 ? <Tag color="blue">有效信号 {signalsNonNullCount}</Tag> : null}
              {signalsTaskStatus ? <Tag color="purple">任务 {signalsTaskStatus}</Tag> : null}
              {signalsTaskId ? (
                <Tag color="geekblue" style={{ maxWidth: 260 }}>
                  <span style={{ display: "inline-block", maxWidth: 240, overflow: "hidden", textOverflow: "ellipsis" }}>
                    {signalsTaskId}
                  </span>
                </Tag>
              ) : null}
            </Space>
          }
          extra={
            <Space wrap>
              <Link href="/tasks" style={{ whiteSpace: "nowrap" }}>
                任务队列
              </Link>
              <Text type="secondary" style={{ fontSize: 12 }}>
                来源：<Text code>{itemsResp?.source_url || "https://sq.deepq.tech/star/api/data"}</Text>
              </Text>
              <Button onClick={() => void load()} loading={loading}>
                刷新
              </Button>
              {isAdmin ? (
                <Button type="primary" icon={<SyncOutlined />} onClick={() => void triggerAdminSync()} loading={syncing}>
                  立即同步
                </Button>
              ) : null}
            </Space>
          }
        >
          <Paragraph type="secondary" style={{ marginBottom: 0 }}>
            系统每天 03:10（Asia/Shanghai）自动采集星标快照，并镜像同步到所有用户自选组；页面右侧为“中性”购买建议（叠加
            ML 位置/抄底/反转信号），仅供参考。
          </Paragraph>
          <Paragraph type="secondary" style={{ marginTop: 8, marginBottom: 0 }}>
            你也可以在 <Link href="/watchlists">自选</Link> 中查看同步后的分组。
          </Paragraph>
        </Card>

        <Row gutter={[12, 12]}>
          <Col xs={24} md={6}>
            <Card styles={{ body: { padding: 12 } }}>
              <Statistic title="总基金数" value={itemsResp?.item_count ?? 0} />
            </Card>
          </Col>
          <Col xs={24} md={6}>
            <Card styles={{ body: { padding: 12 } }}>
              <Statistic title="筛选后" value={filteredItems.length} />
            </Card>
          </Col>
          <Col xs={24} md={6}>
            <Card styles={{ body: { padding: 12 } }}>
              <Statistic title="板块数" value={(itemsResp?.sectors ?? []).length} />
            </Card>
          </Col>
          <Col xs={24} md={6}>
            <Card styles={{ body: { padding: 12 } }}>
              <Statistic title="标签数" value={(itemsResp?.tags ?? []).length} />
            </Card>
          </Col>
        </Row>

        <Row gutter={[16, 16]}>
          <Col xs={24} lg={17}>
            <Card
              title="筛选与结果"
              extra={
                <Space wrap>
                  <Tag color={lastRunOk === true ? "green" : lastRunOk === false ? "red" : "default"}>
                    {lastRunOk === true ? "最近运行：成功" : lastRunOk === false ? "最近运行：失败" : "最近运行：-"}
                  </Tag>
                  {lastRunError ? <Tag color="red">{lastRunError.slice(0, 30)}</Tag> : null}
                  {itemsResp?.fetched_at ? <Tag>快照：{String(itemsResp.fetched_at).slice(0, 19)}</Tag> : null}
                </Space>
              }
              styles={{ body: { padding: 12 } }}
            >
              <div className="fv-toolbar">
                <div className="fv-toolbarLeft fv-toolbarScroll">
                  <Select
                    allowClear
                    style={{ width: isMobile ? 140 : 200 }}
                    placeholder="板块"
                    value={sector ?? undefined}
                    onChange={(v) => setSector(v ? String(v) : null)}
                    options={(itemsResp?.sectors ?? []).map((s) => ({ value: s, label: s }))}
                  />
                  <Select
                    mode="multiple"
                    allowClear
                    style={{ width: isMobile ? 220 : 320 }}
                    placeholder="标签（多选）"
                    value={tags}
                    onChange={(v) => setTags(Array.isArray(v) ? (v as string[]).map(String) : [])}
                    options={(itemsResp?.tags ?? []).map((t) => ({ value: t, label: t }))}
                  />
                  <Input
                    placeholder="搜索：名称/代码"
                    style={{ width: isMobile ? 180 : 260 }}
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                    allowClear
                  />
                </div>
                <div className="fv-toolbarRight fv-toolbarScroll">
                  <Button
                    icon={<ReloadOutlined />}
                    onClick={() => {
                      setSector(null);
                      setTags([]);
                      setSearch("");
                    }}
                  >
                    清空筛选
                  </Button>
                </div>
              </div>

              <div style={{ marginTop: 12 }}>
                {loading ? (
                  <div style={{ padding: "24px 0", display: "flex", justifyContent: "center" }}>
                    <Spin />
                  </div>
                ) : error ? (
                  <Result status="error" title="加载失败" subTitle={error} />
                ) : !itemsResp?.has_snapshot ? (
                  <Result
                    status="info"
                    title="暂无嗅探快照"
                    subTitle="系统将在每天 03:10 自动采集；如你是管理员，可点击“立即同步”触发一次。"
                    extra={
                      isAdmin ? (
                        <Button type="primary" icon={<SyncOutlined />} onClick={() => void triggerAdminSync()} loading={syncing}>
                          立即同步
                        </Button>
                      ) : null
                    }
                  />
                ) : (
                   <Table<SnifferItem>
                     rowKey={(r) => r.fund_code}
                     dataSource={filteredItems}
                     size="small"
                    pagination={{
                      pageSize: isMobile ? 20 : 50,
                      showSizeChanger: true,
                      showQuickJumper: !isMobile,
                      simple: isMobile,
                      showLessItems: isMobile,
                    }}
                    scroll={{ x: "max-content" }}
                    sticky={isDesktop}
                    columns={[
                      {
                        title: "基金",
                        key: "fund",
                        width: 320,
                        render: (_, r) => (
                          <Space direction="vertical" size={0}>
                            <Link href={`/funds/${encodeURIComponent(r.fund_code)}`}>{r.fund_name}</Link>
                            <Text type="secondary" style={{ fontSize: 12 }}>
                              {r.fund_code}
                            </Text>
                          </Space>
                        ),
                        sorter: (a, b) => String(a.fund_code).localeCompare(String(b.fund_code)),
                      },
                      {
                        title: "信号（ML）",
                        key: "signals",
                        width: 320,
                        render: (_, r) => {
                          const s = signalsByFund[r.fund_code] ?? null;
                          if (!s) return <Text type="secondary">-</Text>;
                          const b = bucketLabel(s.position_bucket);
                          const dip20 = typeof s.dip_buy_p_20t === "number" ? s.dip_buy_p_20t * 100 : null;
                          const reb20 = typeof s.magic_rebound_p_20t === "number" ? s.magic_rebound_p_20t * 100 : null;
                          return (
                            <Space size={[4, 4]} wrap>
                              <Tag color={b.color}>{b.text}</Tag>
                              <Tooltip title="抄底概率（20个交易日）">
                                <Tag>抄底 {dip20 !== null ? dip20.toFixed(1) : "-"}%</Tag>
                              </Tooltip>
                              <Tooltip title="反转概率（20个交易日）">
                                <Tag>反转 {reb20 !== null ? reb20.toFixed(1) : "-"}%</Tag>
                              </Tooltip>
                              <Text type="secondary" style={{ fontSize: 12 }}>
                                {String(s.peer_name ?? "-")}
                              </Text>
                            </Space>
                          );
                        },
                      },
                      {
                        title: "板块",
                        dataIndex: "sector",
                        width: 160,
                        responsive: ["md"],
                        sorter: (a, b) => String(a.sector).localeCompare(String(b.sector)),
                      },
                      {
                        title: "星级",
                        dataIndex: "star_count",
                        width: 120,
                        responsive: ["md"],
                        render: (v: any) => <Text>{starsText(typeof v === "number" ? v : null)}</Text>,
                        sorter: (a, b) => (a.star_count ?? -1) - (b.star_count ?? -1),
                        defaultSortOrder: "descend",
                      },
                      {
                        title: "近1周涨幅",
                        dataIndex: "week_growth",
                        width: 140,
                        render: (v: any) => (v ? `${String(v)}%` : "-"),
                        sorter: (a, b) => (toNumber(a.week_growth) ?? -Infinity) - (toNumber(b.week_growth) ?? -Infinity),
                      },
                      {
                        title: "年涨幅",
                        dataIndex: "year_growth",
                        width: 140,
                        responsive: ["md"],
                        render: (v: any) => (v ? `${String(v)}%` : "-"),
                        sorter: (a, b) => (toNumber(a.year_growth) ?? -Infinity) - (toNumber(b.year_growth) ?? -Infinity),
                      },
                      {
                        title: "最大回撤",
                        dataIndex: "max_drawdown",
                        width: 140,
                        responsive: ["md"],
                        render: (v: any) => (v ? `${String(v)}%` : "-"),
                        sorter: (a, b) =>
                          (toNumber(a.max_drawdown) ?? -Infinity) - (toNumber(b.max_drawdown) ?? -Infinity),
                      },
                      {
                        title: "标签",
                        dataIndex: "tags",
                        responsive: ["lg"],
                        render: (v: any) => {
                          const list = Array.isArray(v) ? (v as string[]) : [];
                          if (!list.length) return "-";
                          return (
                            <Space size={[4, 4]} wrap>
                              {list.slice(0, 8).map((t) => (
                                <Tag key={t}>{t}</Tag>
                              ))}
                              {list.length > 8 ? <Text type="secondary">+{list.length - 8}</Text> : null}
                            </Space>
                          );
                        },
                      },
                      {
                        title: "规模",
                        dataIndex: "fund_size_text",
                        width: 180,
                        responsive: ["lg"],
                        render: (v: any) => (v ? String(v) : "-"),
                      },
                    ]}
                  />
                )}
              </div>
            </Card>
          </Col>

          <Col xs={24} lg={7}>
            <div style={isDesktop ? { position: "sticky", top: 80 } : undefined}>
              <Card
                title="购买建议"
                extra={
                  <Space size={8} wrap>
                    <Tag color="green">中性</Tag>
                    {signalsLoading ? <Tag>加载信号…</Tag> : null}
                  </Space>
                }
                styles={{
                  body: isDesktop
                    ? {
                        padding: 12,
                        maxHeight: "calc(100vh - 140px)",
                        overflowY: "auto",
                      }
                    : { padding: 12 },
                }}
              >
                <Paragraph type="secondary" style={{ marginBottom: 12 }}>
                  结合位置（20/60/20）、抄底/反转概率（20T/5T）与星级/回撤，给出“买入候选/观望/回避”的中性分桶。
                </Paragraph>

                <Tabs
                  size="small"
                  items={[
                    {
                      key: "buy",
                      label: `买入候选 (${buyList.length})`,
                      children: (
                        <List
                          size="small"
                          dataSource={buyList}
                          locale={{ emptyText: "暂无数据" }}
                          renderItem={(it) => {
                            const s = signalsByFund[it.fund_code] ?? null;
                            const b = bucketLabel(s?.position_bucket);
                            const dip20 = typeof s?.dip_buy_p_20t === "number" ? s.dip_buy_p_20t * 100 : null;
                            const dip5 = typeof s?.dip_buy_p_5t === "number" ? s.dip_buy_p_5t * 100 : null;
                            const reb20 = typeof s?.magic_rebound_p_20t === "number" ? s.magic_rebound_p_20t * 100 : null;
                            const reb5 = typeof s?.magic_rebound_p_5t === "number" ? s.magic_rebound_p_5t * 100 : null;
                            return (
                              <List.Item>
                                <Space direction="vertical" size={0} style={{ width: "100%" }}>
                                  <Space style={{ justifyContent: "space-between", width: "100%" }}>
                                    <Link href={`/funds/${encodeURIComponent(it.fund_code)}`}>{it.fund_name}</Link>
                                    <Text type="secondary">{starsText(it.star_count)}</Text>
                                  </Space>
                                  <Text type="secondary" style={{ fontSize: 12 }}>
                                    {it.fund_code} · 年涨幅 {it.year_growth ?? "-"}% · 最大回撤 {it.max_drawdown ?? "-"}%
                                  </Text>
                                  {"reasons" in (it as any) ? (
                                    <Text type="secondary" style={{ fontSize: 12, marginTop: 6 }}>
                                      {(it as any).reasons.slice(0, 2).join(" · ")}
                                    </Text>
                                  ) : null}
                                  {s ? (
                                    <Space size={[4, 4]} wrap style={{ marginTop: 6 }}>
                                      <Tag color={b.color}>{b.text}</Tag>
                                      <Tooltip title="抄底概率（20/5 个交易日）">
                                        <Tag>
                                          抄底 {dip20 !== null ? dip20.toFixed(1) : "-"}% / {dip5 !== null ? dip5.toFixed(1) : "-"}%
                                        </Tag>
                                      </Tooltip>
                                      <Tooltip title="反转概率（20/5 个交易日）">
                                        <Tag>
                                          反转 {reb20 !== null ? reb20.toFixed(1) : "-"}% / {reb5 !== null ? reb5.toFixed(1) : "-"}%
                                        </Tag>
                                      </Tooltip>
                                      <Text type="secondary" style={{ fontSize: 12 }}>
                                        同类：{String(s.peer_name ?? "-")}
                                      </Text>
                                    </Space>
                                  ) : null}
                                </Space>
                              </List.Item>
                            );
                          }}
                        />
                      ),
                    },
                    {
                      key: "watch",
                      label: `观望 (${watchList.length})`,
                      children: (
                        <List
                          size="small"
                          dataSource={watchList}
                          locale={{ emptyText: "暂无数据" }}
                          renderItem={(it) => {
                            const s = signalsByFund[it.fund_code] ?? null;
                            const b = bucketLabel(s?.position_bucket);
                            const dip20 = typeof s?.dip_buy_p_20t === "number" ? s.dip_buy_p_20t * 100 : null;
                            const dip5 = typeof s?.dip_buy_p_5t === "number" ? s.dip_buy_p_5t * 100 : null;
                            return (
                              <List.Item>
                                <Space direction="vertical" size={0} style={{ width: "100%" }}>
                                  <Space style={{ justifyContent: "space-between", width: "100%" }}>
                                    <Link href={`/funds/${encodeURIComponent(it.fund_code)}`}>{it.fund_name}</Link>
                                    <Text type="secondary">{starsText(it.star_count)}</Text>
                                  </Space>
                                  <Text type="secondary" style={{ fontSize: 12 }}>
                                    {it.fund_code} · 最大回撤 {it.max_drawdown ?? "-"}% · 年涨幅 {it.year_growth ?? "-"}%
                                  </Text>
                                  {"reasons" in (it as any) ? (
                                    <Text type="secondary" style={{ fontSize: 12, marginTop: 6 }}>
                                      {(it as any).reasons.slice(0, 2).join(" · ")}
                                    </Text>
                                  ) : null}
                                  {s ? (
                                    <Space size={[4, 4]} wrap style={{ marginTop: 6 }}>
                                      <Tag color={b.color}>{b.text}</Tag>
                                      <Tooltip title="抄底概率（20/5 个交易日）">
                                        <Tag>
                                          抄底 {dip20 !== null ? dip20.toFixed(1) : "-"}% / {dip5 !== null ? dip5.toFixed(1) : "-"}%
                                        </Tag>
                                      </Tooltip>
                                      <Text type="secondary" style={{ fontSize: 12 }}>
                                        同类：{String(s.peer_name ?? "-")}
                                      </Text>
                                    </Space>
                                  ) : null}
                                </Space>
                              </List.Item>
                            );
                          }}
                        />
                      ),
                    },
                    {
                      key: "avoid",
                      label: `回避 (${avoidList.length})`,
                      children: (
                        <List
                          size="small"
                          dataSource={avoidList}
                          locale={{ emptyText: "暂无数据" }}
                          renderItem={(it) => (
                            <List.Item>
                              <Space direction="vertical" size={0} style={{ width: "100%" }}>
                                <Space style={{ justifyContent: "space-between", width: "100%" }}>
                                  <Link href={`/funds/${encodeURIComponent(it.fund_code)}`}>{it.fund_name}</Link>
                                  <Text type="secondary">{starsText(it.star_count)}</Text>
                                </Space>
                                <Text type="secondary" style={{ fontSize: 12 }}>
                                  {it.fund_code} · 年涨幅 {it.year_growth ?? "-"}% · 最大回撤 {it.max_drawdown ?? "-"}%
                                </Text>
                                {"reasons" in (it as any) ? (
                                  <Text type="secondary" style={{ fontSize: 12, marginTop: 6 }}>
                                    {(it as any).reasons.slice(0, 2).join(" · ")}
                                  </Text>
                                ) : null}
                              </Space>
                            </List.Item>
                          )}
                        />
                      ),
                    },
                  ]}
                />

                <Divider style={{ margin: "12px 0" }} />
                <Text type="secondary" style={{ fontSize: 12 }}>
                  风险提示：所有建议均不构成投资建议；请结合你的持有周期（自然日/交易日）与风险承受能力。
                </Text>
              </Card>
            </div>
          </Col>
        </Row>
      </Space>
    </AuthedLayout>
  );
}

