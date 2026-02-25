"use client";

import { useEffect, useState } from "react";
import { Button, Form, Input, message, Typography, Result, Spin } from "antd";
import { CloudServerOutlined, LockOutlined, UserAddOutlined, UserOutlined } from "@ant-design/icons";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { PublicShell } from "../../components/PublicShell";
import { register } from "../../lib/api";
import { setToken } from "../../lib/auth";
import { useAuth } from "../../contexts/AuthContext";
import { isAuthenticated } from "../../lib/auth";
import { shouldRedirectAuthedPublicPage } from "../../lib/entryRouting";

const { Text } = Typography;

type RegisterValues = { username: string; password: string; password_confirm: string };

export default function RegisterPage() {
  const router = useRouter();
  const [loading, setLoading] = useState(false);
  const [checking, setChecking] = useState(true);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [checkNonce, setCheckNonce] = useState(0);
  const [registerClosed, setRegisterClosed] = useState(false);
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
        const res = await fetch("/api/health", { headers: { Accept: "application/json" } });
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

  const onFinish = async (values: RegisterValues) => {
    setLoading(true);
    setRegisterClosed(false);
    try {
      const response = await register(values.username, values.password, values.password_confirm);
      const { access_token, refresh_token, user } = response.data as any;

      setToken(access_token, refresh_token);
      authLogin(user);
      message.success(`注册成功，欢迎 ${user?.username ?? values.username}！`);

      router.push("/dashboard");
    } catch (error: any) {
      if (error?.response?.status === 403) {
        setRegisterClosed(true);
        return;
      }

      const errorMsg =
        error?.response?.data?.error ||
        error?.response?.data?.username?.[0] ||
        error?.response?.data?.password?.[0] ||
        error?.response?.data?.password_confirm?.[0] ||
        "注册失败";
      message.error(errorMsg);
    } finally {
      setLoading(false);
    }
  };

  return (
    <PublicShell
      title={
        <span style={{ display: "inline-flex", alignItems: "center", gap: 10 }}>
          <CloudServerOutlined />
          注册
        </span>
      }
      subtitle="创建账号以开始使用（如管理员已关闭注册则无法注册）"
      maxWidth={460}
    >
      {checking ? (
        <div style={{ textAlign: "center", padding: "24px 0" }}>
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
      ) : registerClosed ? (
        <Result
          status="warning"
          title="注册未开放"
          subTitle="管理员在初始化时关闭了注册功能。如需账号，请联系管理员创建，或使用已有账号登录。"
          extra={[
            <Button type="primary" key="login" onClick={() => router.push("/login")}>
              前往登录页
            </Button>,
            <Button key="retry" onClick={() => setRegisterClosed(false)}>
              返回重试
            </Button>,
          ]}
        />
      ) : (
        <Form name="register" onFinish={onFinish} autoComplete="off" layout="vertical" size="large">
              <Form.Item
                name="username"
                rules={[
                  { required: true, message: "请输入用户名" },
                  { min: 3, message: "用户名至少 3 个字符" },
                  { max: 150, message: "用户名最多 150 个字符" },
                ]}
              >
                <Input prefix={<UserOutlined style={{ color: "rgba(0,0,0,.25)" }} />} placeholder="用户名" />
              </Form.Item>

              <Form.Item
                name="password"
                rules={[
                  { required: true, message: "请输入密码" },
                  { min: 8, message: "密码至少 8 个字符" },
                ]}
              >
                <Input.Password
                  prefix={<LockOutlined style={{ color: "rgba(0,0,0,.25)" }} />}
                  placeholder="密码"
                />
              </Form.Item>

              <Form.Item
                name="password_confirm"
                dependencies={["password"]}
                rules={[
                  { required: true, message: "请确认密码" },
                  ({ getFieldValue }) => ({
                    validator(_, value) {
                      if (!value || getFieldValue("password") === value) {
                        return Promise.resolve();
                      }
                      return Promise.reject(new Error("两次密码不一致"));
                    },
                  }),
                ]}
              >
                <Input.Password
                  prefix={<LockOutlined style={{ color: "rgba(0,0,0,.25)" }} />}
                  placeholder="确认密码"
                />
              </Form.Item>

              <Form.Item style={{ marginBottom: 16 }}>
                <Button
                  type="primary"
                  htmlType="submit"
                  loading={loading}
                  block
                  size="large"
                  icon={<UserAddOutlined />}
                >
                  注册
                </Button>
              </Form.Item>

              <div style={{ textAlign: "center" }}>
                <Text type="secondary">
                  已有账号？ <Link href="/login">立即登录</Link>
                </Text>
              </div>
            </Form>
      )}
    </PublicShell>
  );
}
