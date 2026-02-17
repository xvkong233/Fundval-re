"use client";

import { Button, Card, Input, Modal, Select, Space, Table, Typography, message } from "antd";
import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { ReloadOutlined, SearchOutlined, StarOutlined } from "@ant-design/icons";
import { useRouter } from "next/navigation";
import { AuthedLayout } from "../../components/AuthedLayout";
import { addWatchlistItem, batchEstimate, batchUpdateNav, listFunds, listWatchlists } from "../../lib/api";
import { mergeBatchEstimate, mergeBatchNav, normalizeFundList, type Fund } from "../../lib/funds";
import { pickDefaultWatchlistId, type Watchlist } from "../../lib/watchlists";

const { Text } = Typography;

const PAGE_SIZE = 10;

export default function FundsPage() {
  const router = useRouter();
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

  const fundCodes = useMemo(() => funds.map((f) => f.fund_code).filter(Boolean), [funds]);

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
      const [estRes, navRes] = await Promise.all([batchEstimate(codes), batchUpdateNav(codes)]);
      setFunds((prev) => mergeBatchEstimate(mergeBatchNav(prev, navRes.data), estRes.data));
      setLastUpdateTime(new Date());
      message.success("数据已刷新");
    } catch {
      message.error("获取估值/净值失败");
    } finally {
      setRefreshing(false);
    }
  };

  useEffect(() => {
    void loadFunds({ page: 1 });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (fundCodes.length === 0) return;
    void refreshEstimatesAndNavs(fundCodes);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [total, page, search]);

  const openAddToWatchlist = async (fund: Fund) => {
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
  };

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
      title={
        <div style={{ display: "flex", alignItems: "baseline", gap: 12 }}>
          <span>基金</span>
          {lastUpdateTime ? (
            <Text type="secondary" style={{ fontSize: 12 }}>
              更新于 {lastUpdateTime.toLocaleTimeString()}
            </Text>
          ) : null}
        </div>
      }
    >
      <Card>
        <Space style={{ width: "100%", justifyContent: "space-between" }} wrap>
          <Input.Search
            allowClear
            placeholder="搜索基金代码或名称"
            enterButton={<SearchOutlined />}
            style={{ maxWidth: 420 }}
            onSearch={(value) => {
              setSearch(value);
              setPage(1);
              void loadFunds({ page: 1, search: value });
            }}
          />
          <Button
            icon={<ReloadOutlined />}
            loading={refreshing}
            onClick={() => void refreshEstimatesAndNavs(fundCodes)}
          >
            刷新估值/净值
          </Button>
        </Space>

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
            }}
            columns={[
              { title: "代码", dataIndex: "fund_code", width: 110 },
              { title: "基金名称", dataIndex: "fund_name", ellipsis: true },
              {
                title: "最新净值",
                dataIndex: "latest_nav",
                width: 150,
                render: (nav: any, record) => {
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
                render: (nav: any) => {
                  if (!nav) return "-";
                  const v = Number(nav);
                  return Number.isFinite(v) ? v.toFixed(4) : String(nav);
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
                render: (_, record) => (
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
            ]}
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

