"use client";

import { Button, Card, Descriptions, Form, Input, Result, Space, Spin, Statistic, Typography, message, theme } from "antd";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { AuthedLayout } from "../../components/AuthedLayout";
import { changePassword, getCurrentUser, getMySummary } from "../../lib/api";
import { getChangePasswordErrorMessage } from "../../lib/changePassword";
import { useAuth } from "../../contexts/AuthContext";
import { normalizeUserSummary } from "../../lib/userSummary";

const { Paragraph, Text, Title } = Typography;

type ChangePasswordValues = {
  old_password: string;
  new_password: string;
  confirm_password: string;
};

export default function SettingsPage() {
  const { logout, user, updateUser } = useAuth();
  const [saving, setSaving] = useState(false);
  const [done, setDone] = useState(false);
  const [form] = Form.useForm<ChangePasswordValues>();

  const [profileLoading, setProfileLoading] = useState(true);
  const [profileError, setProfileError] = useState<string | null>(null);
  const [me, setMe] = useState<any | null>(null);
  const [summary, setSummary] = useState<any | null>(null);
  const [profileNonce, setProfileNonce] = useState(0);

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
        </Space>
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
