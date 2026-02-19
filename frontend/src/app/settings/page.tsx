"use client";

import { Button, Card, Descriptions, Form, Input, Result, Space, Spin, Statistic, Typography, message, theme } from "antd";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { AuthedLayout } from "../../components/AuthedLayout";
import { changePassword, getCurrentUser, getMySummary, getTushareTokenStatus, setTushareToken } from "../../lib/api";
import { getChangePasswordErrorMessage } from "../../lib/changePassword";
import { useAuth } from "../../contexts/AuthContext";
import { normalizeUserSummary } from "../../lib/userSummary";

const { Paragraph, Text, Title } = Typography;

type ChangePasswordValues = {
  old_password: string;
  new_password: string;
  confirm_password: string;
};

type TushareTokenStatus = {
  configured?: boolean;
  token_hint?: string | null;
};

export default function SettingsPage() {
  const { logout, user, updateUser } = useAuth();
  const [saving, setSaving] = useState(false);
  const [done, setDone] = useState(false);
  const [form] = Form.useForm<ChangePasswordValues>();
  const [tushareForm] = Form.useForm<{ token: string }>();

  const [profileLoading, setProfileLoading] = useState(true);
  const [profileError, setProfileError] = useState<string | null>(null);
  const [me, setMe] = useState<any | null>(null);
  const [summary, setSummary] = useState<any | null>(null);
  const [profileNonce, setProfileNonce] = useState(0);

  const [tushareLoading, setTushareLoading] = useState(true);
  const [tushareError, setTushareError] = useState<string | null>(null);
  const [tushareStatus, setTushareStatus] = useState<TushareTokenStatus | null>(null);
  const [tushareSaving, setTushareSaving] = useState(false);

  const { token } = theme.useToken();

  useEffect(() => {
    let cancelled = false;

    async function run() {
      setProfileLoading(true);
      setProfileError(null);
      try {
        const [meRes, sumRes] = await Promise.all([getCurrentUser(), getMySummary()]);
        if (cancelled) return;
        setMe(meRes.data);
        setSummary(sumRes.data);

        // 尝试同步上下文（不覆盖已存在字段以外的内容也没关系）
        if (meRes.data) {
          updateUser(meRes.data);
        }
      } catch (error: any) {
        if (cancelled) return;
        const msg = error?.response?.data?.error || "加载用户信息失败";
        setProfileError(msg);
      } finally {
        if (!cancelled) setProfileLoading(false);
      }
    }

    void run();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [profileNonce]);

  useEffect(() => {
    let cancelled = false;

    async function run() {
      setTushareLoading(true);
      setTushareError(null);
      try {
        const res = await getTushareTokenStatus();
        if (cancelled) return;
        setTushareStatus((res.data ?? {}) as TushareTokenStatus);
      } catch (error: any) {
        if (cancelled) return;
        // 403：非管理员正常现象，不作为“设置页错误”处理
        if (error?.response?.status === 403) {
          setTushareStatus(null);
          setTushareError("需要管理员权限");
        } else {
          const msg = error?.response?.data?.error || "加载 Tushare Token 状态失败";
          setTushareStatus(null);
          setTushareError(msg);
        }
      } finally {
        if (!cancelled) setTushareLoading(false);
      }
    }

    void run();
    return () => {
      cancelled = true;
    };
  }, []);

  const normalizedSummary = useMemo(() => {
    if (!summary) return null;
    return normalizeUserSummary(summary);
  }, [summary]);

  const onChangePassword = async (values: ChangePasswordValues) => {
    setSaving(true);
    try {
      await changePassword(values.old_password, values.new_password);
      setDone(true);
      message.success("密码修改成功");
      form.resetFields();
    } catch (error: unknown) {
      message.error(getChangePasswordErrorMessage(error));
    } finally {
      setSaving(false);
    }
  };

  const onSaveTushareToken = async (values: { token: string }) => {
    setTushareSaving(true);
    try {
      const t = String(values?.token ?? "").trim();
      await setTushareToken(t.length ? t : null);
      const res = await getTushareTokenStatus();
      setTushareStatus((res.data ?? {}) as TushareTokenStatus);
      message.success("Tushare Token 已保存");
      tushareForm.resetFields();
    } catch (error: any) {
      const msg = error?.response?.data?.error || "保存 Tushare Token 失败";
      message.error(msg);
    } finally {
      setTushareSaving(false);
    }
  };

  return (
    <AuthedLayout title="设置">
      <Card style={{ marginBottom: 16 }}>
        <Title level={3} style={{ marginTop: 0 }}>
          系统设置
        </Title>
        <Paragraph type="secondary">
          Web 版本默认通过 <Text code>/api</Text> 反向代理访问后端，无需配置服务器地址。
        </Paragraph>
        <Paragraph type="secondary" style={{ marginBottom: 0 }}>
          如需切换后端地址，请在部署阶段调整 <Text code>API_PROXY_TARGET</Text>（或 Docker 环境变量）。
        </Paragraph>

        <Space style={{ marginTop: 16 }} wrap>
          <Link href="/server-config" prefetch={false}>
            <Button>查看服务器配置说明</Button>
          </Link>
          <Link href="/sources" prefetch={false}>
            <Button>查看数据源状态</Button>
          </Link>
          </Space>
        </Card>

        <Card style={{ marginBottom: 16 }}>
          <Title level={3} style={{ marginTop: 0 }}>
            数据源 Token
          </Title>
          <Paragraph type="secondary" style={{ marginBottom: 8 }}>
            Tushare 数据源需要在此配置 Token（仅管理员可操作）。配置后可在“数据源状态”页面查看 <Text strong>Tushare</Text>{" "}
            的健康度。
          </Paragraph>

          {tushareLoading ? (
            <div style={{ padding: "16px 0", display: "flex", justifyContent: "center" }}>
              <Spin />
            </div>
          ) : tushareError ? (
            <Result status="info" title="Tushare Token" subTitle={tushareError} />
          ) : (
            <Space direction="vertical" style={{ width: "100%" }} size={12}>
              <Descriptions size="small" column={1} bordered>
                <Descriptions.Item label="状态">
                  {tushareStatus?.configured ? (
                    <Text type="success">已配置（{tushareStatus.token_hint || "已隐藏"}）</Text>
                  ) : (
                    <Text type="warning">未配置</Text>
                  )}
                </Descriptions.Item>
              </Descriptions>

              <Form form={tushareForm} layout="vertical" onFinish={onSaveTushareToken} style={{ maxWidth: 520 }}>
                <Form.Item label="Tushare Token" name="token">
                  <Input.Password placeholder="粘贴 Token（留空并保存可清空）" autoComplete="off" />
                </Form.Item>
                <Space wrap>
                  <Button type="primary" htmlType="submit" loading={tushareSaving}>
                    保存
                  </Button>
                  <Button
                    danger
                    disabled={tushareSaving}
                    onClick={async () => {
                      setTushareSaving(true);
                      try {
                        await setTushareToken(null);
                        const res = await getTushareTokenStatus();
                        setTushareStatus((res.data ?? {}) as TushareTokenStatus);
                        message.success("Tushare Token 已清空");
                        tushareForm.resetFields();
                      } catch (error: any) {
                        const msg = error?.response?.data?.error || "清空 Tushare Token 失败";
                        message.error(msg);
                      } finally {
                        setTushareSaving(false);
                      }
                    }}
                  >
                    清空
                  </Button>
                </Space>
              </Form>
            </Space>
          )}
        </Card>

      <Card style={{ marginBottom: 16 }}>
        <Title level={3} style={{ marginTop: 0 }}>
          账号信息
        </Title>

        {profileLoading ? (
          <div style={{ padding: "16px 0", display: "flex", justifyContent: "center" }}>
            <Spin />
          </div>
        ) : profileError ? (
          <Result
            status="warning"
            title="信息加载失败"
            subTitle={profileError}
            extra={[
              <Button
                key="retry"
                onClick={() => {
                  setProfileNonce((v) => v + 1);
                }}
              >
                重试
              </Button>,
            ]}
          />
        ) : (
          <Space direction="vertical" size="large" style={{ width: "100%" }}>
            <Descriptions
              column={{ xs: 1, sm: 2, md: 3 }}
              items={[
                { key: "username", label: "用户名", children: me?.username ?? user?.username ?? "-" },
                { key: "role", label: "角色", children: me?.role ?? user?.role ?? "-" },
                { key: "email", label: "邮箱", children: me?.email ?? "-" },
                { key: "created_at", label: "创建时间", children: me?.created_at ?? "-" },
              ]}
            />

            {normalizedSummary ? (
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
                  gap: 12,
                }}
              >
                <Card size="small" styles={{ body: { padding: 12 } }}>
                  <Statistic title="账户数" value={normalizedSummary.account_count} />
                </Card>
                <Card size="small" styles={{ body: { padding: 12 } }}>
                  <Statistic title="持仓数" value={normalizedSummary.position_count} />
                </Card>
                <Card size="small" styles={{ body: { padding: 12 } }}>
                  <Statistic
                    title="总成本"
                    value={normalizedSummary.total_cost}
                    precision={2}
                    prefix="¥"
                  />
                </Card>
                <Card size="small" styles={{ body: { padding: 12 } }}>
                  <Statistic
                    title="总市值"
                    value={normalizedSummary.total_value}
                    precision={2}
                    prefix="¥"
                  />
                </Card>
                <Card size="small" styles={{ body: { padding: 12 } }}>
                  <Statistic
                    title="总盈亏"
                    value={normalizedSummary.total_pnl}
                    precision={2}
                    prefix="¥"
                    valueStyle={{
                      color: normalizedSummary.total_pnl >= 0 ? "#cf1322" : "#3f8600",
                    }}
                    suffix={
                      normalizedSummary.total_pnl_rate === null
                        ? ""
                        : ` (${normalizedSummary.total_pnl_rate >= 0 ? "+" : ""}${normalizedSummary.total_pnl_rate.toFixed(2)}%)`
                    }
                  />
                </Card>
              </div>
            ) : (
              <div
                style={{
                  border: `1px dashed ${token.colorBorder}`,
                  borderRadius: token.borderRadiusLG,
                  padding: 12,
                  color: token.colorTextSecondary,
                }}
              >
                暂无资产汇总数据
              </div>
            )}
          </Space>
        )}
      </Card>

      <Card>
        <Title level={3} style={{ marginTop: 0 }}>
          账号安全
        </Title>

        {done ? (
          <Result
            status="success"
            title="密码修改成功"
            subTitle="建议重新登录以确保所有设备上的登录状态安全。"
            extra={[
              <Button key="back" onClick={() => setDone(false)}>
                返回
              </Button>,
              <Button
                type="primary"
                key="relogin"
                onClick={() => {
                  logout();
                  window.location.href = "/login";
                }}
              >
                退出并重新登录
              </Button>,
            ]}
          />
        ) : (
          <Form form={form} layout="vertical" onFinish={onChangePassword} style={{ maxWidth: 520 }}>
            <Form.Item
              label="旧密码"
              name="old_password"
              rules={[{ required: true, message: "请输入旧密码" }]}
            >
              <Input.Password placeholder="请输入旧密码" autoComplete="current-password" />
            </Form.Item>

            <Form.Item
              label="新密码"
              name="new_password"
              rules={[
                { required: true, message: "请输入新密码" },
                { min: 8, message: "密码至少 8 个字符" },
              ]}
            >
              <Input.Password placeholder="至少 8 个字符" autoComplete="new-password" />
            </Form.Item>

            <Form.Item
              label="确认新密码"
              name="confirm_password"
              dependencies={["new_password"]}
              rules={[
                { required: true, message: "请确认新密码" },
                ({ getFieldValue }) => ({
                  validator(_, value) {
                    if (!value || getFieldValue("new_password") === value) {
                      return Promise.resolve();
                    }
                    return Promise.reject(new Error("两次输入的新密码不一致"));
                  },
                }),
              ]}
            >
              <Input.Password placeholder="重复输入新密码" autoComplete="new-password" />
            </Form.Item>

            <Space>
              <Button type="primary" htmlType="submit" loading={saving}>
                修改密码
              </Button>
              <Button
                onClick={() => {
                  form.resetFields();
                }}
                disabled={saving}
              >
                清空
              </Button>
            </Space>
          </Form>
        )}
      </Card>
    </AuthedLayout>
  );
}
