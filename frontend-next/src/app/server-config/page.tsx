"use client";

import type { CSSProperties } from "react";
import { Button, Card, Layout, Typography, theme } from "antd";
import { CloudServerOutlined, HomeOutlined, SettingOutlined } from "@ant-design/icons";
import Link from "next/link";

const { Title, Paragraph, Text } = Typography;
const { Content, Footer } = Layout;

export default function ServerConfigPage() {
  const { token } = theme.useToken();

  const layoutStyle: CSSProperties = {
    minHeight: "100vh",
    display: "flex",
    flexDirection: "column",
    justifyContent: "center",
    background: "#f0f2f5",
  };

  const cardStyle: CSSProperties = {
    width: "100%",
    maxWidth: 560,
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
        <div style={{ textAlign: "center", marginBottom: 32 }}>
          <div style={{ display: "flex", justifyContent: "center" }}>
            <div style={logoBoxStyle}>
              <CloudServerOutlined style={{ fontSize: 24, color: "#fff" }} />
            </div>
          </div>
          <Title level={2} style={{ marginBottom: 0 }}>
            服务器配置
          </Title>
          <Text type="secondary">Next.js Web 版本说明</Text>
        </div>

        <Card style={cardStyle} styles={{ body: { padding: 32 } }}>
          <Paragraph style={{ marginTop: 0 }}>
            当前 <Text strong>Next.js Web 版本</Text>默认通过同源 <Text code>/api</Text> 反向代理访问后端，
            一般不需要在浏览器里配置服务器地址。
          </Paragraph>

          <Paragraph type="secondary">
            如需切换后端地址，请在部署阶段调整 <Text code>API_PROXY_TARGET</Text>（或 Docker Compose 变量）。
          </Paragraph>

          <Paragraph type="secondary" style={{ marginBottom: 0 }}>
            提示：桌面端/移动端的“自定义服务器地址”能力属于原项目的原生容器逻辑，本仓库的 Web 迁移版暂不实现该运行时切换。
          </Paragraph>

          <div style={{ marginTop: 24, display: "flex", gap: 12, flexWrap: "wrap" }}>
            <Link href="/" prefetch={false}>
              <Button icon={<HomeOutlined />}>返回首页</Button>
            </Link>
            <Link href="/settings" prefetch={false}>
              <Button icon={<SettingOutlined />}>前往设置</Button>
            </Link>
          </div>
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

