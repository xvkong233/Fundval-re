"use client";

import { Button, Card, Input, Space, Table, Typography, message } from "antd";
import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { ReloadOutlined, SearchOutlined } from "@ant-design/icons";
import { AuthedLayout } from "../../components/AuthedLayout";
import { batchEstimate, batchUpdateNav, listFunds } from "../../lib/api";
import { mergeBatchEstimate, mergeBatchNav, normalizeFundList, type Fund } from "../../lib/funds";

const { Text } = Typography;

const PAGE_SIZE = 10;

export default function FundsPage() {
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [funds, setFunds] = useState<Fund[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [search, setSearch] = useState("");
  const [lastUpdateTime, setLastUpdateTime] = useState<Date | null>(null);

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
    } catch (e) {
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
                width: 120,
                render: (_, record) => <Link href={`/funds/${encodeURIComponent(record.fund_code)}`}>查看</Link>,
              },
            ]}
          />
        </div>
      </Card>
    </AuthedLayout>
  );
}

