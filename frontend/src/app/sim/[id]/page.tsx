"use client";

import dynamic from "next/dynamic";
import Link from "next/link";
import { useParams } from "next/navigation";
import {
  Button,
  Card,
  Col,
  Descriptions,
  Divider,
  Empty,
  Result,
  Row,
  Space,
  Table,
  Tag,
  Typography,
  message,
} from "antd";
import type { TableColumnsType } from "antd";
import { useCallback, useEffect, useMemo, useState } from "react";
import { AuthedLayout } from "../../../components/AuthedLayout";
import {
  getSimEnvObservation,
  getSimEquity,
  listSimRuns,
  runSimBacktest,
  type SimObservation,
  type SimRunSummary,
} from "../../../lib/api";

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });
const { Text } = Typography;

function toNumber(value: any): number | null {
  if (value === null || value === undefined || value === "") return null;
  const n = Number(value);
  return Number.isFinite(n) ? n : null;
}

function money(v: any): string {
  const n = toNumber(v);
  if (n === null) return "-";
  return n.toLocaleString("zh-CN", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function pct(v: number | null): string {
  if (v === null) return "-";
  return `${(v * 100).toFixed(2)}%`;
}

function apiErrorMessage(error: any, fallback: string): string {
  const data = error?.response?.data;
  if (!data) return fallback;
  if (typeof data === "string") return data;
  if (typeof data?.error === "string") return data.error;
  if (typeof data?.detail === "string") return data.detail;
  if (typeof data?.message === "string") return data.message;
  if (typeof data === "object" && data) {
    for (const v of Object.values(data)) {
      if (typeof v === "string" && v.trim()) return v;
      if (Array.isArray(v) && v.length > 0) return String(v[0]);
    }
  }
  return fallback;
}

type EquityRow = {
  date: string;
  total_equity: number;
  cash_available: number;
  cash_frozen: number;
  cash_receivable: number;
  positions_value: number;
};

export default function SimRunDetailPage() {
  const params = useParams<{ id: string }>();
  const id = decodeURIComponent(params?.id ?? "");

  const [loading, setLoading] = useState(true);
  const [run, setRun] = useState<SimRunSummary | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [equityLoading, setEquityLoading] = useState(false);
  const [equity, setEquity] = useState<EquityRow[]>([]);

  const [obsLoading, setObsLoading] = useState(false);
  const [obs, setObs] = useState<SimObservation | null>(null);

  const loadRun = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const resp = await listSimRuns();
      const rows = (resp.data ?? []) as SimRunSummary[];
      const found = rows.find((r) => String(r.id) === id) ?? null;
      setRun(found);
      if (!found) {
        setLoadError("未找到该运行（可能不属于当前账号，或已被清理）");
      }
    } catch (e: any) {
      setLoadError(apiErrorMessage(e, "加载失败"));
    } finally {
      setLoading(false);
    }
  }, [id]);

  const loadEquity = useCallback(async () => {
    if (!id) return;
    setEquityLoading(true);
    try {
      const resp = await getSimEquity(id);
      const rows = Array.isArray(resp.data) ? resp.data : [];
      const parsed: EquityRow[] = rows
        .map((r: any) => ({
          date: String(r?.date ?? ""),
          total_equity: Number(r?.total_equity ?? 0),
          cash_available: Number(r?.cash_available ?? 0),
          cash_frozen: Number(r?.cash_frozen ?? 0),
          cash_receivable: Number(r?.cash_receivable ?? 0),
          positions_value: Number(r?.positions_value ?? 0),
        }))
        .filter((r) => r.date);
      setEquity(parsed);
    } catch (e: any) {
      message.error(apiErrorMessage(e, "加载权益曲线失败"));
    } finally {
      setEquityLoading(false);
    }
  }, [id]);

  const loadObservation = useCallback(async () => {
    if (!id) return;
    setObsLoading(true);
    try {
      const resp = await getSimEnvObservation(id);
      setObs(resp.data as SimObservation);
    } catch (e: any) {
      message.error(apiErrorMessage(e, "加载 observation 失败"));
    } finally {
      setObsLoading(false);
    }
  }, [id]);

  useEffect(() => {
    void loadRun();
  }, [loadRun]);

  useEffect(() => {
    if (!run) return;
    if (run.mode === "backtest") void loadEquity();
    if (run.mode === "env") void loadObservation();
  }, [run, loadEquity, loadObservation]);

  const pageTitle = useMemo(() => {
    if (!run) return "模拟盘详情";
    const name = String(run.name ?? "").trim();
    return name ? name : "未命名运行";
  }, [run]);

  const pageSubtitle = useMemo(() => {
    if (!run) return undefined;
    return `${run.mode} · ${run.start_date} ~ ${run.end_date}`;
  }, [run]);

  const equityOption = useMemo(() => {
    const dates = equity.map((r) => r.date);
    const total = equity.map((r) => r.total_equity);
    return {
      grid: { left: 56, right: 16, top: 24, bottom: 40 },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: dates, axisLabel: { formatter: (v: any) => String(v).slice(5) } },
      yAxis: { type: "value", axisLabel: { formatter: (v: any) => money(v) } },
      series: [{ name: "总权益", type: "line", smooth: true, data: total, symbolSize: 6 }],
    } as any;
  }, [equity]);

  const positionColumns: TableColumnsType<any> = useMemo(
    () => [
      { title: "基金", dataIndex: "fund_code", width: 110 },
      { title: "可用份额", dataIndex: "shares_available", width: 120, render: (v) => <Text type="secondary">{String(v ?? "-")}</Text> },
      { title: "冻结份额", dataIndex: "shares_frozen", width: 120, render: (v) => <Text type="secondary">{String(v ?? "-")}</Text> },
      { title: "估值净值", dataIndex: "nav", width: 120, render: (v) => <Text>{String(v ?? "-")}</Text> },
      { title: "市值", dataIndex: "value", width: 120, render: (v) => <Text>{String(v ?? "-")}</Text> },
    ],
    []
  );

  if (loading) {
    return (
      <AuthedLayout title="模拟盘详情">
        <Card loading />
      </AuthedLayout>
    );
  }

  if (!run) {
    return (
      <AuthedLayout title="模拟盘详情">
        <Result
          status="404"
          title="未找到运行"
          subTitle={loadError || "该 run 不存在或不可访问"}
          extra={
            <Space>
              <Link href="/sim">
                <Button type="primary">返回列表</Button>
              </Link>
              <Button onClick={() => void loadRun()}>重试</Button>
            </Space>
          }
        />
      </AuthedLayout>
    );
  }

  const initialCash = toNumber(run.initial_cash);
  const latestEquity = equity.length ? equity[equity.length - 1].total_equity : null;
  const backtestReturn =
    initialCash && latestEquity !== null && initialCash > 0 ? (latestEquity - initialCash) / initialCash : null;

  return (
    <AuthedLayout title={pageTitle} subtitle={pageSubtitle}>
      <Space direction="vertical" size={12} style={{ width: "100%" }}>
        <Card>
          <Row gutter={12} align="middle" justify="space-between">
            <Col>
              <Space wrap>
                <Tag color={run.mode === "env" ? "blue" : "purple"}>{run.mode}</Tag>
                <Text type="secondary">{run.id}</Text>
              </Space>
            </Col>
            <Col>
              <Space wrap>
                <Link href="/sim">
                  <Button>返回</Button>
                </Link>
                <Button onClick={() => void loadRun()}>刷新</Button>
                {run.mode === "backtest" ? (
                  <Button
                    type="primary"
                    onClick={async () => {
                      try {
                        await runSimBacktest(run.id);
                        message.success("回测已触发");
                        void loadEquity();
                      } catch (e: any) {
                        message.error(apiErrorMessage(e, "运行回测失败"));
                      }
                    }}
                  >
                    运行回测
                  </Button>
                ) : null}
                {run.mode === "env" ? (
                  <Button type="primary" onClick={() => void loadObservation()} loading={obsLoading}>
                    获取 observation
                  </Button>
                ) : null}
              </Space>
            </Col>
          </Row>
          <Divider style={{ margin: "12px 0" }} />
          <Descriptions bordered size="small" column={3}>
            <Descriptions.Item label="区间">{`${run.start_date} ~ ${run.end_date}`}</Descriptions.Item>
            <Descriptions.Item label="当前日期">{run.current_date || "-"}</Descriptions.Item>
            <Descriptions.Item label="来源">{run.source_name}</Descriptions.Item>
            <Descriptions.Item label="策略">{run.strategy || "-"}</Descriptions.Item>
            <Descriptions.Item label="初始资金">{money(run.initial_cash)}</Descriptions.Item>
            <Descriptions.Item label="可用现金">{money(run.cash_available)}</Descriptions.Item>
            <Descriptions.Item label="冻结现金">{money(run.cash_frozen)}</Descriptions.Item>
            <Descriptions.Item label="买入费率">{pct(run.buy_fee_rate)}</Descriptions.Item>
            <Descriptions.Item label="卖出费率">{pct(run.sell_fee_rate)}</Descriptions.Item>
            <Descriptions.Item label="赎回到账">{`T+${run.settlement_days}`}</Descriptions.Item>
          </Descriptions>
        </Card>

        {run.mode === "backtest" ? (
          <Card
            title="权益曲线"
            extra={
              <Space>
                {backtestReturn !== null ? (
                  <Tag color={backtestReturn >= 0 ? "red" : "green"}>区间收益：{pct(backtestReturn)}</Tag>
                ) : null}
                <Button onClick={() => void loadEquity()} loading={equityLoading}>
                  刷新曲线
                </Button>
              </Space>
            }
          >
            {equity.length ? (
              <ReactECharts option={equityOption} style={{ height: 360 }} notMerge />
            ) : (
              <Empty description="暂无权益数据（可先运行回测）" />
            )}
          </Card>
        ) : null}

        {run.mode === "env" ? (
          <Card
            title="Observation"
            extra={
              <Text type="secondary">
                说明：这是只读状态，不会推进交易日。训练请前往 <Link href="/sim">模拟盘</Link> 的“强化训练（实验）”。
              </Text>
            }
          >
            {obs ? (
              <Space direction="vertical" size={12} style={{ width: "100%" }}>
                <Descriptions bordered size="small" column={3}>
                  <Descriptions.Item label="日期">{obs.date}</Descriptions.Item>
                  <Descriptions.Item label="总权益">{obs.total_equity}</Descriptions.Item>
                  <Descriptions.Item label="现金(可用)">{obs.cash_available}</Descriptions.Item>
                  <Descriptions.Item label="现金(冻结)">{obs.cash_frozen}</Descriptions.Item>
                  <Descriptions.Item label="现金(应收)">{obs.cash_receivable}</Descriptions.Item>
                  <Descriptions.Item label="持仓数">{obs.positions?.length ?? 0}</Descriptions.Item>
                </Descriptions>
                <Card title="持仓" size="small" bodyStyle={{ padding: 0 }}>
                  <Table rowKey="fund_code" columns={positionColumns} dataSource={obs.positions ?? []} pagination={false} />
                </Card>
              </Space>
            ) : (
              <Empty description={obsLoading ? "加载中…" : "暂无数据"} />
            )}
          </Card>
        ) : null}
      </Space>
    </AuthedLayout>
  );
}
