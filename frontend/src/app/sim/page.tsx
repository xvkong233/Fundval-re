"use client";

import dynamic from "next/dynamic";
import Link from "next/link";
import {
  Button,
  Card,
  Col,
  Grid,
  DatePicker,
  Descriptions,
  Divider,
  Collapse,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
  Row,
  Select,
  Space,
  Table,
  Tabs,
  Tag,
  Typography,
  Switch,
  message,
} from "antd";
import type { TableColumnsType } from "antd";
import { useEffect, useMemo, useRef, useState } from "react";
import { DeleteOutlined, ReloadOutlined, EyeOutlined, PlayCircleOutlined } from "@ant-design/icons";
import { AuthedLayout } from "../../components/AuthedLayout";
import {
  createSimRun,
  deleteSimRun,
  listSimRuns,
  runSimBacktest,
  trainSimRunAuto,
  type SimRunSummary,
  type SimTrainRoundOut,
} from "../../lib/api";
import { useRouter } from "next/navigation";

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });
const { Title, Text, Paragraph } = Typography;

function asYmd(dateValue: any): string | null {
  if (!dateValue) return null;
  if (typeof dateValue.format === "function") return dateValue.format("YYYY-MM-DD");
  return null;
}

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

function parseFundCodes(input: string): string[] {
  const raw = String(input ?? "")
    .split(/[\s,，;；|/]+/g)
    .map((s) => s.trim())
    .filter(Boolean);
  const uniq = new Set<string>();
  for (const c of raw) uniq.add(c);
  return Array.from(uniq);
}

type CreateFormValues = {
  mode: "backtest" | "env";
  strategy?: "buy_and_hold_equal" | "auto_topk_snapshot" | "auto_topk_ts_timing";
  name?: string;
  source?: string;
  fund_codes_text: string;
  date_range?: any;
  initial_cash: number;
  buy_fee_rate?: number;
  sell_fee_rate?: number;
  settlement_days?: number;
  // auto strategies
  top_k?: number;
  rebalance_every?: number;
  // auto_topk_ts_timing
  refer_index_code?: string;
  sell_macd_point?: number | null;
  buy_macd_point?: number | null;
  sh_composite_index?: number;
  fund_position?: number;
  sell_at_top?: boolean;
  sell_num?: number;
  sell_unit?: "amount" | "fundPercent";
  profit_rate?: number;
  buy_amount_percent?: number;
};

type TrainFormValues = {
  source?: string;
  date_range?: any;
  initial_cash: number;
  buy_fee_rate?: number;
  sell_fee_rate?: number;
  settlement_days?: number;
  top_k: number;
  rounds: number;
  rebalance_every: number;
  population: number;
  elite_ratio: number;
  seed?: number;
};

type TrainRoundResult = {
  round: number;
  best_total_return: number;
  best_final_equity: number;
  best_weights: number[];
};

function weightsText(weights: number[]): string {
  const names = ["pos", "dip5", "dip20", "magic5", "magic20"];
  return names.map((n, i) => `${n}:${(weights[i] ?? 0).toFixed(3)}`).join("  ");
}

export default function SimPage() {
  const router = useRouter();
  const screens = Grid.useBreakpoint();
  const isMobile = !screens.md;

  const [loading, setLoading] = useState(false);
  const [runs, setRuns] = useState<SimRunSummary[]>([]);
  const [selectedRunIds, setSelectedRunIds] = useState<string[]>([]);

  const [createModalOpen, setCreateModalOpen] = useState(false);
  const [createForm] = Form.useForm<CreateFormValues>();
  const createMode = Form.useWatch("mode", createForm);
  const createStrategy = Form.useWatch("strategy", createForm);
  const backtestStrategy = createStrategy ?? "buy_and_hold_equal";
  const needsFundCodes = createMode === "env" || backtestStrategy === "buy_and_hold_equal";

  const [trainForm] = Form.useForm<TrainFormValues>();
  const [training, setTraining] = useState(false);
  const stopRef = useRef(false);
  const [trainResults, setTrainResults] = useState<TrainRoundResult[]>([]);
  const [bestWeightsState, setBestWeightsState] = useState<number[] | null>(null);
  const [bestReturnState, setBestReturnState] = useState<number | null>(null);
  const [trainRunId, setTrainRunId] = useState<string | null>(null);
  const [trainLog, setTrainLog] = useState<string>("");

  async function reloadRuns() {
    setLoading(true);
    try {
      const resp = await listSimRuns();
      const list = (resp.data ?? []) as SimRunSummary[];
      setRuns(list);
      const set = new Set(list.map((r) => String(r.id)));
      setSelectedRunIds((prev) => prev.filter((id) => set.has(String(id))));
    } catch (e: any) {
      message.error(apiErrorMessage(e, "加载模拟盘列表失败"));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void reloadRuns();
  }, []);

  const runColumns: TableColumnsType<SimRunSummary> = useMemo(() => {
    const actionColumn: TableColumnsType<SimRunSummary>[number] = {
      title: "操作",
      key: "action",
      width: isMobile ? 120 : 220,
      render: (_: any, r: SimRunSummary) => (
        <Space size={6} wrap={false}>
          <Link href={`/sim/${encodeURIComponent(r.id)}`}>
            <Button size="small" icon={isMobile ? <EyeOutlined /> : undefined}>
              {isMobile ? null : "详情"}
            </Button>
          </Link>
          {r.mode === "backtest" ? (
            <Popconfirm
              title="运行回测？"
              okText="运行"
              cancelText="取消"
              onConfirm={async () => {
                try {
                  await runSimBacktest(r.id);
                  message.success("回测已触发");
                } catch (e: any) {
                  message.error(apiErrorMessage(e, "运行回测失败"));
                }
              }}
            >
              <Button size="small" icon={isMobile ? <PlayCircleOutlined /> : undefined}>
                {isMobile ? null : "回测"}
              </Button>
            </Popconfirm>
          ) : null}
          <Popconfirm
            title="删除该模拟盘运行？"
            description="删除后将清理该 run 的回测/训练结果（不可恢复）"
            okText="删除"
            cancelText="取消"
            okButtonProps={{ danger: true }}
            onConfirm={async () => {
              try {
                await deleteSimRun(r.id);
                message.success("已删除");
                await reloadRuns();
              } catch (e: any) {
                message.error(apiErrorMessage(e, "删除失败"));
              }
            }}
          >
            <Button size="small" danger icon={<DeleteOutlined />}>
              {isMobile ? null : "删除"}
            </Button>
          </Popconfirm>
        </Space>
      ),
    };

    if (isMobile) {
      return [
        {
          title: "运行",
          key: "name",
          render: (_: any, r: SimRunSummary) => (
            <div style={{ minWidth: 0 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
                <Tag color={r.mode === "env" ? "blue" : r.mode === "backtest" ? "purple" : "default"}>{String(r.mode)}</Tag>
                <Link href={`/sim/${encodeURIComponent(r.id)}`} style={{ minWidth: 0 }}>
                  <Text ellipsis style={{ maxWidth: 220 }}>
                    {String(r.name || "未命名")}
                  </Text>
                </Link>
              </div>
              <Text type="secondary" style={{ fontSize: 12, whiteSpace: "nowrap" }}>
                {r.start_date} ~ {r.end_date}
              </Text>
            </div>
          ),
        },
        {
          title: "状态",
          dataIndex: "status",
          width: 110,
          render: (v: any) => <Tag>{String(v)}</Tag>,
        },
        actionColumn,
      ];
    }

    return [
      {
        title: "模式",
        dataIndex: "mode",
        width: 90,
        render: (v: any) => <Tag color={v === "env" ? "blue" : v === "backtest" ? "purple" : "default"}>{String(v)}</Tag>,
      },
      {
        title: "名称",
        dataIndex: "name",
        ellipsis: true,
        render: (v: any, r: SimRunSummary) => (
          <Space direction="vertical" size={0}>
            <Link href={`/sim/${encodeURIComponent(r.id)}`}>{String(v || "未命名")}</Link>
            <Text type="secondary" style={{ fontSize: 12 }}>
              {r.id}
            </Text>
          </Space>
        ),
      },
      {
        title: "区间",
        width: 190,
        render: (_: any, r: SimRunSummary) => (
          <Space direction="vertical" size={0}>
            <Text>{`${r.start_date} ~ ${r.end_date}`}</Text>
            <Text type="secondary" style={{ fontSize: 12 }}>
              {r.current_date ? `当前：${r.current_date}` : ""}
            </Text>
          </Space>
        ),
      },
      { title: "现金(可用)", dataIndex: "cash_available", width: 120, render: (v: any) => money(v) },
      { title: "现金(冻结)", dataIndex: "cash_frozen", width: 120, render: (v: any) => money(v) },
      {
        title: "费率/结算",
        width: 160,
        render: (_: any, r: SimRunSummary) => (
          <Text type="secondary">
            买 {pct(r.buy_fee_rate)} / 卖 {pct(r.sell_fee_rate)} / T+{r.settlement_days}
          </Text>
        ),
      },
      {
        title: "状态",
        dataIndex: "status",
        width: 110,
        render: (v: any) => <Tag>{String(v)}</Tag>,
      },
      actionColumn,
    ];
  }, [isMobile]);

  const trainChartOption = useMemo(() => {
    const xs = trainResults.map((r) => r.round);
    const ys = trainResults.map((r) => (Number.isFinite(r.best_total_return) ? r.best_total_return : 0));
    const ysBest: number[] = [];
    let best = -Infinity;
    for (const y of ys) {
      if (y > best) best = y;
      ysBest.push(best);
    }
    return {
      grid: { left: 48, right: 16, top: 20, bottom: 40 },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: xs, axisLabel: { formatter: (v: any) => String(v) } },
      yAxis: {
        type: "value",
        axisLabel: { formatter: (v: any) => `${(Number(v) * 100).toFixed(1)}%` },
        splitLine: { lineStyle: { color: "#e5e7eb" } },
      },
      series: [
        { name: "最佳收益", type: "line", smooth: true, data: ys, symbolSize: 6 },
        { name: "历史最佳", type: "line", smooth: true, data: ysBest, symbolSize: 6 },
      ],
    } as any;
  }, [trainResults]);

  const trainColumns: TableColumnsType<TrainRoundResult> = useMemo(
    () => [
      { title: "轮次", dataIndex: "round", width: 70 },
      {
        title: "最佳收益",
        dataIndex: "best_total_return",
        width: 110,
        render: (v) => <Text style={{ color: (v as number) >= 0 ? "#cf1322" : "#3f8600" }}>{pct(v as number)}</Text>,
      },
      {
        title: "期末权益",
        dataIndex: "best_final_equity",
        width: 140,
        render: (v) => <Text type="secondary">{money(v)}</Text>,
      },
      {
        title: "权重",
        render: (_, r) => <Text type="secondary" style={{ fontFamily: "var(--font-mono)" }}>{weightsText(r.best_weights)}</Text>,
      },
    ],
    []
  );

  async function submitCreate(values: CreateFormValues) {
    const strategy = values.mode === "backtest" ? (values.strategy ?? "buy_and_hold_equal") : undefined;
    const fundCodes = parseFundCodes(values.fund_codes_text);
    const submitNeedsFundCodes =
      values.mode === "env" || (values.mode === "backtest" && (strategy ?? "buy_and_hold_equal") === "buy_and_hold_equal");
    const range = values.date_range as any;
    const start = asYmd(range?.[0]);
    const end = asYmd(range?.[1]);
    if (submitNeedsFundCodes && !fundCodes.length) {
      message.error("请填写基金代码");
      return;
    }
    if (!start || !end) {
      message.error("请选择日期区间");
      return;
    }
    const initialCash = values.initial_cash;
    if (!Number.isFinite(initialCash) || initialCash <= 0) {
      message.error("请输入合法的初始资金");
      return;
    }

    try {
      const resp = await createSimRun({
        mode: values.mode,
        strategy,
        name: values.name?.trim() || undefined,
        source: values.source?.trim() || "tiantian",
        fund_codes: fundCodes,
        start_date: start,
        end_date: end,
        initial_cash: String(initialCash),
        buy_fee_rate: values.buy_fee_rate ?? 0,
        sell_fee_rate: values.sell_fee_rate ?? 0,
        settlement_days: values.settlement_days ?? 2,
        ...(strategy === "auto_topk_snapshot" || strategy === "auto_topk_ts_timing"
          ? {
              top_k: Math.max(1, Math.min(200, Math.floor(Number(values.top_k ?? 20)))),
              rebalance_every: Math.max(1, Math.min(60, Math.floor(Number(values.rebalance_every ?? 5)))),
            }
          : {}),
        ...(strategy === "auto_topk_ts_timing"
          ? {
              refer_index_code: String(values.refer_index_code ?? "1.000001").trim(),
              sell_macd_point: values.sell_macd_point ?? null,
              buy_macd_point: values.buy_macd_point ?? null,
              sh_composite_index: Number(values.sh_composite_index ?? 3000),
              fund_position: Number(values.fund_position ?? 70),
              sell_at_top: Boolean(values.sell_at_top ?? true),
              sell_num: Number(values.sell_num ?? 10),
              sell_unit: String(values.sell_unit ?? "fundPercent"),
              profit_rate: Number(values.profit_rate ?? 10),
              buy_amount_percent: Number(values.buy_amount_percent ?? 20),
            }
          : {}),
      });
      const runId = String(resp.data?.run_id ?? "");
      if (!runId) throw new Error("missing run_id");
      message.success("已创建");
      setCreateModalOpen(false);
      void reloadRuns();
    } catch (e: any) {
      message.error(apiErrorMessage(e, "创建失败"));
    }
  }

  async function startTraining(values: TrainFormValues) {
    const range = values.date_range as any;
    const startDate = asYmd(range?.[0]);
    const endDate = asYmd(range?.[1]);
    if (!startDate || !endDate) {
      message.error("请选择日期区间");
      return;
    }
    if (!Number.isFinite(values.initial_cash) || values.initial_cash <= 0) {
      message.error("请输入合法的初始资金");
      return;
    }

    const rounds = Math.max(1, Math.min(200, Math.floor(values.rounds || 1)));
    const population = Math.max(5, Math.min(200, Math.floor(values.population || 30)));
    const eliteRatio = Math.max(0.05, Math.min(0.5, Number(values.elite_ratio ?? 0.2)));
    const topK = Math.max(1, Math.min(200, Math.floor(values.top_k || 20)));
    const rebalanceEvery = Math.max(1, Math.min(60, Math.floor(values.rebalance_every || 5)));
    const seed = values.seed !== undefined && values.seed !== null ? Math.floor(Number(values.seed)) : undefined;

    stopRef.current = false;
    setTraining(true);
    setTrainResults([]);
    setTrainLog("");
    setBestWeightsState(null);
    setBestReturnState(null);
    setTrainRunId(null);

    try {
      const source = values.source?.trim() || "tiantian";
      const created = await createSimRun({
        mode: "backtest",
        strategy: "auto_topk_snapshot",
        source,
        fund_codes: [],
        start_date: startDate,
        end_date: endDate,
        initial_cash: String(values.initial_cash),
        buy_fee_rate: values.buy_fee_rate ?? 0,
        sell_fee_rate: values.sell_fee_rate ?? 0,
        settlement_days: values.settlement_days ?? 2,
        top_k: topK,
        rebalance_every: rebalanceEvery,
      });

      const runId = (created.data as any)?.run_id as string | undefined;
      if (!runId) throw new Error("创建运行失败：缺少 run_id");

      setTrainRunId(runId);
      setTrainLog(`#run ${runId}\n策略：auto_topk_snapshot  top_k=${topK}  rebalance_every=${rebalanceEvery}\n开始训练…`);

      const trainResp = await trainSimRunAuto(runId, {
        rounds,
        population,
        elite_ratio: eliteRatio,
        seed,
      });

      if (stopRef.current) {
        setTrainLog((prev) => `${prev}\n已请求停止：训练结果已忽略`.trim());
        return;
      }

      const results = (trainResp.data ?? []) as SimTrainRoundOut[];
      const mapped: TrainRoundResult[] = results.map((r) => ({
        round: r.round,
        best_total_return: r.best_total_return,
        best_final_equity: r.best_final_equity,
        best_weights: r.best_weights ?? [],
      }));

      setTrainResults(mapped);

      if (mapped.length) {
        const best = mapped.reduce((a, b) => (b.best_total_return > a.best_total_return ? b : a), mapped[0]);
        setBestReturnState(best.best_total_return);
        setBestWeightsState(best.best_weights);
        setTrainLog((prev) => `${prev}\n完成：最佳收益 ${pct(best.best_total_return)}\n权重：${weightsText(best.best_weights)}`.trim());
      } else {
        setTrainLog((prev) => `${prev}\n完成：无结果（可能缺少信号快照或净值数据）`.trim());
      }
    } catch (e: any) {
      message.error(apiErrorMessage(e, "训练失败"));
    } finally {
      setTraining(false);
    }
  }

  return (
    <AuthedLayout
      title="模拟盘"
      subtitle="回测 · 环境 step · 自动交易训练（全市场）"
    >
      <Tabs
        size={isMobile ? "small" : "middle"}
        items={[
          {
            key: "runs",
            label: "运行列表",
            children: (
              <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                <Card
                  title="运行列表"
                  extra={<Text type="secondary">点击名称进入详情页</Text>}
                >
                  <div className="fv-toolbarScroll">
                    <Space wrap={false}>
                      <Button icon={<ReloadOutlined />} onClick={() => void reloadRuns()} loading={loading}>
                        刷新
                      </Button>
                      <Button type="primary" onClick={() => setCreateModalOpen(true)}>
                        创建运行
                      </Button>
                      <Tag color={selectedRunIds.length ? "blue" : "default"}>{`已选 ${selectedRunIds.length}`}</Tag>
                      <Popconfirm
                        title={`批量删除 ${selectedRunIds.length} 条运行？`}
                        description="删除后会清理该运行的回测/训练结果（不可恢复）"
                        okText="删除"
                        cancelText="取消"
                        okButtonProps={{ danger: true }}
                        disabled={selectedRunIds.length === 0}
                        onConfirm={async () => {
                          const ids = selectedRunIds.slice();
                          if (!ids.length) return;
                          setLoading(true);
                          try {
                            await Promise.all(ids.map((id) => deleteSimRun(id)));
                            message.success(`已删除 ${ids.length} 条`);
                            setSelectedRunIds([]);
                            await reloadRuns();
                          } catch (e: any) {
                            message.error(apiErrorMessage(e, "批量删除失败"));
                          } finally {
                            setLoading(false);
                          }
                        }}
                      >
                        <Button danger icon={<DeleteOutlined />} disabled={selectedRunIds.length === 0}>
                          批量删除
                        </Button>
                      </Popconfirm>
                      <Button disabled={selectedRunIds.length === 0} onClick={() => setSelectedRunIds([])}>
                        清空选择
                      </Button>
                    </Space>
                  </div>
                </Card>

                <Card bodyStyle={{ padding: 0 }}>
                  <Table
                    rowKey="id"
                    loading={loading}
                    columns={runColumns}
                    dataSource={runs}
                    pagination={{
                      pageSize: isMobile ? 10 : 20,
                      simple: isMobile,
                      showLessItems: isMobile,
                      showSizeChanger: !isMobile,
                    }}
                    rowSelection={{
                      selectedRowKeys: selectedRunIds,
                      onChange: (keys) => setSelectedRunIds(keys.map((k) => String(k))),
                      preserveSelectedRowKeys: true,
                    }}
                    size={isMobile ? "small" : "middle"}
                    scroll={isMobile ? undefined : { x: "max-content" }}
                  />
                </Card>
              </div>
            ),
          },
          {
            key: "train",
            label: "自动交易训练（全市场）",
            children: (
              <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                <Card>
                  <Row gutter={12}>
                    <Col span={24}>
                      <Title level={5} style={{ margin: 0 }}>
                        说明
                      </Title>
                      <Paragraph type="secondary" style={{ marginTop: 8, marginBottom: 0 }}>
                        本功能在服务端执行“全市场自动交易”的策略搜索（CEM），每轮会找到一组更优的线性权重（基于预测信号快照）。
                        训练完成后会把最佳权重写回该 run，随后可直接点击“运行回测”生成权益曲线。
                      </Paragraph>
                    </Col>
                  </Row>
                </Card>

                <Card>
                  <Form<TrainFormValues>
                    form={trainForm}
                    layout="vertical"
                    initialValues={{
                      source: "tiantian",
                      initial_cash: 10000,
                      buy_fee_rate: 0,
                      sell_fee_rate: 0,
                      settlement_days: 2,
                      top_k: 20,
                      rebalance_every: 5,
                      rounds: 20,
                      population: 30,
                      elite_ratio: 0.2,
                      seed: 42,
                    }}
                    onFinish={(v) => void startTraining(v)}
                  >
                    <Row gutter={12}>
                      <Col xs={24} md={6}>
                        <Form.Item label="数据源" name="source" rules={[{ required: true, message: "请选择数据源" }]}>
                          <Select
                            options={[
                              { label: "天天基金", value: "tiantian" },
                              { label: "东方财富", value: "eastmoney" },
                            ]}
                          />
                        </Form.Item>
                      </Col>
                      <Col xs={24} md={18}>
                        <Form.Item label="日期区间" name="date_range" rules={[{ required: true, message: "请选择日期区间" }]}>
                          <DatePicker.RangePicker style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                    </Row>

                    <Row gutter={12}>
                      <Col xs={24} md={6}>
                        <Form.Item label="初始资金" name="initial_cash" rules={[{ required: true, message: "请输入初始资金" }]}>
                          <InputNumber min={1} precision={2} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                      <Col xs={24} md={6}>
                        <Form.Item label="买入费率" name="buy_fee_rate">
                          <InputNumber min={0} max={0.5} step={0.001} precision={4} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                      <Col xs={24} md={6}>
                        <Form.Item label="卖出费率" name="sell_fee_rate">
                          <InputNumber min={0} max={0.5} step={0.001} precision={4} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                      <Col xs={24} md={6}>
                        <Form.Item label="赎回到账(T+n)" name="settlement_days">
                          <InputNumber min={0} max={10} step={1} precision={0} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                    </Row>

                    <Divider style={{ margin: "12px 0" }} />

                    <Row gutter={12}>
                      <Col xs={24} md={6}>
                        <Form.Item label="Top-K" name="top_k" rules={[{ required: true, message: "请输入 Top-K" }]}>
                          <InputNumber min={1} max={200} step={1} precision={0} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                      <Col xs={24} md={6}>
                        <Form.Item label="调仓频率(交易日)" name="rebalance_every">
                          <InputNumber min={1} max={60} step={1} precision={0} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                      <Col xs={24} md={6}>
                        <Form.Item label="轮次" name="rounds" rules={[{ required: true, message: "请输入轮次" }]}>
                          <InputNumber min={1} max={200} step={1} precision={0} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                      <Col xs={24} md={6}>
                        <Form.Item label="种群" name="population" tooltip="每轮采样多少组权重">
                          <InputNumber min={5} max={200} step={1} precision={0} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                    </Row>

                    <Row gutter={12}>
                      <Col xs={24} md={6}>
                        <Form.Item label="精英比例" name="elite_ratio" tooltip="保留前多少比例用于更新分布">
                          <InputNumber min={0.05} max={0.5} step={0.05} precision={2} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                      <Col xs={24} md={6}>
                        <Form.Item label="随机种子" name="seed">
                          <InputNumber min={0} step={1} precision={0} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                    </Row>

                    <div className="fv-toolbarScroll">
                      <Space wrap={false}>
                        <Button type="primary" htmlType="submit" loading={training} disabled={training}>
                          开始训练
                        </Button>
                        <Button
                          danger
                          disabled={!training}
                          onClick={() => {
                            stopRef.current = true;
                            message.info("已请求停止（无法中断服务端训练；将忽略返回结果）");
                          }}
                        >
                          停止
                        </Button>
                        {trainRunId ? (
                          <Button
                            type="default"
                            onClick={async () => {
                              try {
                                await runSimBacktest(trainRunId);
                                message.success("回测已触发");
                                router.push(`/sim/${encodeURIComponent(trainRunId)}`);
                              } catch (e: any) {
                                message.error(apiErrorMessage(e, "运行回测失败"));
                              }
                            }}
                            disabled={training}
                          >
                            运行回测并查看
                          </Button>
                        ) : null}
                        <Button
                          onClick={() => {
                            setTrainResults([]);
                            setTrainLog("");
                            setBestWeightsState(null);
                            setBestReturnState(null);
                            setTrainRunId(null);
                          }}
                          disabled={training}
                        >
                          清空结果
                        </Button>
                        {bestReturnState !== null ? (
                          <Tag color={bestReturnState >= 0 ? "red" : "green"}>当前最佳：{pct(bestReturnState)}</Tag>
                        ) : null}
                        {bestWeightsState ? (
                          <Text type="secondary" style={{ fontSize: 12 }}>
                            {weightsText(bestWeightsState)}
                          </Text>
                        ) : null}
                      </Space>
                    </div>
                  </Form>
                </Card>

                <Row gutter={12}>
                  <Col xs={24} lg={12}>
                    <Card title="训练曲线" extra={<Text type="secondary">轮次收益 vs 最佳</Text>}>
                      <ReactECharts option={trainChartOption} style={{ height: 320 }} notMerge />
                    </Card>
                  </Col>
                  <Col xs={24} lg={12}>
                    <Card
                      title="日志"
                      extra={
                        <Button size="small" onClick={() => navigator.clipboard?.writeText(trainLog ?? "")}>
                          复制
                        </Button>
                      }
                    >
                      <pre style={{ margin: 0, whiteSpace: "pre-wrap" }}>{trainLog || "—"}</pre>
                    </Card>
                  </Col>
                </Row>

                <Card title="结果明细" bodyStyle={{ padding: 0 }}>
                  <Table
                    rowKey={(r) => String(r.round)}
                    columns={trainColumns}
                    dataSource={trainResults}
                    pagination={{
                      pageSize: isMobile ? 10 : 20,
                      simple: isMobile,
                      showLessItems: isMobile,
                      showSizeChanger: !isMobile,
                    }}
                    size={isMobile ? "small" : "middle"}
                    scroll={isMobile ? undefined : { x: "max-content" }}
                  />
                </Card>
              </div>
            ),
          },
        ]}
      />

      <Modal
        title="创建模拟盘运行"
        open={createModalOpen}
        onCancel={() => setCreateModalOpen(false)}
        onOk={() => void createForm.submit()}
        okText="创建"
        cancelText="取消"
        confirmLoading={loading}
      >
        <Form<CreateFormValues>
          form={createForm}
          layout="vertical"
          preserve={false}
          initialValues={{
            mode: "backtest",
            strategy: "buy_and_hold_equal",
            source: "tiantian",
            initial_cash: 10000,
            buy_fee_rate: 0,
            sell_fee_rate: 0,
            settlement_days: 2,
            fund_codes_text: "000001",
            top_k: 20,
            rebalance_every: 5,
            refer_index_code: "1.000001",
            buy_macd_point: 50,
            sh_composite_index: 3000,
            fund_position: 70,
            profit_rate: 10,
            sell_at_top: true,
            sell_num: 10,
            sell_unit: "fundPercent",
            buy_amount_percent: 20,
          }}
          onFinish={(v) => void submitCreate(v)}
        >
          <Row gutter={12}>
            <Col xs={24} sm={12}>
              <Form.Item label="模式" name="mode" rules={[{ required: true, message: "请选择模式" }]}>
                <Select options={[{ label: "回测(backtest)", value: "backtest" }, { label: "环境(env)", value: "env" }]} />
              </Form.Item>
            </Col>
            <Col xs={24} sm={12}>
              <Form.Item label="数据源" name="source" rules={[{ required: true, message: "请选择数据源" }]}>
                <Select
                  options={[
                    { label: "天天基金", value: "tiantian" },
                    { label: "东方财富", value: "eastmoney" },
                  ]}
                />
              </Form.Item>
            </Col>
          </Row>

          {createMode === "backtest" ? (
            <Form.Item label="策略" name="strategy" rules={[{ required: true, message: "请选择策略" }]}>
              <Select
                options={[
                  { label: "等权买入持有 (buy_and_hold_equal)", value: "buy_and_hold_equal" },
                  { label: "全市场 Top-K (auto_topk_snapshot)", value: "auto_topk_snapshot" },
                  { label: "全市场 Top-K + 参考指数择时 (auto_topk_ts_timing)", value: "auto_topk_ts_timing" },
                ]}
              />
            </Form.Item>
          ) : null}

          <Form.Item label="名称" name="name">
            <Input placeholder="可选" />
          </Form.Item>

          <Form.Item
            label={needsFundCodes ? "基金代码（逗号/空格分隔）" : "基金代码（可留空=全市场；逗号/空格分隔）"}
            name="fund_codes_text"
            rules={[{ required: needsFundCodes, message: "请输入基金代码" }]}
          >
            <Input.TextArea rows={2} placeholder={needsFundCodes ? "例如：000001, 000002" : "留空表示全市场；或填写部分基金代码作为 universe"} />
          </Form.Item>

          {createMode === "backtest" && (backtestStrategy === "auto_topk_snapshot" || backtestStrategy === "auto_topk_ts_timing") ? (
            <>
              <Divider style={{ margin: "12px 0" }}>策略参数</Divider>
              <Row gutter={12}>
                <Col xs={24} sm={12}>
                  <Form.Item label="Top-K" name="top_k" rules={[{ required: true, message: "请输入 Top-K" }]}>
                    <InputNumber min={1} max={200} step={1} precision={0} style={{ width: "100%" }} />
                  </Form.Item>
                </Col>
                <Col xs={24} sm={12}>
                  <Form.Item label="调仓频率(交易日)" name="rebalance_every" rules={[{ required: true, message: "请输入调仓频率" }]}>
                    <InputNumber min={1} max={60} step={1} precision={0} style={{ width: "100%" }} />
                  </Form.Item>
                </Col>
              </Row>
            </>
          ) : null}

          {createMode === "backtest" && backtestStrategy === "auto_topk_ts_timing" ? (
            <>
              <Row gutter={12}>
                <Col xs={24} sm={12}>
                  <Form.Item label="参考指数" name="refer_index_code" rules={[{ required: true, message: "请选择参考指数" }]}>
                    <Select
                      options={[
                        { label: "上证指数 (1.000001)", value: "1.000001" },
                        { label: "沪深300 (1.000300)", value: "1.000300" },
                        { label: "中证500 (1.000905)", value: "1.000905" },
                      ]}
                    />
                  </Form.Item>
                </Col>
                <Col xs={24} sm={12}>
                  <Form.Item label="买入 MACD 临界点(%)" name="buy_macd_point" tooltip="留空表示不使用 MACD 择时">
                    <InputNumber min={0} max={100} step={1} precision={0} style={{ width: "100%" }} placeholder="例如：50" />
                  </Form.Item>
                </Col>
              </Row>

              <Collapse
                items={[
                  {
                    key: "stopProfit",
                    label: "止盈/补仓（可选）",
                    children: (
                      <Space direction="vertical" style={{ width: "100%" }} size={12}>
                        <Paragraph type="secondary" style={{ margin: 0 }}>
                          当启用 <Text code>buy_macd_point</Text> 时：每个 BUY 信号日都会按 <Text code>buy_amount_percent</Text>{" "}
                          预算追加买入；调仓时仅卖出出榜基金（不强制全清仓）。
                          当日若触发止盈条件，则优先止盈，不再买入。
                        </Paragraph>
                        <Row gutter={12}>
                          <Col xs={24} sm={12}>
                            <Form.Item label="卖出 MACD 临界点(%)" name="sell_macd_point" tooltip="留空表示不使用 MACD 止盈择时">
                              <InputNumber min={0} max={100} step={1} precision={0} style={{ width: "100%" }} placeholder="例如：50" />
                            </Form.Item>
                          </Col>
                          <Col xs={24} sm={12}>
                            <Form.Item label="上证指数阈值" name="sh_composite_index">
                              <InputNumber min={0} step={10} precision={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                        </Row>

                        <Row gutter={12}>
                          <Col xs={24} sm={12}>
                            <Form.Item label="仓位阈值(%)" name="fund_position">
                              <InputNumber min={0} max={100} step={1} precision={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col xs={24} sm={12}>
                            <Form.Item label="收益率阈值(%)" name="profit_rate">
                              <InputNumber min={-100} max={10000} step={1} precision={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                        </Row>

                        <Row gutter={12}>
                          <Col xs={24} sm={12}>
                            <Form.Item label="最高值止盈" name="sell_at_top" valuePropName="checked">
                              <Switch />
                            </Form.Item>
                          </Col>
                          <Col xs={24} sm={12}>
                            <Form.Item label="卖出单位" name="sell_unit">
                              <Select options={[{ label: "持仓百分比(fundPercent)", value: "fundPercent" }, { label: "金额(amount)", value: "amount" }]} />
                            </Form.Item>
                          </Col>
                        </Row>

                        <Row gutter={12}>
                          <Col xs={24} sm={12}>
                            <Form.Item label="卖出数值" name="sell_num">
                              <InputNumber min={0} step={1} precision={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col xs={24} sm={12}>
                            <Form.Item label="补仓金额(%)" name="buy_amount_percent" tooltip="<=100 表示剩余现金百分比；>100 表示固定金额（元）">
                              <InputNumber min={0} step={1} precision={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                        </Row>
                      </Space>
                    ),
                  },
                ]}
              />
            </>
          ) : null}

          <Form.Item label="日期区间" name="date_range" rules={[{ required: true, message: "请选择日期区间" }]}>
            <DatePicker.RangePicker style={{ width: "100%" }} />
          </Form.Item>

          <Row gutter={12}>
            <Col xs={24} sm={12}>
              <Form.Item label="初始资金" name="initial_cash" rules={[{ required: true, message: "请输入初始资金" }]}>
                <InputNumber min={1} precision={2} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
            <Col xs={24} sm={12}>
              <Form.Item label="赎回到账(T+n)" name="settlement_days">
                <InputNumber min={0} max={10} step={1} precision={0} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
          </Row>
          <Row gutter={12}>
            <Col xs={24} sm={12}>
              <Form.Item label="买入费率" name="buy_fee_rate">
                <InputNumber min={0} max={0.5} step={0.001} precision={4} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
            <Col xs={24} sm={12}>
              <Form.Item label="卖出费率" name="sell_fee_rate">
                <InputNumber min={0} max={0.5} step={0.001} precision={4} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
          </Row>

          <Descriptions size="small" column={1} style={{ marginTop: 8 }} bordered>
            <Descriptions.Item label="回测(backtest)">
              支持策略：等权买入持有 / 全市场 Top-K / 全市场 Top-K + 参考指数择时。创建后可在列表或详情页触发运行。
            </Descriptions.Item>
            <Descriptions.Item label="环境(env)">
              用于 step / 训练。创建时会返回初始 observation；建议在训练页批量使用。
            </Descriptions.Item>
          </Descriptions>
        </Form>
      </Modal>
    </AuthedLayout>
  );
}
