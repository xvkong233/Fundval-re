"use client";

import { ReloadOutlined, ThunderboltOutlined } from "@ant-design/icons";
import { Button, Card, Grid, Result, Segmented, Space, Table, Tag, Typography, message } from "antd";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useState } from "react";
import { AuthedLayout } from "../../components/AuthedLayout";
import { getTasksOverview, trainForecastModel } from "../../lib/api";

const { Text } = Typography;
const { useBreakpoint } = Grid;

type TaskOverview = {
  crawl_queue: any[];
  task_queue: any[];
  recent_jobs?: any[];
  running: any[];
  recent: any[];
};

function statusTag(status: string) {
  const s = String(status || "").toLowerCase();
  if (s === "ok" || s === "done") return <Tag color="green">{status}</Tag>;
  if (s === "running") return <Tag color="blue">{status}</Tag>;
  if (s === "queued") return <Tag color="gold">{status}</Tag>;
  if (s === "error") return <Tag color="red">{status}</Tag>;
  return <Tag>{status || "-"}</Tag>;
}

function safeJsonParse(raw: unknown): any | null {
  if (typeof raw !== "string") return null;
  const s = raw.trim();
  if (!s) return null;
  try {
    return JSON.parse(s);
  } catch {
    return null;
  }
}

function payloadSummary(payloadJson: unknown): string {
  const v = safeJsonParse(payloadJson);
  if (!v || typeof v !== "object") return "-";
  const fundCodes = Array.isArray((v as any).fund_codes) ? (v as any).fund_codes : null;
  const source = typeof (v as any).source === "string" ? (v as any).source : null;
  const start = typeof (v as any).start_date === "string" ? (v as any).start_date : null;
  const end = typeof (v as any).end_date === "string" ? (v as any).end_date : null;
  const parts: string[] = [];
  if (Array.isArray(fundCodes)) parts.push(`fund_codes=${fundCodes.length}`);
  if (source) parts.push(`source=${source}`);
  if (start || end) parts.push(`${start || "-"}~${end || "-"}`);
  return parts.length ? parts.join(" · ") : "-";
}

export default function TasksPage() {
  const router = useRouter();
  const screens = useBreakpoint();
  const isMobile = !screens.md;
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [overview, setOverview] = useState<TaskOverview | null>(null);
  const [view, setView] = useState<"all" | "running" | "queued" | "done">("all");
  const [trainLoading, setTrainLoading] = useState(false);

  const loadOverview = async (silent?: boolean) => {
    if (!silent) setLoading(true);
    setError(null);
    try {
      const r = await getTasksOverview({ recent_limit: 20 });
      setOverview((r.data ?? null) as TaskOverview | null);
    } catch (e: any) {
      setOverview(null);
      setError(e?.response?.data?.error || "加载任务队列失败");
    } finally {
      if (!silent) setLoading(false);
    }
  };

  useEffect(() => {
    void loadOverview();
    const t = window.setInterval(() => void loadOverview(true), 2500);
    return () => window.clearInterval(t);
  }, []);

  const taskQueue: any[] = useMemo(() => (Array.isArray(overview?.task_queue) ? (overview?.task_queue as any[]) : []), [overview?.task_queue]);
  const recentJobs: any[] = useMemo(() => (Array.isArray(overview?.recent_jobs) ? (overview?.recent_jobs as any[]) : []), [overview?.recent_jobs]);

  const runningJobs = useMemo(() => taskQueue.filter((x: any) => String(x.status || "").toLowerCase() === "running"), [taskQueue]);
  const queuedJobs = useMemo(() => taskQueue.filter((x: any) => String(x.status || "").toLowerCase() === "queued"), [taskQueue]);

  const recentJobIds = useMemo(() => {
    const set = new Set<string>();
    for (const j of recentJobs) {
      const id = String(j?.id ?? "").trim();
      if (id) set.add(id);
    }
    return set;
  }, [recentJobs]);

  const allRows = useMemo(() => {
    const normalizeJob = (j: any) => ({
      ...j,
      __phase: String(j?.status || "").toLowerCase(),
    });
    return [...runningJobs.map(normalizeJob), ...queuedJobs.map(normalizeJob), ...recentJobs.map(normalizeJob)];
  }, [runningJobs, queuedJobs, recentJobs]);

  const filteredRows = useMemo(() => {
    if (view === "running") return allRows.filter((r: any) => String(r?.status || "").toLowerCase() === "running");
    if (view === "queued") return allRows.filter((r: any) => String(r?.status || "").toLowerCase() === "queued");
    if (view === "done")
      return allRows.filter((r: any) => ["ok", "error", "done"].includes(String(r?.status || "").toLowerCase()));
    return allRows;
  }, [allRows, view]);

  const desktopColumns = useMemo(
    () => [
      {
        title: "任务",
        dataIndex: "task_type",
        key: "task_type",
        width: 240,
        ellipsis: true,
        render: (v: any, record: any) => {
          const id = String(record?.id ?? "").trim();
          const href = id ? `/tasks/${encodeURIComponent(id)}` : "#";
          return id ? (
            <Link href={href} onClick={(e) => e.stopPropagation()} style={{ display: "inline-block", maxWidth: "100%" }}>
              <Text underline>{String(v ?? "-")}</Text>
            </Link>
          ) : (
            <Text>{String(v ?? "-")}</Text>
          );
        },
      },
      {
        title: "状态",
        dataIndex: "status",
        key: "status",
        width: 110,
        render: (v: any) => statusTag(String(v ?? "")),
      },
      {
        title: "参数",
        dataIndex: "payload_json",
        key: "payload_json",
        ellipsis: true,
        render: (v: any) => <Text type="secondary">{payloadSummary(v)}</Text>,
      },
      { title: "创建", dataIndex: "created_at", key: "created_at", width: 180, ellipsis: true },
      { title: "开始", dataIndex: "started_at", key: "started_at", width: 180, ellipsis: true },
      { title: "结束", dataIndex: "finished_at", key: "finished_at", width: 180, ellipsis: true },
      {
        title: "错误",
        dataIndex: "error",
        key: "error",
        ellipsis: true,
        render: (v: any) => (v ? <Text type="danger">{String(v)}</Text> : "-"),
      },
      {
        title: "",
        key: "action",
        width: 80,
        fixed: "right" as const,
        render: (_: any, record: any) => {
          const id = String(record?.id ?? "").trim();
          if (!id) return null;
          return (
            <Link href={`/tasks/${encodeURIComponent(id)}`} onClick={(e) => e.stopPropagation()}>
              查看
            </Link>
          );
        },
      },
    ],
    []
  );

  const mobileColumns = useMemo(
    () =>
      [
        {
          title: "任务",
          dataIndex: "task_type",
          key: "task_type",
          ellipsis: true,
          render: (v: any, record: any) => {
            const id = String(record?.id ?? "").trim();
            return id ? <Link href={`/tasks/${encodeURIComponent(id)}`}>{String(v ?? "-")}</Link> : String(v ?? "-");
          },
        },
        {
          title: "状态",
          dataIndex: "status",
          key: "status",
          width: 86,
          render: (v: any) => statusTag(String(v ?? "")),
        },
        {
          title: "创建",
          dataIndex: "created_at",
          key: "created_at",
          width: 120,
          ellipsis: true,
          render: (v: any) => {
            const s = String(v ?? "");
            return s && s.length >= 16 ? s.slice(5, 16).replace("T", " ") : s || "-";
          },
        },
      ] as any[],
    []
  );

  const columns = isMobile ? mobileColumns : desktopColumns;

  return (
    <AuthedLayout
      title="任务队列"
      subtitle="一次提交/一次点击触发 = 1 个任务；点击任务进入查看子步骤日志"
      extra={
        <Space size={8}>
          <Button
            icon={<ThunderboltOutlined />}
            loading={trainLoading}
            onClick={async () => {
              setTrainLoading(true);
              try {
                const r = await trainForecastModel({
                  source: "tiantian",
                  model_name: "global_ols_v1",
                  horizon: 60,
                  lag_k: 20,
                });
                const taskId = String(r?.data?.task_id ?? "");
                if (taskId) {
                  message.success(`已入队：${taskId}`);
                  router.push(`/tasks/${encodeURIComponent(taskId)}`);
                } else {
                  message.success("已入队");
                }
              } catch (e: any) {
                const msg = e?.response?.data?.error || e?.response?.data?.detail || "入队失败";
                message.error(String(msg));
              } finally {
                setTrainLoading(false);
              }
            }}
          >
            训练预测模型（全市场）
          </Button>
          <Button icon={<ReloadOutlined />} onClick={() => void loadOverview()} loading={loading}>
            刷新
          </Button>
        </Space>
      }
    >
      {error ? <Result status="error" title="任务队列" subTitle={error} /> : null}

      <Card
        styles={{ body: { padding: 12 } }}
        title={<Text strong>任务列表</Text>}
        extra={
          <Segmented
            size="small"
            value={view}
            onChange={(v) => setView(v as any)}
            options={[
              { label: `全部 (${allRows.length})`, value: "all" },
              { label: `运行中 (${runningJobs.length})`, value: "running" },
              { label: `队列中 (${queuedJobs.length})`, value: "queued" },
              { label: `已完成 (${recentJobIds.size})`, value: "done" },
            ]}
          />
        }
      >
        <Table
          rowKey={(r) => String((r as any).id)}
          size={isMobile ? "small" : "middle"}
          loading={loading}
          columns={columns as any}
          dataSource={filteredRows}
          pagination={{
            pageSize: 20,
            showSizeChanger: !isMobile,
            pageSizeOptions: [20, 50, 100],
            simple: isMobile,
            showLessItems: isMobile,
          }}
          scroll={isMobile ? { x: true } : { x: 1100 }}
          sticky
          expandable={
            isMobile
              ? {
                  expandedRowRender: (record: any) => (
                    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                      <div>
                        <Text type="secondary">参数：</Text> <Text>{payloadSummary(record?.payload_json)}</Text>
                      </div>
                      <div>
                        <Text type="secondary">开始：</Text> <Text>{String(record?.started_at ?? "-")}</Text>
                      </div>
                      <div>
                        <Text type="secondary">结束：</Text> <Text>{String(record?.finished_at ?? "-")}</Text>
                      </div>
                      {record?.error ? (
                        <div>
                          <Text type="danger">错误：</Text> <Text type="danger">{String(record?.error)}</Text>
                        </div>
                      ) : null}
                    </div>
                  ),
                }
              : undefined
          }
          onRow={(record: any) => ({
            onClick: () => {
              const id = String(record?.id ?? "").trim();
              if (!id) return;
              router.push(`/tasks/${encodeURIComponent(id)}`);
            },
            style: { cursor: "pointer" },
          })}
        />
      </Card>
    </AuthedLayout>
  );
}
