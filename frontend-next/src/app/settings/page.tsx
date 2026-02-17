"use client";

import { Button, Card, Form, Input, Space, Typography, message, Result } from "antd";
import Link from "next/link";
import { useState } from "react";
import { AuthedLayout } from "../../components/AuthedLayout";
import { changePassword } from "../../lib/api";
import { getChangePasswordErrorMessage } from "../../lib/changePassword";
import { useAuth } from "../../contexts/AuthContext";

const { Paragraph, Text, Title } = Typography;

type ChangePasswordValues = {
  old_password: string;
  new_password: string;
  confirm_password: string;
};

export default function SettingsPage() {
  const { logout } = useAuth();
  const [saving, setSaving] = useState(false);
  const [done, setDone] = useState(false);
  const [form] = Form.useForm<ChangePasswordValues>();

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
