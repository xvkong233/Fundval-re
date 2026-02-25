"use client";

import { useState } from "react";
import {
  Button,
  Divider,
  Form,
  Input,
  Result,
  Space,
  Steps,
  Switch,
  Typography,
  theme,
} from "antd";
import {
  CheckCircleOutlined,
  KeyOutlined,
  LockOutlined,
  SafetyCertificateOutlined,
  UserAddOutlined,
  UserOutlined,
} from "@ant-design/icons";
import { useRouter } from "next/navigation";
import { PublicShell } from "../../components/PublicShell";
import { initializeSystem, verifyBootstrapKey } from "../../lib/api";
import type { BootstrapInitErrorInfo } from "../../lib/bootstrapInit";
import { getBootstrapInitError, maskBootstrapKey } from "../../lib/bootstrapInit";

const { Text, Paragraph } = Typography;

type VerifyFormValues = { bootstrap_key: string };
type InitFormValues = {
  admin_username: string;
  admin_password: string;
  confirm_password: string;
  allow_register: boolean;
};

export default function InitializePage() {
  const router = useRouter();
  const [currentStep, setCurrentStep] = useState(0);
  const [bootstrapKey, setBootstrapKey] = useState("");
  const [loading, setLoading] = useState(false);
  const [blockingError, setBlockingError] = useState<BootstrapInitErrorInfo | null>(null);
  const [form] = Form.useForm<InitFormValues>();
  const { token } = theme.useToken();

  const onVerifyKey = async (values: VerifyFormValues) => {
    setLoading(true);
    setBlockingError(null);
    try {
      await verifyBootstrapKey(values.bootstrap_key);
      setBootstrapKey(values.bootstrap_key);
      setCurrentStep(1);
    } catch (error: any) {
      const info = getBootstrapInitError(error);
      if (info.kind === "already_initialized" || info.kind === "network") {
        setBlockingError(info);
        return;
      }
      (await import("antd")).message.error(info.message);
    } finally {
      setLoading(false);
    }
  };

  const onInitialize = async (values: InitFormValues) => {
    setLoading(true);
    setBlockingError(null);
    try {
      await initializeSystem({
        bootstrap_key: bootstrapKey,
        admin_username: values.admin_username,
        admin_password: values.admin_password,
        allow_register: values.allow_register,
      });
      setCurrentStep(2);
    } catch (error: any) {
      const info = getBootstrapInitError(error);
      if (info.kind === "already_initialized" || info.kind === "network") {
        setBlockingError(info);
        return;
      }
      (await import("antd")).message.error(info.message);
    } finally {
      setLoading(false);
    }
  };

  const handleGoLogin = () => router.push("/login");
  const handleGoServerConfig = () => router.push("/server-config");
  const handleBackToKey = () => {
    setBootstrapKey("");
    setCurrentStep(0);
    setBlockingError(null);
    form.resetFields();
  };

  return (
    <PublicShell title="系统初始化" subtitle="请完成必要的配置以启动服务" maxWidth={760}>
      {blockingError ? (
        <Result
          status={blockingError.kind === "already_initialized" ? "warning" : "error"}
          title={blockingError.kind === "already_initialized" ? "系统已初始化" : "无法连接到服务器"}
          subTitle={
            blockingError.kind === "already_initialized"
              ? "检测到系统已完成初始化，Bootstrap 接口已失效。请直接前往登录页。"
              : blockingError.message
          }
          extra={[
            <Button type="primary" key="login" onClick={handleGoLogin} block size="large">
              前往登录页
            </Button>,
            <Button key="server-config" onClick={handleGoServerConfig} block size="large" disabled={loading}>
              查看服务器配置说明
            </Button>,
            <Button key="retry" onClick={() => setBlockingError(null)} block size="large">
              返回重试
            </Button>,
          ]}
        />
      ) : (
        <>
          <Steps
            current={currentStep}
            size="small"
            style={{ marginBottom: 20 }}
            items={[
              { title: "验证身份", icon: <KeyOutlined /> },
              { title: "管理员配置", icon: <UserAddOutlined /> },
              { title: "完成", icon: <CheckCircleOutlined /> },
            ]}
          />

          {currentStep === 0 ? (
            <>
              <div
                style={{
                  background: token.colorPrimaryBg,
                  border: `1px solid ${token.colorPrimaryBorder}`,
                  padding: 14,
                  borderRadius: token.borderRadiusLG,
                  marginBottom: 16,
                }}
              >
                <Space align="start">
                  <SafetyCertificateOutlined style={{ fontSize: 18, color: token.colorPrimary, marginTop: 2 }} />
                  <div>
                    <Text strong>安全验证</Text>
                    <Paragraph type="secondary" style={{ marginBottom: 0, fontSize: 13 }}>
                      为了确保安全，请输入服务器启动日志中生成的 <b>Bootstrap Key</b>。
                    </Paragraph>
                  </div>
                </Space>
              </div>

              <Form onFinish={onVerifyKey} layout="vertical" size="large">
                <Form.Item name="bootstrap_key" rules={[{ required: true, message: "请输入 Bootstrap Key" }]}>
                  <Input.TextArea rows={4} placeholder="请输入密钥..." style={{ resize: "none" }} className="fv-mono" />
                </Form.Item>
                <Form.Item style={{ marginBottom: 0 }}>
                  <Space direction="vertical" style={{ width: "100%" }} size={12}>
                    <Button type="primary" htmlType="submit" loading={loading} block size="large" icon={<CheckCircleOutlined />}>
                      验证并继续
                    </Button>
                    <Button onClick={handleGoServerConfig} block size="large" disabled={loading}>
                      查看服务器配置说明
                    </Button>
                  </Space>
                </Form.Item>
              </Form>
            </>
          ) : null}

          {currentStep === 1 ? (
            <>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  gap: 12,
                  padding: 12,
                  background: "rgba(15, 23, 42, 0.03)",
                  border: "1px solid rgba(15, 23, 42, 0.08)",
                  borderRadius: token.borderRadiusLG,
                  marginBottom: 16,
                }}
              >
                <div style={{ minWidth: 0 }}>
                  <div style={{ fontWeight: 600, marginBottom: 2 }}>已验证 Bootstrap Key</div>
                  <Text type="secondary" className="fv-mono" style={{ fontSize: 12 }}>
                    {maskBootstrapKey(bootstrapKey) || "（空）"}
                  </Text>
                </div>
                <Button onClick={handleBackToKey} disabled={loading}>
                  返回修改
                </Button>
              </div>

              <Form form={form} onFinish={onInitialize} layout="vertical" initialValues={{ allow_register: false }} size="large">
                <Form.Item label="管理员用户名" name="admin_username" rules={[{ required: true, message: "请设置管理员用户名" }]}>
                  <Input prefix={<UserOutlined style={{ color: "rgba(0,0,0,.25)" }} />} placeholder="例如: admin" />
                </Form.Item>

                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
                  <Form.Item
                    label="设置密码"
                    name="admin_password"
                    rules={[
                      { required: true, message: "请设置密码" },
                      { min: 8, message: "密码至少 8 位" },
                    ]}
                  >
                    <Input.Password prefix={<LockOutlined style={{ color: "rgba(0,0,0,.25)" }} />} placeholder="至少 8 位" />
                  </Form.Item>

                  <Form.Item
                    label="确认密码"
                    name="confirm_password"
                    dependencies={["admin_password"]}
                    rules={[
                      { required: true, message: "请确认密码" },
                      ({ getFieldValue }) => ({
                        validator(_, value) {
                          if (!value || getFieldValue("admin_password") === value) return Promise.resolve();
                          return Promise.reject(new Error("两次输入的密码不一致"));
                        },
                      }),
                    ]}
                  >
                    <Input.Password prefix={<LockOutlined style={{ color: "rgba(0,0,0,.25)" }} />} placeholder="重复密码" />
                  </Form.Item>
                </div>

                <Divider style={{ margin: "8px 0 16px" }} />

                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    gap: 12,
                    padding: 12,
                    background: "rgba(15, 23, 42, 0.03)",
                    borderRadius: token.borderRadiusLG,
                    marginBottom: 16,
                  }}
                >
                  <div>
                    <div style={{ fontWeight: 600 }}>开放注册</div>
                    <div style={{ fontSize: 12, color: "rgba(15, 23, 42, 0.62)" }}>是否允许其他人注册账户</div>
                  </div>
                  <Form.Item name="allow_register" valuePropName="checked" noStyle>
                    <Switch />
                  </Form.Item>
                </div>

                <Form.Item style={{ marginBottom: 0 }}>
                  <Space direction="vertical" style={{ width: "100%" }} size={12}>
                    <Button type="primary" htmlType="submit" loading={loading} block size="large">
                      完成初始化
                    </Button>
                    <Button onClick={handleBackToKey} block disabled={loading}>
                      返回修改密钥
                    </Button>
                  </Space>
                </Form.Item>
              </Form>

              <style jsx>{`
                @media (max-width: 768px) {
                  div[style*="grid-template-columns: 1fr 1fr"] {
                    grid-template-columns: 1fr !important;
                  }
                }
              `}</style>
            </>
          ) : null}

          {currentStep === 2 ? (
            <Result
              status="success"
              title="系统初始化成功！"
              subTitle="管理员账户已创建。请使用管理员账号前往登录页登录。"
              extra={[
                <Button type="primary" key="login" onClick={handleGoLogin} block size="large">
                  前往登录页
                </Button>,
              ]}
            />
          ) : null}
        </>
      )}
    </PublicShell>
  );
}

