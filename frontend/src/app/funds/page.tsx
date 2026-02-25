"use client";

import { Button, Card, Grid, Input, Modal, Select, Space, Table, Tag, Typography, message } from "antd";
import { useCallback, useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { ReloadOutlined, SearchOutlined, StarOutlined } from "@ant-design/icons";
import { useRouter } from "next/navigation";
import { AuthedLayout } from "../../components/AuthedLayout";
import {
  addWatchlistItem,
  getTaskJobDetail,
  listFunds,
  listSources,
  listWatchlists,
  refreshPricesBatchAsync,
} from "../../lib/api";
import { normalizeFundList, type Fund } from "../../lib/funds";
import { sourceDisplayName, type SourceItem } from "../../lib/sources";
import { pickDefaultWatchlistId, type Watchlist } from "../../lib/watchlists";

const { Text } = Typography;
const { useBreakpoint } = Grid;

const PAGE_SIZE = 10;

export default function FundsPage() {
  const router = useRouter();
  const screens = useBreakpoint();
  const isMobile = !screens.md;
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [funds, setFunds] = useState<Fund[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [search, setSearch] = useState("");
  const [lastUpdateTime, setLastUpdateTime] = useState<Date | null>(null);

  const [watchlistModalOpen, setWatchlistModalOpen] = useState(false);
  const [watchlists, setWatchlists] = useState<Watchlist[]>([]);
  const [selectedWatchlistId, setSelectedWatchlistId] = useState<string | null>(null);
  const [selectedFund, setSelectedFund] = useState<Fund | null>(null);
  const [watchlistLoading, setWatchlistLoading] = useState(false);

  const [sourcesLoading, setSourcesLoading] = useState(false);
  const [sources, setSources] = useState<SourceItem[]>([]);
  const [source, setSource] = useState<string>("tiantian");

  const fundCodes = useMemo(() => funds.map((f) => f.fund_code).filter(Boolean), [funds]);

  const desktopColumns = useMemo(
    () => [
      {
        title: "代码",
        dataIndex: "fund_code",
        width: 110,
        render: (v: any, record: any) => {
          const code = String(record?.fund_code ?? v ?? "").trim();
          if (!code) return "-";
          return (
            <Link href={`/funds/${encodeURIComponent(code)}`} className="fv-mono" style={{ whiteSpace: "nowrap" }}>
              {code}
            </Link>
          );
        },
      },
      {
        title: "基金名称",
        dataIndex: "fund_name",
        ellipsis: true,
        render: (v: any, record: any) => {
          const code = String(record?.fund_code ?? "").trim();
          const name = String(v ?? "").trim();
          if (!code) return name || "-";
          return (
            <Link
              href={`/funds/${encodeURIComponent(code)}`}
              style={{
                display: "inline-block",
                maxWidth: "100%",
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
                verticalAlign: "bottom",
              }}
              title={name || code}
            >
              {name || code}
            </Link>
          );
        },
      },
      {
        title: "最新净值",
        dataIndex: "latest_nav",
        width: 150,
        render: (nav: any, record: any) => {
          if (!nav) return "-";
          const date = record.latest_nav_date;
          const dateStr = typeof date === "string" ? `(${date.slice(5)})` : "";
          const v = Number(nav);
          return (
            <span style={{ whiteSpace: "nowrap" }}>
              {Number.isFinite(v) ? v.toFixed(4) : String(nav)}
              <Text type="secondary" style={{ fontSize: 11, marginLeft: 4 }}>
                {dateStr}
              </Text>
            </span>
          );
        },
      },
      {
        title: "实时估值",
        dataIndex: "estimate_nav",
        width: 140,
        render: (nav: any, record: any) => {
          if (!nav) return "-";
          const v = Number(nav);
          const text = Number.isFinite(v) ? v.toFixed(4) : String(nav);
          const t = typeof record?.estimate_time === "string" ? record.estimate_time : "";
          const tStr = t && t.includes("T") ? `(${t.slice(5, 16).replace("T", " ")})` : "";
          return (
            <span style={{ whiteSpace: "nowrap" }}>
              {text}
              {tStr ? (
                <Text type="secondary" style={{ fontSize: 11, marginLeft: 4 }}>
                  {tStr}
                </Text>
              ) : null}
            </span>
          );
        },
      },
      {
        title: "估算涨跌(%)",
        dataIndex: "estimate_growth",
        width: 140,
        render: (g: any) => {
          if (g === undefined || g === null || g === "") return "-";
          const v = Number(g);
          const text = Number.isFinite(v) ? v.toFixed(2) : String(g);
          const positive = Number.isFinite(v) ? v >= 0 : String(g).startsWith("-");
          return (
            <span style={{ color: positive ? "#cf1322" : "#3f8600" }}>
              {Number.isFinite(v) && v >= 0 ? "+" : ""}
              {text}
            </span>
          );
        },
      },
      {
        title: "操作",
        key: "action",
        width: 160,
        render: (_: any, record: any) => (
          <Space>
            <Link href={`/funds/${encodeURIComponent(record.fund_code)}`}>查看</Link>
            <Button
              size="small"
              icon={<StarOutlined />}
              loading={watchlistLoading && selectedFund?.fund_code === record.fund_code}
              onClick={() => void openAddToWatchlist(record)}
            >
              自选
            </Button>
          </Space>
        ),
      },
    ],
    [openAddToWatchlist, selectedFund?.fund_code, watchlistLoading]
  );

  const mobileColumns = useMemo(
    () => [
      {
        title: "基金",
        key: "fund",
        render: (_: any, record: any) => {
          const code = String(record?.fund_code ?? "").trim();
          const name = String(record?.fund_name ?? "").trim();
          return (
            <div style={{ minWidth: 0 }}>
              <Link
                href={`/funds/${encodeURIComponent(code)}`}
                style={{ display: "block", maxWidth: "100%", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}
                title={name || code}
              >
                {name || code}
              </Link>
              <Text type="secondary" className="fv-mono" style={{ fontSize: 12 }}>
                {code}
              </Text>
            </div>
          );
        },
      },
      {
        title: "估值",
        key: "est",
        width: 132,
        render: (_: any, record: any) => {
          const nav = record?.estimate_nav;
          const g = record?.estimate_growth;
          const t = typeof record?.estimate_time === "string" ? record.estimate_time : "";
          const tStr = t && t.includes("T") ? t.slice(11, 16) : "";
          const navNum = Number(nav);
          const navText = nav ? (Number.isFinite(navNum) ? navNum.toFixed(4) : String(nav)) : "-";

          const gv = Number(g);
          const gText = g === undefined || g === null || g === "" ? "-" : Number.isFinite(gv) ? gv.toFixed(2) : String(g);
          const positive = Number.isFinite(gv) ? gv >= 0 : String(g ?? "").startsWith("-");

          return (
            <div style={{ whiteSpace: "nowrap" }}>
              <div>
                <Text>{navText}</Text>
                {tStr ? (
                  <Text type="secondary" style={{ fontSize: 11, marginLeft: 4 }}>
                    {tStr}
                  </Text>
                ) : null}
              </div>
              <div style={{ fontSize: 12, color: positive ? "#cf1322" : "#3f8600" }}>
                {Number.isFinite(gv) && gv >= 0 ? "+" : ""}
                {gText}%
              </div>
            </div>
          );
        },
      },
      {
        title: "",
        key: "action",
        width: 104,
        render: (_: any, record: any) => (
          <Space size={6}>
            <Button
              size="small"
              icon={<StarOutlined />}
              loading={watchlistLoading && selectedFund?.fund_code === record.fund_code}
              onClick={() => void openAddToWatchlist(record)}
            />
            <Button size="small" onClick={() => router.push(`/funds/${encodeURIComponent(record.fund_code)}`)}>
              查看
            </Button>
          </Space>
        ),
      },
    ],
    [openAddToWatchlist, router, selectedFund?.fund_code, watchlistLoading]
  );

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

  const loadFunds = async (opts?: { page?: number; search?: string }) => {
    const nextPage = opts?.page ?? page;
    const nextSearch = opts?.search ?? search;

    setLoading(true);
    try {
      const res = await listFunds({ page: nextPage, page_size: PAGE_SIZE, search: nextSearch || undefined });
      const normalized = normalizeFundList(res.data);
      setFunds(normalized.results);
      setTotal(normalized.total);
    } catch {
      message.error("加载基金列表失败");
    } finally {
      setLoading(false);
    }
  };

  const refreshEstimatesAndNavs = async (codes: string[]) => {
    if (!codes.length) return;
    setRefreshing(true);
    try {
      const r = await refreshPricesBatchAsync(codes, source);
      const taskId = String(r?.data?.task_id ?? "").trim();
      if (taskId) {
        message.success(`已入队刷新任务：${taskId}`);
        const startedAt = Date.now();
        while (Date.now() - startedAt < 30 * 60 * 1000) {
          const d = await getTaskJobDetail(taskId);
          const status = String(d?.data?.job?.status ?? "").toLowerCase();
          if (status === "done") break;
          if (status === "error") {
            const err = String(d?.data?.job?.error ?? "任务执行失败");
            throw new Error(err);
          }
          await new Promise((resolve) => window.setTimeout(resolve, 1200));
        }
      }

      await loadFunds();
      setLastUpdateTime(new Date());
      message.success("数据已刷新");
    } catch (e: any) {
      const msg = e?.response?.data?.error || e?.message || "刷新失败";
      message.error(String(msg));
    } finally {
      setRefreshing(false);
    }
  };

  useEffect(() => {
    void loadSources();
    void loadFunds({ page: 1 });
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

  // 不自动触发批量刷新：避免切页/搜索时重复入队导致上游封锁。

  const openAddToWatchlist = useCallback(async (fund: Fund) => {
    setSelectedFund(fund);
    setWatchlistLoading(true);
    try {
      const res = await listWatchlists();
      const list = Array.isArray(res.data) ? (res.data as Watchlist[]) : [];
      setWatchlists(list);

      const defaultId = pickDefaultWatchlistId(list);
      if (!defaultId) {
        message.warning("请先创建自选列表");
        router.push("/watchlists");
        return;
      }

      setSelectedWatchlistId(defaultId);
      setWatchlistModalOpen(true);
    } catch (error: any) {
      const msg = error?.response?.data?.error || "加载自选列表失败";
      message.error(msg);
    } finally {
      setWatchlistLoading(false);
    }
  }, [router]);

  const confirmAddToWatchlist = async () => {
    if (!selectedFund || !selectedWatchlistId) return;
    setWatchlistLoading(true);
    try {
      await addWatchlistItem(selectedWatchlistId, selectedFund.fund_code);
      message.success("添加成功");
      setWatchlistModalOpen(false);
    } catch (error: any) {
      const msg = error?.response?.data?.error || "添加失败";
      message.error(msg);
    } finally {
      setWatchlistLoading(false);
    }
  };

  return (
    <AuthedLayout
      title="基金"
      subtitle={lastUpdateTime ? `更新于 ${lastUpdateTime.toLocaleTimeString()}` : undefined}
    >
      <Card styles={{ body: { padding: isMobile ? 12 : 16 } }}>
        <div className="fv-toolbar">
          <div className="fv-toolbarLeft">
            <Input.Search
              allowClear
              placeholder="搜索基金代码或名称"
              enterButton={<SearchOutlined />}
              style={{ width: isMobile ? "100%" : 420 }}
              onSearch={(value) => {
                setSearch(value);
                setPage(1);
                void loadFunds({ page: 1, search: value });
              }}
            />
          </div>
          <div className="fv-toolbarRight fv-toolbarScroll">
            <Space>
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
              <Button icon={<ReloadOutlined />} loading={refreshing} onClick={() => void refreshEstimatesAndNavs(fundCodes)}>
                刷新估值/净值
              </Button>
            </Space>
          </div>
        </div>

        <div style={{ marginTop: 16 }}>
          <Table<Fund>
            rowKey={(r) => r.fund_code}
            loading={loading}
            dataSource={funds}
            pagination={{
              current: page,
              pageSize: PAGE_SIZE,
              total,
              onChange: (p) => {
                setPage(p);
                void loadFunds({ page: p });
              },
              showSizeChanger: false,
              simple: isMobile,
              showLessItems: isMobile,
            }}
            size={isMobile ? "small" : "middle"}
            columns={(isMobile ? mobileColumns : desktopColumns) as any}
          />
        </div>

        <Modal
          title={selectedFund ? `添加到自选：${selectedFund.fund_name ?? selectedFund.fund_code}` : "添加到自选"}
          open={watchlistModalOpen}
          onOk={() => void confirmAddToWatchlist()}
          confirmLoading={watchlistLoading}
          onCancel={() => setWatchlistModalOpen(false)}
          okText="添加"
          cancelText="取消"
        >
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <div>
              <Text type="secondary">选择自选列表</Text>
            </div>
            <Select
              value={selectedWatchlistId ?? undefined}
              onChange={(v) => setSelectedWatchlistId(v)}
              placeholder="请选择自选列表"
              options={watchlists.map((w) => ({ label: w.name ?? w.id, value: w.id }))}
            />
          </div>
        </Modal>
      </Card>
    </AuthedLayout>
  );
}

