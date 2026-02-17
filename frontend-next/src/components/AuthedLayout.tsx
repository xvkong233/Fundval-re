"use client";

import { Button, Layout, Menu, Typography } from "antd";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import React, { useMemo } from "react";
import { useAuth } from "../contexts/AuthContext";

const { Header, Sider, Content } = Layout;
const { Text } = Typography;

export function AuthedLayout({
  title,
  children,
}: {
  title?: React.ReactNode;
  children: React.ReactNode;
}) {
  const router = useRouter();
  const pathname = usePathname();
  const { user, logout } = useAuth();

  const selectedKeys = useMemo(() => {
    if (!pathname) return [];
    if (pathname.startsWith("/funds")) return ["funds"];
    if (pathname.startsWith("/watchlists")) return ["watchlists"];
    if (pathname.startsWith("/accounts")) return ["accounts"];
    if (pathname.startsWith("/positions")) return ["positions"];
    if (pathname.startsWith("/dashboard")) return ["dashboard"];
    return [];
  }, [pathname]);

  return (
    <Layout style={{ minHeight: "100vh" }}>
      <Sider breakpoint="lg" collapsedWidth="0">
        <div style={{ padding: 16, color: "white", fontWeight: 600 }}>Fundval</div>
        <Menu
          theme="dark"
          mode="inline"
          selectedKeys={selectedKeys}
          items={[
            {
              key: "dashboard",
              label: <Link href="/dashboard">仪表盘</Link>,
            },
            {
              key: "funds",
              label: <Link href="/funds">基金</Link>,
            },
            {
              key: "accounts",
              label: <Link href="/accounts">账户</Link>,
            },
            {
              key: "positions",
              label: <Link href="/positions">持仓</Link>,
            },
            {
              key: "watchlists",
              label: <Link href="/watchlists">自选</Link>,
            },
          ]}
        />
      </Sider>

      <Layout>
        <Header
          style={{
            background: "#fff",
            padding: "0 16px",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: 12,
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 12, minWidth: 0 }}>
            <div style={{ fontWeight: 600, whiteSpace: "nowrap" }}>{title ?? "Fundval"}</div>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <Text type="secondary">{user?.username ? `你好，${user.username}` : ""}</Text>
            <Button
              onClick={() => {
                logout();
                router.push("/login");
              }}
            >
              退出
            </Button>
          </div>
        </Header>

        <Content style={{ padding: 16, background: "#f0f2f5" }}>{children}</Content>
      </Layout>
    </Layout>
  );
}

