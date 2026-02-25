"use client";

import type { CSSProperties } from "react";
import React from "react";
import { Card, Layout, Typography, theme } from "antd";
import Link from "next/link";

const { Content, Footer } = Layout;
const { Title, Text } = Typography;

export function PublicShell({
  title,
  subtitle,
  children,
  maxWidth = 520,
}: {
  title: React.ReactNode;
  subtitle?: React.ReactNode;
  children: React.ReactNode;
  maxWidth?: number;
}) {
  const { token } = theme.useToken();

  const layoutStyle: CSSProperties = {
    minHeight: "100vh",
    display: "flex",
    flexDirection: "column",
    justifyContent: "center",
    background:
      "radial-gradient(1200px circle at 10% 10%, rgba(30,64,175,0.16), transparent 50%), radial-gradient(1000px circle at 90% 20%, rgba(245,158,11,0.14), transparent 45%), #f8fafc",
  };

  const cardStyle: CSSProperties = {
    width: "100%",
    maxWidth,
    margin: "0 auto",
    borderRadius: token.borderRadiusLG,
    border: "1px solid rgba(15, 23, 42, 0.10)",
    boxShadow: "0 18px 50px rgba(15, 23, 42, 0.10)",
    background: "rgba(255,255,255,0.86)",
    backdropFilter: "blur(10px)",
  };

  return (
    <Layout style={layoutStyle}>
      <Content
        style={{
          padding: "24px 16px",
          display: "flex",
          flexDirection: "column",
          justifyContent: "center",
        }}
      >
        <div style={{ textAlign: "center", marginBottom: 20 }}>
          <Title level={2} style={{ marginBottom: 0, lineHeight: 1.2 }}>
            {title}
          </Title>
          {subtitle ? <Text type="secondary">{subtitle}</Text> : null}
        </div>

        <Card style={cardStyle} styles={{ body: { padding: 28 } }}>
          {children}
        </Card>
      </Content>

      <Footer style={{ textAlign: "center", background: "transparent", paddingTop: 0 }}>
        <Text type="secondary" style={{ fontSize: 12 }}>
          <Link href="/" style={{ color: "inherit" }}>
            Fundval
          </Link>{" "}
          &copy; 2026
        </Text>
      </Footer>
    </Layout>
  );
}

