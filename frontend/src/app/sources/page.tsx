"use client";

import { Button, Card, InputNumber, Result, Space, Spin, Table, Tag, Typography, message } from "antd";
import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { ReloadOutlined } from "@ant-design/icons";
import { AuthedLayout } from "../../components/AuthedLayout";
import { getSourceAccuracy, listSources, listSourcesHealth } from "../../lib/api";
import {
  formatErrorRatePercent,
  normalizeSourceAccuracy,
  normalizeSourceHealth,
  sourceDisplayName,
  type SourceHealthLike,
  type SourceItem,
} from "../../lib/sources";

const { Paragraph, Text, Title } = Typography;

type AccuracyState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ok"; avg_error_rate: number; record_count: number }
  | { status: "error"; message: string };

type HealthState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ok"; ok: boolean; latency_ms: number | null; error: string | null }
  | { status: "error"; message: string };

export default function SourcesPage() {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sources, setSources] = useState<SourceItem[]>([]);
  const [days, setDays] = useState(100);
  const [accuracyByName, setAccuracyByName] = useState<Record<string, AccuracyState>>({});
  const [healthByName, setHealthByName] = useState<Record<string, HealthState>>({});

  const canRefresh = useMemo(() => !loading && sources.length > 0, [loading, sources.length]);

  const loadSources = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await listSources();
      const list = Array.isArray(res.data) ? (res.data as SourceItem[]) : [];
      setSources(list);
      setAccuracyByName({});
      setHealthByName({});
    } catch (e: any) {
      setSources([]);
      setError(e?.response?.data?.error || "加载数据源列表失败");
    } finally {
      setLoading(false);
    }
  };

  const refreshHealth = async () => {
    if (!sources.length) return;
    setHealthByName((prev) => {
      const next: Record<string, HealthState> = { ...prev };
      for (const s of sources) {
        next[s.name] = { status: "loading" };
      }
      return next;
    });

    try {
      const res = await listSourcesHealth();
      const list = Array.isArray(res.data) ? (res.data as SourceHealthLike[]) : [];
      const next: Record<string, HealthState> = {};
      for (const item of list) {
        const name = String(item?.name ?? "");
        if (!name) continue;
        next[name] = { status: "ok", ...normalizeSourceHealth(item) };
      }
      setHealthByName((prev) => ({ ...prev, ...next }));
      message.success("健康度已刷新");
    } catch (e: any) {
      const msg = e?.response?.data?.error || "加载健康度失败";
      setHealthByName((prev) => {
        const next: Record<string, HealthState> = { ...prev };
        for (const s of sources) {
          next[s.name] = { status: "error", message: msg };
        }
        return next;
      });
    }
  };

  const loadAccuracy = async (sourceName: string, daysValue: number) => {
    setAccuracyByName((prev) => ({ ...prev, [sourceName]: { status: "loading" } }));
    try {
      const res = await getSourceAccuracy(sourceName, daysValue);
      const normalized = normalizeSourceAccuracy(res.data ?? {});
      setAccuracyByName((prev) => ({
        ...prev,
        [sourceName]: { status: "ok", ...normalized },
      }));
    } catch (e: any) {
      const msg = e?.response?.data?.error || "加载准确率失败";
      setAccuracyByName((prev) => ({ ...prev, [sourceName]: { status: "error", message: msg } }));
    }
  };

  const refreshAllAccuracy = async () => {
    if (!sources.length) return;
    const tasks = sources.map((s) => loadAccuracy(s.name, days));
    try {
      await Promise.all(tasks);
      message.success("准确率已刷新");
    } catch {
      // ignore: per-source errors are stored
    }
  };

  useEffect(() => {
    void loadSources();
  }, []);

  useEffect(() => {
    if (!sources.length) return;
    void refreshHealth();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sources]);

  useEffect(() => {
    if (!sources.length) return;
    void refreshAllAccuracy();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sources, days]);

  return (
    <AuthedLayout title="数据源">
      <Space direction="vertical" size="large" style={{ width: "100%" }}>
        <Card>
          <Title level={3} style={{ marginTop: 0 }}>
            数据源状态
          </Title>
          <Paragraph type="secondary" style={{ marginBottom: 0 }}>
            当前页展示服务端可用数据源及其整体准确率（平均误差率）。准确率按最近 N 条记录统计。
          </Paragraph>
          <Paragraph type="secondary" style={{ marginTop: 8, marginBottom: 0 }}>
            如需了解数据源与部署配置，请查看 <Link href="/server-config">服务器配置说明</Link>。
          </Paragraph>
        </Card>

        <Card>
          <Space style={{ width: "100%", justifyContent: "space-between" }} wrap>
            <Space wrap>
              <Text type="secondary">统计天数</Text>
              <InputNumber
                min={1}
                max={3650}
                value={days}
                onChange={(v) => setDays(typeof v === "number" ? v : 100)}
              />
              <Text type="secondary">默认 100</Text>
            </Space>
            <Space wrap>
              <Button icon={<ReloadOutlined />} onClick={() => void refreshHealth()} disabled={!canRefresh}>
                刷新健康度
              </Button>
              <Button icon={<ReloadOutlined />} onClick={() => void refreshAllAccuracy()} disabled={!canRefresh}>
                刷新准确率
              </Button>
            </Space>
          </Space>

          {loading ? (
            <div style={{ padding: "24px 0", display: "flex", justifyContent: "center" }}>
              <Spin />
            </div>
          ) : error ? (
            <Result status="error" title="加载失败" subTitle={error} />
          ) : (
            <Table<SourceItem>
              rowKey={(r) => r.name}
              dataSource={sources}
              pagination={false}
              columns={[
                {
                  title: "数据源",
                  dataIndex: "name",
                  width: 220,
                  render: (v: any) => (
                    <Space direction="vertical" size={0}>
                      <span>{sourceDisplayName(String(v ?? ""))}</span>
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        {String(v ?? "")}
                      </Text>
                    </Space>
                  ),
                },
                {
                  title: "健康度",
                  key: "health",
                  width: 220,
                  render: (_, record) => {
                    const state = healthByName[record.name] ?? { status: "idle" };
                    if (state.status === "loading") return <Text type="secondary">检测中...</Text>;
                    if (state.status === "error") return <Text type="danger">{state.message}</Text>;
                    if (state.status === "ok") {
                      const label = state.ok ? "正常" : "异常";
                      const latency = state.latency_ms === null ? "" : ` ${state.latency_ms}ms`;
                      const tag = (
                        <Tag color={state.ok ? "green" : "red"}>
                          {label}
                          {latency}
                        </Tag>
                      );
                      if (state.ok) return tag;
                      return (
                        <Space direction="vertical" size={2}>
                          {tag}
                          {state.error ? <Text type="danger">{state.error}</Text> : null}
                        </Space>
                      );
                    }
                    return "-";
                  },
                },
                {
                  title: "平均误差率",
                  key: "avg_error_rate",
                  width: 160,
                  render: (_, record) => {
                    const state = accuracyByName[record.name] ?? { status: "idle" };
                    if (state.status === "loading") return <Text type="secondary">加载中...</Text>;
                    if (state.status === "error") return <Text type="danger">{state.message}</Text>;
                    if (state.status === "ok") return formatErrorRatePercent(state.avg_error_rate);
                    return "-";
                  },
                },
                {
                  title: "统计记录数",
                  key: "record_count",
                  width: 160,
                  render: (_, record) => {
                    const state = accuracyByName[record.name] ?? { status: "idle" };
                    if (state.status === "loading") return <Text type="secondary">加载中...</Text>;
                    if (state.status === "error") return "-";
                    if (state.status === "ok") return state.record_count;
                    return "-";
                  },
                },
                {
                  title: "操作",
                  key: "action",
                  width: 140,
                  render: (_, record) => (
                    <Button
                      size="small"
                      onClick={() => void loadAccuracy(record.name, days)}
                      disabled={loading}
                    >
                      刷新
                    </Button>
                  ),
                },
              ]}
            />
          )}
        </Card>
      </Space>
    </AuthedLayout>
  );
}

