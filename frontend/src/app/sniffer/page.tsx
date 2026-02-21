"use client";

import {
  Button,
  Card,
  Col,
  Divider,
  Input,
  List,
  Result,
  Row,
  Select,
  Space,
  Spin,
  Table,
  Tag,
  Typography,
  message,
} from "antd";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { ReloadOutlined, SyncOutlined } from "@ant-design/icons";
import { AuthedLayout } from "../../components/AuthedLayout";
import { adminSnifferSync, getFundSignals, getSnifferItems, getSnifferStatus } from "../../lib/api";
import { useAuth } from "../../contexts/AuthContext";
import { buildSnifferAdvice } from "../../lib/snifferAdvice";

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

function bucketLabel(bucket: any) {
  const s = String(bucket ?? "").toLowerCase();
  if (s === "low") return { text: "偏低", color: "green" as const };
  if (s === "high") return { text: "偏高", color: "red" as const };
  return { text: "中等", color: "gold" as const };
}

function pickBestPeerSignals(signals: any | null) {
  const peers = Array.isArray(signals?.peers) ? (signals.peers as any[]) : [];
  if (!peers.length) return null;
  const bestCode = String(signals?.best_peer_code ?? "");
  const best = peers.find((p) => String(p?.peer_code ?? "") === bestCode);
  return best ?? peers[0];
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

  const [signalsLoading, setSignalsLoading] = useState(false);
  const [signalsByFund, setSignalsByFund] = useState<Record<string, any | null>>({});

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

  const advice = useMemo(() => buildSnifferAdvice(itemsResp?.items ?? []), [itemsResp]);
  const focusList = useMemo(() => advice.focus.slice(0, 10), [advice.focus]);
  const dipBuyList = useMemo(() => advice.dipBuy.slice(0, 10), [advice.dipBuy]);

  useEffect(() => {
    const codes = Array.from(
      new Set([...focusList, ...dipBuyList].map((it) => String(it.fund_code ?? "").trim()).filter(Boolean))
    ).slice(0, 20);
    if (!codes.length) {
      setSignalsByFund({});
      return;
    }

    let cancelled = false;
    const run = async () => {
      setSignalsLoading(true);
      try {
        const res = await Promise.all(
          codes.map(async (code) => {
            try {
              const r = await getFundSignals(code, { source: "tiantian" });
              return [code, r?.data ?? null] as const;
            } catch {
              return [code, null] as const;
            }
          })
        );

        if (cancelled) return;
        const next: Record<string, any | null> = {};
        for (const [code, data] of res) next[code] = data;
        setSignalsByFund(next);
      } finally {
        if (!cancelled) setSignalsLoading(false);
      }
    };

    void run();
    return () => {
      cancelled = true;
    };
  }, [focusList, dipBuyList]);

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

        <Row gutter={[16, 16]}>
          <Col xs={24} lg={16}>
            <Card
              title="筛选与结果"
              extra={
                <Space wrap>
                  <Tag color={lastRunOk === true ? "green" : lastRunOk === false ? "red" : "default"}>
                    {lastRunOk === true ? "最近运行：成功" : lastRunOk === false ? "最近运行：失败" : "最近运行：-"}
                  </Tag>
                  {lastRunError ? <Tag color="red">{lastRunError.slice(0, 30)}</Tag> : null}
                  {isAdmin ? (
                    <Button type="primary" icon={<SyncOutlined />} onClick={() => void triggerAdminSync()} loading={syncing}>
                      立即同步
                    </Button>
                  ) : null}
                </Space>
              }
            >
              <Space wrap style={{ width: "100%", justifyContent: "space-between" }}>
                <Space wrap>
                  <Select
                    allowClear
                    style={{ width: 180 }}
                    placeholder="板块"
                    value={sector ?? undefined}
                    onChange={(v) => setSector(v ? String(v) : null)}
                    options={(itemsResp?.sectors ?? []).map((s) => ({ value: s, label: s }))}
                  />
                  <Select
                    mode="multiple"
                    allowClear
                    style={{ width: 260 }}
                    placeholder="标签（多选）"
                    value={tags}
                    onChange={(v) => setTags(Array.isArray(v) ? (v as string[]).map(String) : [])}
                    options={(itemsResp?.tags ?? []).map((t) => ({ value: t, label: t }))}
                  />
                  <Input
                    placeholder="搜索：名称/代码"
                    style={{ width: 260 }}
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                  />
                </Space>
                <Space wrap>
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
                  <Button onClick={() => void load()} loading={loading}>
                    刷新
                  </Button>
                </Space>
              </Space>

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
                    pagination={{ pageSize: 50, showSizeChanger: true }}
                    scroll={{ x: "max-content", y: 640 }}
                    sticky
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
                        sorter: (a, b) =>
                          (toNumber(a.max_drawdown) ?? -Infinity) - (toNumber(b.max_drawdown) ?? -Infinity),
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
              </div>
            </Card>
          </Col>

          <Col xs={24} lg={8}>
            <Card
              title="购买建议"
              extra={
                <Space size={8}>
                  <Tag color="green">ML 信号版（试运行）</Tag>
                  {signalsLoading ? <Tag>加载信号…</Tag> : null}
                </Space>
              }
            >
              <Paragraph type="secondary" style={{ marginBottom: 12 }}>
                建议展示优先基于嗅探源（星级/回撤/涨幅）排序，同时叠加 ML 信号（位置/抄底/反转）供参考；不构成投资建议。
              </Paragraph>

              <Title level={5} style={{ marginTop: 0 }}>
                优先关注
              </Title>
              <List
                size="small"
                dataSource={focusList}
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
                      {(() => {
                        const signals = signalsByFund[it.fund_code] ?? null;
                        const best = pickBestPeerSignals(signals);
                        if (!best) return null;
                        const b = bucketLabel(best?.position_bucket);
                        const dip20 =
                          typeof best?.dip_buy?.p_20t === "number" ? (best.dip_buy.p_20t as number) * 100 : null;
                        const reb20 =
                          typeof best?.magic_rebound?.p_20t === "number"
                            ? (best.magic_rebound.p_20t as number) * 100
                            : null;
                        return (
                          <Space size={[4, 4]} wrap style={{ marginTop: 6 }}>
                            <Tag color={b.color}>{b.text}</Tag>
                            <Tag>抄底 {dip20 !== null ? dip20.toFixed(1) : "-"}%</Tag>
                            <Tag>反转 {reb20 !== null ? reb20.toFixed(1) : "-"}%</Tag>
                            <Text type="secondary" style={{ fontSize: 12 }}>
                              同类：{String(best?.peer_name ?? "-")}
                            </Text>
                          </Space>
                        );
                      })()}
                    </Space>
                  </List.Item>
                )}
              />

              <Divider style={{ margin: "12px 0" }} />

              <Title level={5} style={{ marginTop: 0 }}>
                回撤抄底候选
              </Title>
              <List
                size="small"
                dataSource={dipBuyList}
                locale={{ emptyText: "暂无候选" }}
                renderItem={(it) => (
                  <List.Item>
                    <Space direction="vertical" size={0} style={{ width: "100%" }}>
                      <Space style={{ justifyContent: "space-between", width: "100%" }}>
                        <Link href={`/funds/${encodeURIComponent(it.fund_code)}`}>{it.fund_name}</Link>
                        <Text type="secondary">{starsText(it.star_count)}</Text>
                      </Space>
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        {it.fund_code} · 最大回撤 {it.max_drawdown ?? "-"}% · 年涨幅 {it.year_growth ?? "-"}%
                      </Text>
                      {(() => {
                        const signals = signalsByFund[it.fund_code] ?? null;
                        const best = pickBestPeerSignals(signals);
                        if (!best) return null;
                        const b = bucketLabel(best?.position_bucket);
                        const dip20 =
                          typeof best?.dip_buy?.p_20t === "number" ? (best.dip_buy.p_20t as number) * 100 : null;
                        const dip5 =
                          typeof best?.dip_buy?.p_5t === "number" ? (best.dip_buy.p_5t as number) * 100 : null;
                        return (
                          <Space size={[4, 4]} wrap style={{ marginTop: 6 }}>
                            <Tag color={b.color}>{b.text}</Tag>
                            <Tag>抄底 {dip20 !== null ? dip20.toFixed(1) : "-"}%（20T）</Tag>
                            <Tag>{dip5 !== null ? dip5.toFixed(1) : "-"}%（5T）</Tag>
                            <Text type="secondary" style={{ fontSize: 12 }}>
                              同类：{String(best?.peer_name ?? "-")}
                            </Text>
                          </Space>
                        );
                      })()}
                    </Space>
                  </List.Item>
                )}
              />

              <div style={{ marginTop: 12 }}>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  风险提示：所有建议均不构成投资建议；请结合你的持有周期（自然日/交易日）与风险承受能力。
                </Text>
              </div>
            </Card>
          </Col>
        </Row>
      </Space>
    </AuthedLayout>
  );
}

