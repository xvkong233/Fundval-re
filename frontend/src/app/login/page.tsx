"use client";

import type { CSSProperties } from "react";
import { useEffect, useState } from "react";
import { Button, Card, Form, Input, Layout, message, theme, Typography, Result, Spin } from "antd";
import { CloudServerOutlined, LockOutlined, LoginOutlined, UserOutlined } from "@ant-design/icons";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { login } from "../../lib/api";
import { setToken } from "../../lib/auth";
import { useAuth } from "../../contexts/AuthContext";
import { isAuthenticated } from "../../lib/auth";
import { shouldRedirectAuthedPublicPage } from "../../lib/entryRouting";

const { Title, Text } = Typography;
const { Content, Footer } = Layout;

type LoginValues = { username: string; password: string };

export default function LoginPage() {
  const router = useRouter();
  const [loading, setLoading] = useState(false);
  const [checking, setChecking] = useState(true);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [checkNonce, setCheckNonce] = useState(0);
  const { token } = theme.useToken();
  const { login: authLogin } = useAuth();

  useEffect(() => {
    let cancelled = false;

    async function run() {
      const authed = isAuthenticated();
      if (shouldRedirectAuthedPublicPage(authed)) {
        router.replace("/dashboard");
        return;
      }

      try {
        const res = await fetch("/api/health/", { headers: { Accept: "application/json" } });
        const data = (await res.json()) as any;
        if (cancelled) return;
        if (data?.system_initialized === false) {
          router.replace("/initialize");
          return;
        }
      } catch {
        if (cancelled) return;
        setHealthError("无法连接到服务器。请检查后端服务与 API_PROXY_TARGET 配置。");
      } finally {
        if (!cancelled) setChecking(false);
      }
    }

    void run();
    return () => {
      cancelled = true;
    };
  }, [router, checkNonce]);

  const onFinish = async (values: LoginValues) => {
    setLoading(true);
    try {
      const response = await login(values.username, values.password);
      const { access_token, refresh_token, user } = response.data as any;

      setToken(access_token, refresh_token);
      authLogin(user);
      message.success(`欢迎回来，${user?.username ?? values.username}！`);

      router.push("/dashboard");
    } catch (error: any) {
      message.error(error?.response?.data?.error || "登录失败");
    } finally {
      setLoading(false);
    }
  };

  const layoutStyle: CSSProperties = {
    minHeight: "100vh",
    display: "flex",
    flexDirection: "column",
    justifyContent: "center",
    background: "#f0f2f5",
  };

  const cardStyle: CSSProperties = {
    width: "100%",
    maxWidth: 450,
    margin: "0 auto",
    borderRadius: token.borderRadiusLG,
    boxShadow: "0 10px 25px rgba(0,0,0,0.08)",
  };

  const logoBoxStyle: CSSProperties = {
    width: 48,
    height: 48,
    background: token.colorPrimary,
    borderRadius: 12,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    marginBottom: 16,
    boxShadow: `0 4px 12px ${token.colorPrimary}40`,
  };

  return (
    <Layout style={layoutStyle}>
      <Content
        style={{
          padding: "20px",
          display: "flex",
          flexDirection: "column",
          justifyContent: "center",
        }}
      >
        <div style={{ textAlign: "center", marginBottom: 40 }}>
          <div style={{ display: "flex", justifyContent: "center" }}>
            <div style={logoBoxStyle}>
              <CloudServerOutlined style={{ fontSize: 24, color: "#fff" }} />
            </div>
          </div>
          <Title level={2} style={{ marginBottom: 0 }}>
            Fundval
          </Title>
          <Text type="secondary">基金估值与资产管理系统</Text>
        </div>

        <Card style={cardStyle} styles={{ body: { padding: 40 } }}>
          {checking ? (
            <div style={{ textAlign: "center", padding: "32px 0" }}>
              <Spin />
              <div style={{ marginTop: 12 }}>
                <Text type="secondary">正在检查系统状态...</Text>
              </div>
            </div>
          ) : healthError ? (
            <Result
              status="warning"
              title="服务器不可用"
              subTitle={healthError}
              extra={[
                <Button type="primary" key="server-config" onClick={() => router.push("/server-config")}>
                  查看服务器配置说明
                </Button>,
                <Button
                  key="retry"
                  onClick={() => {
                    setChecking(true);
                    setHealthError(null);
                    setCheckNonce((v) => v + 1);
                  }}
                >
                  重试
                </Button>,
              ]}
            />
          ) : (
            <Form name="login" onFinish={onFinish} autoComplete="off" layout="vertical" size="large">
              <Form.Item name="username" rules={[{ required: true, message: "请输入用户名" }]}>
                <Input prefix={<UserOutlined style={{ color: "rgba(0,0,0,.25)" }} />} placeholder="用户名" />
              </Form.Item>

              <Form.Item name="password" rules={[{ required: true, message: "请输入密码" }]}>
                <Input.Password
                  prefix={<LockOutlined style={{ color: "rgba(0,0,0,.25)" }} />}
                  placeholder="密码"
                />
              </Form.Item>

              <Form.Item style={{ marginBottom: 16 }}>
                <Button
                  type="primary"
                  htmlType="submit"
                  loading={loading}
                  block
                  size="large"
                  icon={<LoginOutlined />}
                >
                  登录
                </Button>
              </Form.Item>

              <div style={{ textAlign: "center" }}>
                <Text type="secondary">
                  还没有账号？ <Link href="/register">立即注册</Link>
                </Text>
              </div>
            </Form>
          )}
        </Card>
      </Content>

      <Footer style={{ textAlign: "center", background: "transparent" }}>
        <Text type="secondary" style={{ fontSize: 12 }}>
          &copy; 2026 Fundval. All rights reserved.
        </Text>
      </Footer>
    </Layout>
  );
}
