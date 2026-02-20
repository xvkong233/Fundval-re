"use client";

import { Button, Card, Input, Result, Select, Space, Spin, Table, Tag, Typography, message } from "antd";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { ReloadOutlined, SyncOutlined } from "@ant-design/icons";
import { AuthedLayout } from "../../components/AuthedLayout";
import { adminSnifferSync, getSnifferItems, getSnifferStatus } from "../../lib/api";
import { useAuth } from "../../contexts/AuthContext";

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

function toNumber(value: string | null | undefined): number | null {
  if (!value) return null;
  const n = Number.parseFloat(String(value));
  return Number.isFinite(n) ? n : null;
}

function starsText(count: number | null | undefined) {
  const n = typeof count === "number" && Number.isFinite(count) ? Math.max(0, Math.min(5, count)) : 0;
  return n ? "★".repeat(n) : "-";
}

export default function SnifferPage() {
  const { user } = useAuth();
  const isAdmin = String(user?.role ?? "") === "admin";

  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [itemsResp, setItemsResp] = useState<SnifferItemsResponse | null>(null);
  const [statusResp, setStatusResp] = useState<SnifferStatusResponse | null>(null);

  const [sector, setSector] = useState<string | null>(null);
  const [tags, setTags] = useState<string[]>([]);
  const [search, setSearch] = useState("");

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
      await adminSnifferSync();
      message.success("已触发同步");
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

  const filteredItems = useMemo(() => {
    const base = itemsResp?.items ?? [];
    const q = search.trim().toLowerCase();
    return base.filter((it) => {
      if (sector && it.sector !== sector) return false;
      if (tags.length > 0) {
        const set = new Set(it.tags ?? []);
        if (!tags.every((t) => set.has(t))) return false;
      }
      if (q) {
        const name = String(it.fund_name ?? "").toLowerCase();
        const code = String(it.fund_code ?? "").toLowerCase();
        if (!name.includes(q) && !code.includes(q)) return false;
      }
      return true;
    });
  }, [itemsResp, sector, tags, search]);

  const lastRun = statusResp?.last_run ?? null;
  const lastRunOk = lastRun ? Boolean(lastRun.ok) : null;
  const lastRunError = lastRun?.error ? String(lastRun.error) : null;

  return (
    <AuthedLayout title="嗅探">
      <Space direction="vertical" size="large" style={{ width: "100%" }}>
        <Card>
          <Title level={3} style={{ marginTop: 0 }}>
            嗅探（自动）
          </Title>
          <Paragraph type="secondary" style={{ marginBottom: 0 }}>
            系统每天 03:10（Asia/Shanghai）自动从 DeepQ 星标数据源采集，并全量镜像同步到所有用户的自选组
            <Text code style={{ marginLeft: 8 }}>
              嗅探（自动）
            </Text>
            。
          </Paragraph>
          <Paragraph type="secondary" style={{ marginTop: 8, marginBottom: 0 }}>
            数据源：<Text code>{itemsResp?.source_url || "https://sq.deepq.tech/star/api/data"}</Text>
          </Paragraph>
          <Paragraph type="secondary" style={{ marginTop: 8, marginBottom: 0 }}>
            你也可以在 <Link href="/watchlists">自选</Link> 中查看同步后的分组。
          </Paragraph>
        </Card>

        <Card>
          <Space style={{ width: "100%", justifyContent: "space-between" }} wrap>
            <Space direction="vertical" size={0}>
              <Text type="secondary">
                最近采集：{itemsResp?.has_snapshot ? itemsResp?.fetched_at || "-" : "暂无快照"}
              </Text>
              {lastRunOk === null ? null : lastRunOk ? (
                <Text type="success">最近一次同步：成功</Text>
              ) : (
                <Text type="danger">最近一次同步：失败{lastRunError ? `（${lastRunError}）` : ""}</Text>
              )}
              <Text type="secondary">条目数：{itemsResp?.has_snapshot ? itemsResp?.item_count ?? 0 : 0}</Text>
            </Space>
            <Space wrap>
              <Button icon={<ReloadOutlined />} onClick={() => void load()} disabled={loading}>
                刷新
              </Button>
              {isAdmin ? (
                <Button
                  type="primary"
                  icon={<SyncOutlined />}
                  onClick={() => void triggerAdminSync()}
                  loading={syncing}
                  disabled={loading}
                >
                  立即同步
                </Button>
              ) : null}
            </Space>
          </Space>

          <div style={{ marginTop: 16 }}>
            <Space wrap style={{ width: "100%" }}>
              <Select
                allowClear
                placeholder="按板块过滤"
                style={{ minWidth: 220 }}
                value={sector}
                onChange={(v) => setSector(v ?? null)}
                options={(itemsResp?.sectors ?? []).map((s) => ({ value: s, label: s }))}
              />
              <Select
                mode="multiple"
                allowClear
                placeholder="按标签过滤（同时包含）"
                style={{ minWidth: 320 }}
                value={tags}
                onChange={(v) => setTags(Array.isArray(v) ? (v as string[]) : [])}
                options={(itemsResp?.tags ?? []).map((t) => ({ value: t, label: t }))}
              />
              <Input
                placeholder="搜索：名称/代码"
                style={{ minWidth: 260 }}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
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
            </Space>
          </div>

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
              pagination={{ pageSize: 50, showSizeChanger: true }}
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
                  title: "板块",
                  dataIndex: "sector",
                  width: 160,
                  sorter: (a, b) => String(a.sector).localeCompare(String(b.sector)),
                },
                {
                  title: "星级",
                  dataIndex: "star_count",
                  width: 120,
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
                  render: (v: any) => (v ? `${String(v)}%` : "-"),
                  sorter: (a, b) => (toNumber(a.year_growth) ?? -Infinity) - (toNumber(b.year_growth) ?? -Infinity),
                },
                {
                  title: "最大回撤",
                  dataIndex: "max_drawdown",
                  width: 140,
                  render: (v: any) => (v ? `${String(v)}%` : "-"),
                  sorter: (a, b) => (toNumber(a.max_drawdown) ?? -Infinity) - (toNumber(b.max_drawdown) ?? -Infinity),
                },
                {
                  title: "标签",
                  dataIndex: "tags",
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
                  render: (v: any) => (v ? String(v) : "-"),
                },
              ]}
            />
          )}
        </Card>
      </Space>
    </AuthedLayout>
  );
}

