"use client";

import { ArrowLeftOutlined, ReloadOutlined } from "@ant-design/icons";
import { Alert, Button, Card, Collapse, Descriptions, Grid, Input, Result, Space, Table, Tag, Typography, message } from "antd";
import type { TableColumnsType } from "antd";
import { useParams, useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useState } from "react";
import { AuthedLayout } from "../../../components/AuthedLayout";
import { getTaskJobDetail, getTaskJobLogs, getTaskJobRuns } from "../../../lib/api";

const { Text } = Typography;

type TaskLogLine = {
  level: string;
  message: string;
  created_at: string;
};

function statusTag(status: string) {
  const s = String(status || "").toLowerCase();
  if (s === "ok" || s === "done") return <Tag color="green">{status}</Tag>;
  if (s === "running") return <Tag color="blue">{status}</Tag>;
  if (s === "queued") return <Tag color="gold">{status}</Tag>;
  if (s === "error") return <Tag color="red">{status}</Tag>;
  return <Tag>{status || "-"}</Tag>;
}

function levelTag(level: string) {
  const v = String(level || "").toUpperCase();
  if (v === "ERROR") return <Tag color="red">ERROR</Tag>;
  if (v === "WARN" || v === "WARNING") return <Tag color="gold">WARN</Tag>;
  if (v === "DEBUG") return <Tag>DEBUG</Tag>;
  return <Tag color="blue">INFO</Tag>;
}

function safeJsonPretty(raw: unknown): string {
  if (typeof raw !== "string") return "";
  const s = raw.trim();
  if (!s) return "";
  try {
    return JSON.stringify(JSON.parse(s), null, 2);
  } catch {
    return s;
  }
}

function extractGroup(message: string): { group: string; text: string } {
  const m = /^\[([^\]]+)\]\s*(.*)$/.exec(message);
  if (!m) return { group: "通用", text: message };
  const g = String(m[1] || "").trim();
  const rest = String(m[2] ?? "");
  if (!g) return { group: "通用", text: message };
  // fund_code 通常是 6 位数字；也允许其他分组（例如 [batch]）
  return { group: g, text: rest || message };
}

export default function TaskDetailPage({ params }: { params: { taskId: string } }) {
  const router = useRouter();
  const screens = Grid.useBreakpoint();
  const isMobile = !screens.md;

  const routeParams = useParams<{ taskId?: string }>();
  const taskId = useMemo(() => {
    const raw = (routeParams as any)?.taskId ?? (params as any)?.taskId ?? "";
    return decodeURIComponent(String(raw)).trim();
  }, [routeParams, params]);

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pollError, setPollError] = useState<string | null>(null);
  const [pollRunsError, setPollRunsError] = useState<string | null>(null);
  const [pollLogsError, setPollLogsError] = useState<string | null>(null);
  const [job, setJob] = useState<any | null>(null);
  const [lastRun, setLastRun] = useState<any | null>(null);
  const [runs, setRuns] = useState<any[]>([]);

  const [logsLoading, setLogsLoading] = useState(false);
  const [logs, setLogs] = useState<TaskLogLine[]>([]);
  const [q, setQ] = useState("");

  const isRunning = useMemo(() => {
    const s = String(job?.status ?? "").toLowerCase();
    if (s === "running" || s === "queued") return true;
    const rs = String(lastRun?.status ?? "").toLowerCase();
    return rs === "running";
  }, [job?.status, lastRun?.status]);

  const loadDetail = useCallback(async (silent?: boolean) => {
    if (!taskId) return;
    if (!silent) setLoading(true);
    if (!silent) setError(null);
    try {
      const r = await getTaskJobDetail(taskId);
      const data = r?.data ?? null;
      setJob(data?.job ?? null);
      setLastRun(data?.last_run ?? null);
      if (silent) setPollError(null);
    } catch (e: any) {
      const msg = e?.response?.data?.error || e?.response?.data?.detail || "加载任务失败";
      if (silent) {
        setPollError(msg);
      } else {
        setJob(null);
        setLastRun(null);
        setError(msg);
      }
    } finally {
      if (!silent) setLoading(false);
    }
  }, [taskId]);

  const loadRuns = useCallback(async (silent?: boolean) => {
    if (!taskId) return;
    try {
      const r = await getTaskJobRuns(taskId, { limit: 20 });
      setRuns(Array.isArray(r?.data) ? (r.data as any[]) : []);
      if (silent) setPollRunsError(null);
    } catch (e: any) {
      const msg = e?.response?.data?.error || "加载任务运行记录失败";
      if (silent) setPollRunsError(msg);
      else message.error(msg);
    }
  }, [taskId]);

  const loadLogs = useCallback(async (silent?: boolean) => {
    if (!taskId) return;
    if (!silent) setLogsLoading(true);
    try {
      const r = await getTaskJobLogs(taskId, { limit: 2000 });
      setLogs(Array.isArray(r?.data) ? (r.data as TaskLogLine[]) : []);
      if (silent) setPollLogsError(null);
    } catch (e: any) {
      const msg = e?.response?.data?.error || "加载任务日志失败";
      if (silent) setPollLogsError(msg);
      else message.error(msg);
    } finally {
      if (!silent) setLogsLoading(false);
    }
  }, [taskId]);

  const refreshAll = useCallback(async (silent?: boolean) => {
    await Promise.all([loadDetail(silent), loadRuns(silent), loadLogs(silent)]);
  }, [loadDetail, loadLogs, loadRuns]);

  useEffect(() => {
    void refreshAll();
  }, [refreshAll]);

  useEffect(() => {
    if (!taskId) return;
    const intervalMs = isRunning ? 2000 : 5000;
    const t = window.setInterval(() => {
      void refreshAll(true);
    }, intervalMs);
    return () => window.clearInterval(t);
  }, [isRunning, refreshAll, taskId]);

  const logRows = useMemo(() => {
    return logs.map((l, idx) => {
      const msg = String(l.message || "");
      const { group, text } = extractGroup(msg);
      const shown = group === "通用" ? msg : text;
      return {
        key: `${String(l.created_at || "-")}-${idx}`,
        created_at: String(l.created_at || "-"),
        level: String(l.level || "INFO"),
        group,
        message: shown,
      };
    });
  }, [logs]);

  const filteredLogRows = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return logRows;
    return logRows.filter((r) => {
      if (String(r.message || "").toLowerCase().includes(s)) return true;
      if (String(r.group || "").toLowerCase().includes(s)) return true;
      if (String(r.level || "").toLowerCase().includes(s)) return true;
      if (String(r.created_at || "").toLowerCase().includes(s)) return true;
      return false;
    });
  }, [logRows, q]);

  const logColumns: TableColumnsType<(typeof filteredLogRows)[number]> = useMemo(
    () => [
      {
        title: "时间",
        dataIndex: "created_at",
        width: 190,
        render: (v: any) => (
          <Text type="secondary" style={{ whiteSpace: "nowrap" }}>
            {String(v || "-")}
          </Text>
        ),
      },
      {
        title: "级别",
        dataIndex: "level",
        width: 90,
        render: (v: any) => levelTag(String(v || "INFO")),
      },
      {
        title: "分组",
        dataIndex: "group",
        width: 120,
        responsive: ["md"],
        render: (v: any) => <Tag>{String(v || "通用")}</Tag>,
      },
      {
        title: "消息",
        dataIndex: "message",
        render: (v: any, r: any) => (
          <div style={{ fontFamily: "var(--font-mono)", fontSize: 12, lineHeight: 1.55, whiteSpace: "pre-wrap" }}>
            {String(v ?? r?.message ?? "")}
          </div>
        ),
      },
    ],
    []
  );

  const payloadPretty = useMemo(() => safeJsonPretty(job?.payload_json), [job?.payload_json]);
  const showCards = loading || !!job;

  return (
    <AuthedLayout
      title="任务详情"
      subtitle={taskId ? `job_id = ${taskId}` : undefined}
      extra={
        <Space size={8} wrap={false}>
          <Button icon={<ArrowLeftOutlined />} onClick={() => router.push("/tasks")}>
            返回
          </Button>
          <Button icon={<ReloadOutlined />} loading={loading || logsLoading} onClick={() => void refreshAll()}>
            刷新
          </Button>
        </Space>
      }
    >
      {error ? (
        <Result
          status="error"
          title="任务详情"
          subTitle={error}
          extra={
            <Button type="primary" onClick={() => void refreshAll()}>
              重试
            </Button>
          }
        />
      ) : null}

      {!error && (pollError || pollRunsError || pollLogsError) ? (
        <Alert
          type="warning"
          showIcon
          message="自动刷新出现错误（已保留上一次成功数据）"
          description={
            <div style={{ display: "flex", gap: 12, alignItems: "center", justifyContent: "space-between" }}>
              <Text type="secondary" style={{ minWidth: 0 }} ellipsis>
                {pollError || pollRunsError || pollLogsError}
              </Text>
              <Button size="small" onClick={() => void refreshAll()}>
                立即刷新
              </Button>
            </div>
          }
          style={{ marginBottom: 12 }}
        />
      ) : null}

      <Space direction="vertical" size="middle" style={{ width: "100%" }}>
        {!loading && !error && !job ? (
          <Result
            status="info"
            title="未找到任务"
            subTitle="该任务可能已被删除或 job_id 不存在"
            extra={
              <Button type="primary" onClick={() => void refreshAll()}>
                刷新
              </Button>
            }
          />
        ) : null}
        {showCards ? (
          <>
            <Card
              title={
                <Space size={8} wrap={false}>
                  <Text strong>任务</Text>
                  {job?.status ? statusTag(String(job.status)) : null}
                  {job?.task_type ? <Tag>{String(job.task_type)}</Tag> : null}
                </Space>
              }
              styles={{ body: { padding: 12 } }}
            >
              <Descriptions size="small" column={2} bordered>
                <Descriptions.Item label="状态">{job?.status ? statusTag(String(job.status)) : "-"}</Descriptions.Item>
                <Descriptions.Item label="类型">{job?.task_type ? String(job.task_type) : "-"}</Descriptions.Item>
                <Descriptions.Item label="创建">{job?.created_at ? String(job.created_at) : "-"}</Descriptions.Item>
                <Descriptions.Item label="更新时间">{job?.updated_at ? String(job.updated_at) : "-"}</Descriptions.Item>
                <Descriptions.Item label="开始">{job?.started_at ? String(job.started_at) : "-"}</Descriptions.Item>
                <Descriptions.Item label="结束">{job?.finished_at ? String(job.finished_at) : "-"}</Descriptions.Item>
                <Descriptions.Item label="优先级">{job?.priority ?? "-"}</Descriptions.Item>
                <Descriptions.Item label="尝试">{job?.attempt ?? "-"}</Descriptions.Item>
                <Descriptions.Item label="Not Before">{job?.not_before ? String(job.not_before) : "-"}</Descriptions.Item>
                <Descriptions.Item label="错误" span={2}>
                  {job?.error ? <Text type="danger">{String(job.error)}</Text> : "-"}
                </Descriptions.Item>
              </Descriptions>

              <div style={{ marginTop: 12 }}>
                <Collapse
                  size="small"
                  items={[
                    {
                      key: "payload",
                      label: "参数（JSON）",
                      children: (
                        <pre
                          style={{
                            margin: 0,
                            padding: 12,
                            background: "rgba(15,23,42,0.04)",
                            border: "1px solid rgba(15,23,42,0.08)",
                            borderRadius: 10,
                            fontFamily: "var(--font-mono)",
                            fontSize: 12,
                            whiteSpace: "pre-wrap",
                          }}
                        >
                          {payloadPretty || "-"}
                        </pre>
                      ),
                    },
                  ]}
                />
              </div>
            </Card>

            <Card
              title={
                <Space size={8} wrap={false}>
                  <Text strong>日志</Text>
                  {lastRun?.status ? statusTag(String(lastRun.status)) : null}
                  {lastRun?.id ? (
                    <Text
                      code
                      style={{ maxWidth: 360, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
                    >
                      run_id={String(lastRun.id)}
                    </Text>
                  ) : null}
                </Space>
              }
              extra={
                <div className="fv-toolbarScroll">
                  <Space size={8} wrap={false}>
                    <Button size="small" icon={<ReloadOutlined />} loading={logsLoading} onClick={() => void loadLogs()}>
                      刷新日志
                    </Button>
                    <Button
                      size="small"
                      onClick={() => {
                        const text = filteredLogRows
                          .map((r) => `[${r.created_at}] ${String(r.level || "INFO")} ${r.group ? `[${r.group}] ` : ""}${r.message}`)
                          .join("\n");
                        void navigator.clipboard?.writeText(text);
                        message.success("已复制");
                      }}
                      disabled={!filteredLogRows.length}
                    >
                      复制
                    </Button>
                    <Input.Search
                      allowClear
                      value={q}
                      onChange={(e) => setQ(e.target.value)}
                      placeholder="搜索日志..."
                      style={{ width: isMobile ? 160 : 280 }}
                    />
                  </Space>
                </div>
              }
              styles={{ body: { padding: 0 } }}
            >
              {loading ? (
                <div style={{ padding: 16 }}>
                  <Text type="secondary">加载中...</Text>
                </div>
              ) : filteredLogRows.length ? (
                <Table
                  rowKey="key"
                  columns={logColumns as any}
                  dataSource={filteredLogRows as any}
                  pagination={{
                    pageSize: isMobile ? 20 : 50,
                    simple: isMobile,
                    showLessItems: isMobile,
                    showSizeChanger: !isMobile,
                  }}
                  size={isMobile ? "small" : "middle"}
                  scroll={isMobile ? undefined : { x: "max-content" }}
                />
              ) : (
                <Result status="info" title="暂无日志" subTitle="任务尚未开始或尚未输出日志" />
              )}
            </Card>

            <Card title="最近运行" styles={{ body: { padding: 12 } }}>
              {runs.length ? (
                <Space direction="vertical" style={{ width: "100%" }} size={8}>
                  {runs.map((r) => (
                    <div
                      key={String(r?.id ?? Math.random())}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "space-between",
                        gap: 12,
                        padding: "8px 10px",
                        border: "1px solid rgba(15,23,42,0.08)",
                        borderRadius: 10,
                        background: "rgba(255,255,255,0.65)",
                        overflow: "hidden",
                      }}
                    >
                      <Space size={8} wrap={false} style={{ minWidth: 0, flex: 1 }}>
                        {statusTag(String(r?.status ?? "-"))}
                        <Text style={{ minWidth: 0 }} ellipsis>
                          {String(r?.job_type ?? "-")}
                        </Text>
                        <Text
                          code
                          style={{ maxWidth: 360, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
                        >
                          {String(r?.id ?? "")}
                        </Text>
                      </Space>
                      <Text type="secondary" style={{ whiteSpace: "nowrap" }}>
                        {String(r?.started_at ?? "")}
                        {r?.finished_at ? ` → ${String(r.finished_at)}` : ""}
                      </Text>
                    </div>
                  ))}
                </Space>
              ) : (
                <Text type="secondary">暂无运行记录</Text>
              )}
            </Card>
          </>
        ) : null}
      </Space>
    </AuthedLayout>
  );
}
