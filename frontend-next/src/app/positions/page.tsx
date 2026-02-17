"use client";

import {
  DeleteOutlined,
  MinusOutlined,
  PlusOutlined,
  ReloadOutlined,
  RollbackOutlined,
  SearchOutlined,
} from "@ant-design/icons";
import {
  AutoComplete,
  Button,
  Card,
  Checkbox,
  Col,
  DatePicker,
  Divider,
  Empty,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
  Row,
  Select,
  Space,
  Statistic,
  Table,
  Tag,
  Typography,
  message,
} from "antd";
import { Suspense, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "next/navigation";
import { AuthedLayout } from "../../components/AuthedLayout";
import {
  batchEstimate,
  batchUpdateNav,
  createPositionOperation,
  deletePositionOperation,
  listAccounts,
  listFunds,
  listPositionOperations,
  listPositions,
  queryFundNav,
  recalculatePositions,
} from "../../lib/api";
import { normalizeFundList, type Fund } from "../../lib/funds";
import { pickDefaultChildAccountId } from "../../lib/positions";

const { Text } = Typography;

type Account = Record<string, any> & { id: string; name?: string; parent: string | null; parent_name?: string };

type Position = Record<string, any> & {
  id: string;
  account: string;
  account_name?: string;
  fund_code: string;
  fund_name?: string;
  fund_type?: string;
  fund?: Fund;
  holding_share?: string;
  holding_cost?: string;
  holding_nav?: string;
  pnl?: string;
};

type Operation = Record<string, any> & {
  id: string;
  account: string;
  account_name?: string;
  fund_code: string;
  fund_name?: string;
  operation_type: "BUY" | "SELL";
  operation_date: string;
  before_15: boolean;
  amount: string;
  share: string;
  nav: string;
  created_at: string;
};

function toNumber(v: any): number | null {
  if (v === null || v === undefined || v === "") return null;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

function money(v: any): string {
  const n = toNumber(v);
  if (n === null) return "-";
  return n.toLocaleString("zh-CN", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function fixed4(v: any): string {
  const n = toNumber(v);
  if (n === null) return "-";
  return n.toFixed(4);
}

function pct(v: any): string {
  const n = toNumber(v);
  if (n === null) return "-";
  return `${n.toFixed(2)}%`;
}

function pnlColor(v: any): string | undefined {
  const n = toNumber(v);
  if (n === null) return undefined;
  return n >= 0 ? "#cf1322" : "#3f8600";
}

function opTag(type: "BUY" | "SELL") {
  return type === "BUY" ? <Tag color="red">买入</Tag> : <Tag color="green">卖出</Tag>;
}

function asYmd(dateValue: any): string | null {
  if (!dateValue) return null;
  if (typeof dateValue.format === "function") return dateValue.format("YYYY-MM-DD");
  return null;
}

export default function PositionsPage() {
  return (
    <Suspense
      fallback={
        <AuthedLayout title="持仓">
          <Card loading />
        </AuthedLayout>
      }
    >
      <PositionsInner />
    </Suspense>
  );
}

function PositionsInner() {
  const searchParams = useSearchParams();
  const preferredAccountId = searchParams?.get("account");

  const [loading, setLoading] = useState(false);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const childAccounts = useMemo(() => accounts.filter((a) => !!a?.parent), [accounts]);
  const accountById = useMemo(() => new Map(accounts.map((a) => [a.id, a])), [accounts]);

  const childAccountOptions = useMemo(() => {
    return childAccounts.map((a) => {
      const parentName =
        a.parent_name ??
        (typeof a.parent === "string" ? (accountById.get(a.parent)?.name as string | undefined) : undefined);
      return {
        label: parentName ? `${parentName} / ${a.name ?? a.id}` : a.name ?? a.id,
        value: a.id,
      };
    });
  }, [accountById, childAccounts]);

  const [selectedAccountId, setSelectedAccountId] = useState<string | null>(null);

  const [positionsLoading, setPositionsLoading] = useState(false);
  const [positions, setPositions] = useState<Position[]>([]);
  const [fundTypeFilter, setFundTypeFilter] = useState<string>("all");

  const [opsLoading, setOpsLoading] = useState(false);
  const [operations, setOperations] = useState<Operation[]>([]);

  const [refreshingFundData, setRefreshingFundData] = useState(false);

  const [opModalOpen, setOpModalOpen] = useState(false);
  const [opModalMode, setOpModalMode] = useState<"build" | "buy" | "sell">("build");
  const [opForm] = Form.useForm();

  const watchedFundCode = Form.useWatch("fund_code", opForm) as string | undefined;
  const watchedDate = Form.useWatch("operation_date", opForm);
  const watchedBefore15 = Form.useWatch("before_15", opForm) as boolean | undefined;

  const [fundOptions, setFundOptions] = useState<Array<{ value: string; label: string }>>([]);
  const [fundSearchLoading, setFundSearchLoading] = useState(false);
  const fundSearchSeq = useRef(0);

  const selectedAccount = useMemo(() => {
    if (!selectedAccountId) return null;
    return childAccounts.find((a) => a.id === selectedAccountId) ?? null;
  }, [childAccounts, selectedAccountId]);

  const accountStats = useMemo(() => {
    if (!selectedAccount) {
      return {
        holding_cost: "0.00",
        holding_value: "0.00",
        pnl: "0.00",
        pnl_rate: null,
        today_pnl: "0.00",
        today_pnl_rate: null,
      };
    }
    return {
      holding_cost: selectedAccount.holding_cost || "0.00",
      holding_value: selectedAccount.holding_value || "0.00",
      pnl: selectedAccount.pnl || "0.00",
      pnl_rate: selectedAccount.pnl_rate,
      today_pnl: selectedAccount.today_pnl || "0.00",
      today_pnl_rate: selectedAccount.today_pnl_rate,
    };
  }, [selectedAccount]);

  const fundTypeOptions = useMemo(() => {
    const set = new Set<string>();
    for (const p of positions) {
      const t = (p.fund_type ?? p.fund?.fund_type) as string | undefined;
      if (typeof t === "string" && t.trim()) set.add(t.trim());
    }
    return ["all", ...Array.from(set).sort((a, b) => a.localeCompare(b, "zh-CN"))];
  }, [positions]);

  const filteredPositions = useMemo(() => {
    if (fundTypeFilter === "all") return positions;
    return positions.filter((p) => {
      const t = (p.fund_type ?? p.fund?.fund_type) as string | undefined;
      return typeof t === "string" && t.includes(fundTypeFilter);
    });
  }, [fundTypeFilter, positions]);

  const getOperationTypeTag = (record: Operation) => {
    if (record.operation_type === "SELL") return <Tag color="green">减仓</Tag>;

    const fundOps = operations
      .filter((op) => op.fund_code === record.fund_code)
      .sort((a, b) => {
        const d = new Date(a.operation_date).getTime() - new Date(b.operation_date).getTime();
        if (d !== 0) return d;
        return new Date(a.created_at).getTime() - new Date(b.created_at).getTime();
      });

    const isBuild = fundOps.length > 0 && fundOps[0]?.id === record.id;
    return <Tag color="red">{isBuild ? "建仓" : "加仓"}</Tag>;
  };

  const loadAccounts = async () => {
    setLoading(true);
    try {
      const res = await listAccounts();
      const list = Array.isArray(res.data) ? (res.data as Account[]) : [];
      setAccounts(list);

      const nextSelected = pickDefaultChildAccountId(list as any, preferredAccountId);
      setSelectedAccountId(nextSelected);
    } catch (error: any) {
      const msg = error?.response?.data?.error || "加载账户失败";
      message.error(msg);
    } finally {
      setLoading(false);
    }
  };

  const refreshFundData = async (pos: Position[]) => {
    const codes = pos.map((p) => p.fund_code).filter(Boolean);
    if (codes.length === 0) return;
    setRefreshingFundData(true);
    try {
      const [navRes, estRes] = await Promise.all([batchUpdateNav(codes), batchEstimate(codes)]);
      setPositions((prev) =>
        prev.map((p) => {
          const nav = navRes.data?.[p.fund_code];
          const est = estRes.data?.[p.fund_code];
          const fund = { ...(p.fund ?? {}) } as any;
          if (nav && !nav.error) {
            fund.latest_nav = nav.latest_nav ?? fund.latest_nav;
            fund.latest_nav_date = nav.latest_nav_date ?? fund.latest_nav_date;
          }
          if (est && !est.error) {
            fund.estimate_nav = est.estimate_nav ?? fund.estimate_nav;
            fund.estimate_growth = est.estimate_growth ?? fund.estimate_growth;
            fund.estimate_time = est.estimate_time ?? fund.estimate_time;
            fund.fund_name = est.fund_name ?? fund.fund_name;
          }
          return { ...p, fund };
        })
      );
      message.success("估值/净值已刷新");
    } catch {
      message.error("刷新基金数据失败");
    } finally {
      setRefreshingFundData(false);
    }
  };

  const loadPositions = async (accountId: string) => {
    setPositionsLoading(true);
    try {
      const res = await listPositions({ account: accountId });
      const list = Array.isArray(res.data) ? (res.data as Position[]) : [];
      setPositions(list);
      await refreshFundData(list);
    } catch (error: any) {
      const msg = error?.response?.data?.error || "加载持仓失败";
      message.error(msg);
    } finally {
      setPositionsLoading(false);
    }
  };

  const loadOperations = async (accountId: string) => {
    setOpsLoading(true);
    try {
      const res = await listPositionOperations({ account: accountId });
      const list = Array.isArray(res.data) ? (res.data as Operation[]) : [];
      const sorted = [...list].sort((a, b) => {
        const d = new Date(b.operation_date).getTime() - new Date(a.operation_date).getTime();
        if (d !== 0) return d;
        return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
      });
      setOperations(sorted);
    } catch (error: any) {
      const msg = error?.response?.data?.error || "加载操作流水失败";
      message.error(msg);
    } finally {
      setOpsLoading(false);
    }
  };

  useEffect(() => {
    void loadAccounts();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!selectedAccountId) return;
    void loadPositions(selectedAccountId);
    void loadOperations(selectedAccountId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedAccountId]);

  useEffect(() => {
    if (!opModalOpen) return;
    const fund_code = String(watchedFundCode ?? "").trim();
    const operation_date = asYmd(watchedDate);
    const before_15 = !!watchedBefore15;
    if (!fund_code || !operation_date) return;

    const run = async () => {
      try {
        const resp = await queryFundNav({ fund_code, operation_date, before_15 });
        const nav = resp.data?.nav ?? resp.data?.latest_nav ?? resp.data?.value;
        if (nav !== undefined && nav !== null && nav !== "") {
          opForm.setFieldValue("nav", Number(nav));
        }
      } catch {
        // ignore; allow manual input
      }
    };
    void run();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [opModalOpen, watchedFundCode, watchedDate, watchedBefore15]);

  const searchFunds = async (keyword: string) => {
    const q = keyword.trim();
    if (!q) {
      setFundOptions([]);
      return;
    }

    const seq = ++fundSearchSeq.current;
    setFundSearchLoading(true);
    try {
      const res = await listFunds({ page: 1, page_size: 10, search: q });
      if (seq !== fundSearchSeq.current) return;
      const normalized = normalizeFundList(res.data);
      setFundOptions(
        normalized.results
          .filter((f) => f.fund_code)
          .map((f) => ({ value: f.fund_code, label: `${f.fund_code}${f.fund_name ? `  ${f.fund_name}` : ""}` }))
      );
    } catch {
      if (seq !== fundSearchSeq.current) return;
      setFundOptions([]);
    } finally {
      if (seq === fundSearchSeq.current) setFundSearchLoading(false);
    }
  };

  const openBuild = () => {
    if (!selectedAccountId) return;
    setOpModalMode("build");
    opForm.resetFields();
    opForm.setFieldsValue({
      account: selectedAccountId,
      operation_type: "BUY",
      before_15: true,
    });
    setOpModalOpen(true);
  };

  const openBuySell = (mode: "buy" | "sell", position: Position) => {
    if (!selectedAccountId) return;
    setOpModalMode(mode);
    opForm.resetFields();
    opForm.setFieldsValue({
      account: selectedAccountId,
      fund_code: position.fund_code,
      operation_type: mode === "sell" ? "SELL" : "BUY",
      before_15: true,
    });
    setOpModalOpen(true);
  };

  const submitOperation = async () => {
    if (!selectedAccountId) return;
    const values = await opForm.validateFields();
    const fund_code = String(values.fund_code ?? "").trim();
    const operation_date = asYmd(values.operation_date);
    if (!fund_code || !operation_date) {
      message.error("请完善基金与日期");
      return;
    }

    setLoading(true);
    try {
      await createPositionOperation({
        account: selectedAccountId,
        fund_code,
        operation_type: values.operation_type,
        operation_date,
        before_15: !!values.before_15,
        amount: values.amount,
        share: values.share,
        nav: values.nav,
      });
      message.success("操作已创建（持仓已自动重算）");
      setOpModalOpen(false);
      await loadPositions(selectedAccountId);
      await loadOperations(selectedAccountId);
    } catch (error: any) {
      const msg = error?.response?.data?.error || "创建操作失败";
      message.error(msg);
    } finally {
      setLoading(false);
    }
  };

  const rollback = async (opId: string) => {
    if (!selectedAccountId) return;
    setLoading(true);
    try {
      await deletePositionOperation(opId);
      message.success("已回滚");
      await loadPositions(selectedAccountId);
      await loadOperations(selectedAccountId);
    } catch (error: any) {
      if (error?.response?.status === 403) {
        message.error("无权限：仅管理员可回滚操作");
        return;
      }
      const msg = error?.response?.data?.error || "回滚失败";
      message.error(msg);
    } finally {
      setLoading(false);
    }
  };

  const recalc = async () => {
    if (!selectedAccountId) return;
    setLoading(true);
    try {
      await recalculatePositions(selectedAccountId);
      message.success("已触发重算");
      await loadPositions(selectedAccountId);
      await loadOperations(selectedAccountId);
    } catch (error: any) {
      if (error?.response?.status === 403) {
        message.error("无权限：仅管理员可重算");
        return;
      }
      const msg = error?.response?.data?.error || "重算失败";
      message.error(msg);
    } finally {
      setLoading(false);
    }
  };

  return (
    <AuthedLayout title="持仓">
      <Card
        title="持仓管理"
        extra={
          <Space wrap>
            <Button icon={<ReloadOutlined />} loading={loading} onClick={() => void loadAccounts()}>
              刷新账户
            </Button>
            <Button icon={<RollbackOutlined />} loading={loading} onClick={() => void recalc()}>
              重算持仓
            </Button>
            <Button type="primary" icon={<PlusOutlined />} onClick={openBuild} disabled={!selectedAccountId}>
              建仓/加减仓
            </Button>
          </Space>
        }
      >
        {childAccounts.length === 0 ? (
          <Empty description="请先创建子账户（子账户才能持仓）" />
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <Select
              style={{ width: 420, maxWidth: "100%" }}
              value={selectedAccountId ?? undefined}
              onChange={(v) => setSelectedAccountId(v)}
              placeholder="选择子账户"
              options={childAccountOptions}
            />

            <Card size="small" title={selectedAccount ? `账户：${selectedAccount.name}` : "账户汇总"}>
              <Row gutter={16}>
                <Col span={6}>
                  <Statistic title="持仓成本" value={money(accountStats.holding_cost)} />
                </Col>
                <Col span={6}>
                  <Statistic title="持仓市值" value={money(accountStats.holding_value)} />
                </Col>
                <Col span={6}>
                  <Statistic
                    title="总盈亏"
                    valueStyle={{ color: pnlColor(accountStats.pnl) }}
                    value={money(accountStats.pnl)}
                  />
                </Col>
                <Col span={6}>
                  <Statistic title="今日盈亏(预估)" valueStyle={{ color: pnlColor(accountStats.today_pnl) }} value={money(accountStats.today_pnl)} />
                </Col>
              </Row>
            </Card>

            <Space wrap>
              <Button
                icon={<ReloadOutlined />}
                loading={refreshingFundData}
                onClick={() => void refreshFundData(positions)}
                disabled={positions.length === 0}
              >
                刷新估值/净值
              </Button>
              <Select
                style={{ width: 220 }}
                value={fundTypeFilter}
                onChange={(v) => setFundTypeFilter(v)}
                options={fundTypeOptions.map((t) => ({ value: t, label: t === "all" ? "全部类型" : t }))}
              />
            </Space>

            <Table<Position>
              rowKey={(r) => r.id}
              loading={positionsLoading}
              dataSource={filteredPositions}
              pagination={false}
              size="middle"
              columns={[
                { title: "代码", dataIndex: "fund_code", width: 110 },
                { title: "基金名称", dataIndex: "fund_name", ellipsis: true },
                {
                  title: "最新净值",
                  key: "latest_nav",
                  width: 160,
                  render: (_, record) => {
                    const nav = record.fund?.latest_nav ?? (record as any).latest_nav;
                    const date = record.fund?.latest_nav_date ?? (record as any).latest_nav_date;
                    if (!nav) return "-";
                    const dateStr = typeof date === "string" ? `(${date.slice(5)})` : "";
                    return (
                      <span style={{ whiteSpace: "nowrap" }}>
                        {fixed4(nav)}
                        <Text type="secondary" style={{ fontSize: 11, marginLeft: 4 }}>
                          {dateStr}
                        </Text>
                      </span>
                    );
                  },
                },
                {
                  title: "实时估值",
                  key: "estimate_nav",
                  width: 140,
                  render: (_, record) => {
                    const nav = record.fund?.estimate_nav;
                    return nav ? fixed4(nav) : "-";
                  },
                },
                {
                  title: "估算涨跌(%)",
                  key: "estimate_growth",
                  width: 140,
                  render: (_, record) => {
                    const g = record.fund?.estimate_growth;
                    if (g === undefined || g === null || g === "") return "-";
                    const n = toNumber(g);
                    const text = n === null ? String(g) : n.toFixed(2);
                    const positive = n === null ? !String(g).startsWith("-") : n >= 0;
                    return (
                      <span style={{ color: positive ? "#cf1322" : "#3f8600" }}>
                        {n !== null && n >= 0 ? "+" : ""}
                        {text}
                      </span>
                    );
                  },
                },
                { title: "持有份额", dataIndex: "holding_share", width: 130, render: (v) => (v ? String(v) : "-") },
                { title: "持有成本", dataIndex: "holding_cost", width: 130, render: money },
                {
                  title: "盈亏",
                  dataIndex: "pnl",
                  width: 120,
                  render: (v) => <span style={{ color: pnlColor(v) }}>{money(v)}</span>,
                },
                {
                  title: "操作",
                  key: "action",
                  width: 160,
                  render: (_, record) => (
                    <Space size="small">
                      <Button size="small" icon={<PlusOutlined />} onClick={() => openBuySell("buy", record)}>
                        加仓
                      </Button>
                      <Button size="small" icon={<MinusOutlined />} onClick={() => openBuySell("sell", record)}>
                        减仓
                      </Button>
                    </Space>
                  ),
                },
              ]}
            />
          </div>
        )}
      </Card>

      <Card title="操作流水" style={{ marginTop: 16 }}>
            <Table<Operation>
              rowKey={(r) => r.id}
              loading={opsLoading}
              dataSource={operations}
              pagination={false}
              size="small"
              locale={{ emptyText: selectedAccountId ? "暂无操作流水" : "请选择子账户" }}
              columns={[
                { title: "日期", dataIndex: "operation_date", width: 120 },
                {
                  title: "类型",
                  dataIndex: "operation_type",
                  width: 90,
                  render: (_: any, record) => getOperationTypeTag(record),
                },
            { title: "基金", key: "fund", render: (_, r) => `${r.fund_code}${r.fund_name ? ` ${r.fund_name}` : ""}` },
            { title: "金额", dataIndex: "amount", width: 110, render: money },
            { title: "份额", dataIndex: "share", width: 110, render: (v) => (v ? String(v) : "-") },
            { title: "净值", dataIndex: "nav", width: 110, render: fixed4 },
            {
              title: "15点前",
              dataIndex: "before_15",
              width: 80,
              render: (v: any) => (v ? "是" : "否"),
            },
            {
              title: "操作",
              key: "action",
              width: 110,
              render: (_, record) => (
                <Popconfirm
                  title="确认回滚该操作？"
                  okText="回滚"
                  cancelText="取消"
                  onConfirm={() => void rollback(record.id)}
                >
                  <Button size="small" danger icon={<DeleteOutlined />}>
                    回滚
                  </Button>
                </Popconfirm>
              ),
            },
          ]}
        />
      </Card>

      <Modal
        title={opModalMode === "sell" ? "减仓" : opModalMode === "buy" ? "加仓" : "创建操作"}
        open={opModalOpen}
        onCancel={() => setOpModalOpen(false)}
        onOk={() => void submitOperation()}
        okText="提交"
        cancelText="取消"
        confirmLoading={loading}
      >
        <Form
          form={opForm}
          layout="vertical"
          preserve={false}
          initialValues={{ before_15: true, operation_type: "BUY" }}
        >
          <Form.Item name="operation_type" hidden>
            <Input />
          </Form.Item>
          <Form.Item label="基金" name="fund_code" rules={[{ required: true, message: "请选择基金" }]}>
            <AutoComplete
              options={fundOptions}
              onSearch={(v) => void searchFunds(v)}
              onSelect={(v) => opForm.setFieldValue("fund_code", v)}
              placeholder="搜索基金代码或名称"
              notFoundContent={fundSearchLoading ? <Text type="secondary">搜索中…</Text> : null}
              disabled={opModalMode !== "build"}
              filterOption={false}
            />
          </Form.Item>

          <Row gutter={12}>
            <Col span={12}>
              <Form.Item label="操作日期" name="operation_date" rules={[{ required: true, message: "请选择日期" }]}>
                <DatePicker style={{ width: "100%" }} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="before_15" valuePropName="checked" label=" ">
                <Checkbox>15:00 前操作</Checkbox>
              </Form.Item>
            </Col>
          </Row>

          <Divider style={{ margin: "12px 0" }} />

          <Row gutter={12}>
            <Col span={8}>
              <Form.Item label="金额" name="amount" rules={[{ required: true, message: "请输入金额" }]}>
                <InputNumber style={{ width: "100%" }} min={0} precision={2} />
              </Form.Item>
            </Col>
            <Col span={8}>
              <Form.Item label="份额" name="share" rules={[{ required: true, message: "请输入份额" }]}>
                <InputNumber style={{ width: "100%" }} min={0} precision={4} />
              </Form.Item>
            </Col>
            <Col span={8}>
              <Form.Item label="净值" name="nav" rules={[{ required: true, message: "请输入净值" }]}>
                <InputNumber style={{ width: "100%" }} min={0} precision={4} />
              </Form.Item>
            </Col>
          </Row>

          <Space wrap>
            <Button
              icon={<SearchOutlined />}
              onClick={async () => {
                const fund_code = String(opForm.getFieldValue("fund_code") ?? "").trim();
                const operation_date = asYmd(opForm.getFieldValue("operation_date"));
                const before_15 = !!opForm.getFieldValue("before_15");
                if (!fund_code || !operation_date) {
                  message.error("请先选择基金与日期");
                  return;
                }
                try {
                  const resp = await queryFundNav({ fund_code, operation_date, before_15 });
                  const nav = resp.data?.nav ?? resp.data?.latest_nav ?? resp.data?.value;
                  if (nav !== undefined && nav !== null && nav !== "") {
                    opForm.setFieldValue("nav", Number(nav));
                    message.success("已填充净值");
                  } else {
                    message.warning("未获取到净值，请手动填写");
                  }
                } catch {
                  message.error("查询净值失败，请手动填写");
                }
              }}
            >
              查询净值
            </Button>
          </Space>
        </Form>
      </Modal>
    </AuthedLayout>
  );
}
