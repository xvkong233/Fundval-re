"use client";

import { DeleteOutlined, EditOutlined, PlusOutlined, ReloadOutlined } from "@ant-design/icons";
import {
  Button,
  Card,
  Checkbox,
  Form,
  Grid,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Statistic,
  Table,
  type TableColumnsType,
  Typography,
  message,
} from "antd";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useState } from "react";
import { AuthedLayout } from "../../components/AuthedLayout";
import { createAccount, deleteAccount, listAccounts, patchAccount } from "../../lib/api";
import { pickDefaultParentAccountId, type Account } from "../../lib/accounts";

const { Text } = Typography;

function toNumber(v: any): number | null {
  if (v === null || v === undefined || v === "") return null;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

function formatMoney(v: any): string {
  const n = toNumber(v);
  if (n === null) return "-";
  return n.toFixed(2);
}

function formatPercent(v: any): string {
  const n = toNumber(v);
  if (n === null) return "-";
  return `${(n * 100).toFixed(2)}%`;
}

function pnlColor(v: any): string | undefined {
  const n = toNumber(v);
  if (n === null) return undefined;
  return n >= 0 ? "#cf1322" : "#3f8600";
}

export default function AccountsPage() {
  const router = useRouter();
  const screens = Grid.useBreakpoint();
  const isMobile = !screens.md;

  const [loading, setLoading] = useState(false);
  const [accounts, setAccounts] = useState<Account[]>([]);

  const [showAllSummary, setShowAllSummary] = useState(false);
  const [selectedParentId, setSelectedParentId] = useState<string | null>(null);

  const [modalOpen, setModalOpen] = useState(false);
  const [modalMode, setModalMode] = useState<"create" | "edit">("create");
  const [currentAccount, setCurrentAccount] = useState<Account | null>(null);
  const [form] = Form.useForm();

  const parentAccounts = useMemo(() => accounts.filter((a) => !a?.parent), [accounts]);

  const selectedParent = useMemo(() => {
    if (showAllSummary) return null;
    if (!selectedParentId) return null;
    return parentAccounts.find((a) => a.id === selectedParentId) ?? null;
  }, [parentAccounts, selectedParentId, showAllSummary]);

  const childAccounts = useMemo(() => {
    const children = selectedParent?.children;
    return Array.isArray(children) ? (children as Account[]) : [];
  }, [selectedParent]);

  const allSummary = useMemo(() => {
    const parents = parentAccounts;
    const sum = (key: string) =>
      parents.reduce((acc, a) => acc + (toNumber((a as any)[key]) ?? 0), 0);

    const holding_value = sum("holding_value");
    const today_pnl = sum("today_pnl");
    return {
      holding_cost: sum("holding_cost"),
      holding_value,
      pnl: sum("pnl"),
      estimate_value: sum("estimate_value"),
      estimate_pnl: sum("estimate_pnl"),
      today_pnl,
      today_pnl_rate: holding_value > 0 ? today_pnl / holding_value : null,
    };
  }, [parentAccounts]);

  const load = useCallback(async (opts?: { keepSelected?: boolean }) => {
    setLoading(true);
    try {
      const res = await listAccounts();
      const list = Array.isArray(res.data) ? (res.data as Account[]) : [];
      setAccounts(list);

      const keep = opts?.keepSelected ?? true;
      if (!keep || !selectedParentId || !list.some((a) => a.id === selectedParentId)) {
        setSelectedParentId(pickDefaultParentAccountId(list));
      }
    } catch (error: any) {
      const msg = error?.response?.data?.error || "加载账户失败";
      message.error(msg);
    } finally {
      setLoading(false);
    }
  }, [selectedParentId]);

  useEffect(() => {
    void load({ keepSelected: false });
  }, [load]);

  const openCreate = () => {
    setModalMode("create");
    setCurrentAccount(null);
    form.resetFields();
    setModalOpen(true);
  };

  const openEdit = useCallback(
    (account: Account) => {
      setModalMode("edit");
      setCurrentAccount(account);
      form.setFieldsValue({
        name: account.name ?? "",
        parent: account.parent ?? null,
        is_default: account.is_default ?? false,
      });
      setModalOpen(true);
    },
    [form]
  );

  const submit = async () => {
    const values = await form.validateFields();
    const payload = {
      name: String(values.name ?? "").trim(),
      parent: values.parent ?? null,
      is_default: !!values.is_default,
    };
    if (!payload.name) {
      message.error("请输入账户名称");
      return;
    }

    setLoading(true);
    try {
      if (modalMode === "create") {
        await createAccount(payload);
        message.success("创建成功");
      } else if (currentAccount) {
        await patchAccount(currentAccount.id, payload);
        message.success("更新成功");
      }
      setModalOpen(false);
      await load({ keepSelected: true });
    } catch (error: any) {
      const msg = error?.response?.data?.error || (modalMode === "create" ? "创建失败" : "更新失败");
      message.error(msg);
    } finally {
      setLoading(false);
    }
  };

  const remove = useCallback(async (accountId: string) => {
    setLoading(true);
    try {
      await deleteAccount(accountId);
      message.success("删除成功");
      await load({ keepSelected: false });
    } catch (error: any) {
      const msg = error?.response?.data?.error || "删除失败";
      message.error(msg);
    } finally {
      setLoading(false);
    }
  }, [load]);

  const parentOptions = useMemo(() => {
    if (modalMode === "create") return parentAccounts;
    return parentAccounts.filter((p) => p.id !== currentAccount?.id);
  }, [currentAccount?.id, modalMode, parentAccounts]);

  const columns = useMemo<TableColumnsType<Account>>(() => {
    if (isMobile) {
      return [
        {
          title: "账户",
          key: "name",
          render: (_: any, record: Account) => (
            <div style={{ minWidth: 0 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
                <Text ellipsis style={{ maxWidth: 220 }}>
                  {String(record.name ?? record.id)}
                </Text>
                {record.is_default ? (
                  <Text type="secondary" style={{ fontSize: 12, whiteSpace: "nowrap" }}>
                    (默认)
                  </Text>
                ) : null}
              </div>
              <Text type="secondary" style={{ fontSize: 12, whiteSpace: "nowrap" }}>
                成本 {formatMoney((record as any).holding_cost)} · 市值 {formatMoney((record as any).holding_value)}
              </Text>
            </div>
          ),
        },
        {
          title: "盈亏",
          key: "pnl",
          width: 140,
          render: (_: any, record: Account) => (
            <span style={{ color: pnlColor((record as any).pnl), whiteSpace: "nowrap" }}>
              {formatMoney((record as any).pnl)}（{formatPercent((record as any).pnl_rate)}）
            </span>
          ),
        },
        {
          title: "操作",
          key: "action",
          width: 140,
          render: (_: any, record: Account) => (
            <Space size={4}>
              {record.parent ? (
                <Button
                  type="link"
                  size="small"
                  onClick={() => router.push(`/positions?account=${encodeURIComponent(record.id)}`)}
                >
                  持仓
                </Button>
              ) : null}
              <Button type="link" size="small" icon={<EditOutlined />} onClick={() => openEdit(record)} />
              <Popconfirm
                title="确定要删除账户吗？"
                description="删除后无法恢复"
                okText="确定"
                cancelText="取消"
                onConfirm={() => void remove(record.id)}
              >
                <Button type="link" size="small" danger icon={<DeleteOutlined />} />
              </Popconfirm>
            </Space>
          ),
        },
      ];
    }

    return [
      {
        title: "账户名称",
        dataIndex: "name",
        key: "name",
        render: (v: any, record: Account) => (
          <span style={{ whiteSpace: "nowrap" }}>
            {String(v ?? "")}
            {record.is_default ? (
              <Text type="secondary" style={{ marginLeft: 8, fontSize: 12 }}>
                (默认)
              </Text>
            ) : null}
          </span>
        ),
      },
      { title: "持仓成本", dataIndex: "holding_cost", key: "holding_cost", render: formatMoney },
      { title: "持仓市值", dataIndex: "holding_value", key: "holding_value", render: formatMoney },
      {
        title: "总盈亏",
        dataIndex: "pnl",
        key: "pnl",
        render: (v: any) => <span style={{ color: pnlColor(v) }}>{formatMoney(v)}</span>,
      },
      {
        title: "收益率",
        dataIndex: "pnl_rate",
        key: "pnl_rate",
        render: (v: any) => <span style={{ color: pnlColor(v) }}>{formatPercent(v)}</span>,
      },
      {
        title: "预估市值",
        dataIndex: "estimate_value",
        key: "estimate_value",
        render: formatMoney,
        responsive: ["lg"],
      },
      {
        title: "今日盈亏(预估)",
        dataIndex: "today_pnl",
        key: "today_pnl",
        render: (v: any) => <span style={{ color: pnlColor(v) }}>{formatMoney(v)}</span>,
        responsive: ["md"],
      },
      {
        title: "操作",
        key: "action",
        width: 160,
        render: (_: any, record: Account) => (
          <Space size="small">
            {record.parent ? (
              <Button
                type="link"
                size="small"
                onClick={() => router.push(`/positions?account=${encodeURIComponent(record.id)}`)}
              >
                持仓
              </Button>
            ) : null}
            <Button type="link" size="small" icon={<EditOutlined />} onClick={() => openEdit(record)} />
            <Popconfirm
              title="确定要删除账户吗？"
              description="删除后无法恢复"
              okText="确定"
              cancelText="取消"
              onConfirm={() => void remove(record.id)}
            >
              <Button type="link" size="small" danger icon={<DeleteOutlined />} />
            </Popconfirm>
          </Space>
        ),
      },
    ];
  }, [isMobile, openEdit, remove, router]);

  const columnsNoAction = useMemo(() => columns.filter((c) => (c as any).key !== "action"), [columns]);

  return (
    <AuthedLayout title="账户">
      <Card
        title="账户管理"
        extra={
          <div className="fv-toolbarScroll">
            <Space wrap>
              <Button onClick={() => setShowAllSummary((v) => !v)}>
                {showAllSummary ? "返回单账户" : "全部账户汇总"}
              </Button>
              <Button icon={<ReloadOutlined />} loading={loading} onClick={() => void load({ keepSelected: true })}>
                刷新
              </Button>
              <Button type="primary" icon={<PlusOutlined />} onClick={openCreate}>
                创建账户
              </Button>
            </Space>
          </div>
        }
      >
        {!showAllSummary ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <Select
              style={{ width: isMobile ? "100%" : 320, maxWidth: "100%" }}
              placeholder="选择父账户"
              value={selectedParentId ?? undefined}
              onChange={(v) => setSelectedParentId(v)}
              options={parentAccounts.map((a) => ({
                label: `${a.name}${a.is_default ? " (默认)" : ""}`,
                value: a.id,
              }))}
            />

            <Card size="small" title={selectedParent ? `汇总：${selectedParent.name}` : "汇总"}>
              <div className="fv-kpiGrid4">
                <Statistic title="持仓成本" value={formatMoney(selectedParent?.holding_cost)} />
                <Statistic title="持仓市值" value={formatMoney(selectedParent?.holding_value)} />
                <Statistic
                  title="总盈亏"
                  valueStyle={{ color: pnlColor(selectedParent?.pnl) }}
                  value={formatMoney(selectedParent?.pnl)}
                />
                <Statistic
                  title="收益率"
                  valueStyle={{ color: pnlColor(selectedParent?.pnl_rate) }}
                  value={formatPercent(selectedParent?.pnl_rate)}
                />
              </div>
            </Card>

            <Table<Account>
              rowKey={(r) => r.id}
              loading={loading}
              columns={columns}
              dataSource={childAccounts}
              pagination={{
                pageSize: isMobile ? 10 : 20,
                simple: isMobile,
                showLessItems: isMobile,
                showSizeChanger: !isMobile,
              }}
              size={isMobile ? "small" : "middle"}
              scroll={isMobile ? undefined : { x: "max-content" }}
            />
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <Card size="small" title="全部账户汇总">
              <div className="fv-kpiGrid4">
                <Statistic title="持仓成本" value={formatMoney(allSummary.holding_cost)} />
                <Statistic title="持仓市值" value={formatMoney(allSummary.holding_value)} />
                <Statistic title="预估市值" value={formatMoney(allSummary.estimate_value)} />
                <Statistic
                  title="今日盈亏(预估)"
                  valueStyle={{ color: pnlColor(allSummary.today_pnl) }}
                  value={formatMoney(allSummary.today_pnl)}
                />
              </div>
            </Card>

            <Table<Account>
              rowKey={(r) => r.id}
              loading={loading}
              columns={columnsNoAction}
              dataSource={parentAccounts}
              pagination={{
                pageSize: isMobile ? 10 : 20,
                simple: isMobile,
                showLessItems: isMobile,
                showSizeChanger: !isMobile,
              }}
              size={isMobile ? "small" : "middle"}
              scroll={isMobile ? undefined : { x: "max-content" }}
            />
          </div>
        )}
      </Card>

      <Modal
        title={modalMode === "create" ? "创建账户" : "编辑账户"}
        open={modalOpen}
        onCancel={() => setModalOpen(false)}
        onOk={() => void submit()}
        confirmLoading={loading}
        okText={modalMode === "create" ? "创建" : "保存"}
        cancelText="取消"
      >
        <Form form={form} layout="vertical" preserve={false} initialValues={{ parent: null, is_default: false }}>
          <Form.Item
            label="账户名称"
            name="name"
            rules={[{ required: true, message: "请输入账户名称" }]}
          >
            <Input placeholder="例如：我的账户" maxLength={32} />
          </Form.Item>

          <Form.Item label="父账户" name="parent" extra="留空表示顶级账户（父账户）">
            <Select
              allowClear
              placeholder="选择父账户（可选）"
              options={parentOptions.map((p) => ({ label: p.name ?? p.id, value: p.id }))}
            />
          </Form.Item>

          <Form.Item name="is_default" valuePropName="checked">
            <Checkbox>设为默认账户</Checkbox>
          </Form.Item>
        </Form>
      </Modal>
    </AuthedLayout>
  );
}
