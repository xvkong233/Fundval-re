"use client";

import {
  AppstoreOutlined,
  DashboardOutlined,
  FundOutlined,
  LineChartOutlined,
  MenuFoldOutlined,
  MenuUnfoldOutlined,
  SettingOutlined,
  StarOutlined,
  UnorderedListOutlined,
  WalletOutlined,
} from "@ant-design/icons";
import { Button, Drawer, Grid, Layout, Menu, Spin, Typography } from "antd";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import React, { useEffect, useMemo, useState } from "react";
import { useAuth } from "../contexts/AuthContext";
import { isAuthenticated } from "../lib/auth";

const { Header, Sider, Content } = Layout;
const { Text } = Typography;
const { useBreakpoint } = Grid;

const FV_SIDER_PREF_KEY = "fv_sider_collapsed";

function readSiderCollapsedPreference(): boolean | null {
  if (typeof window === "undefined") return null;
  const raw = window.localStorage.getItem(FV_SIDER_PREF_KEY);
  if (raw === "true") return true;
  if (raw === "false") return false;
  return null;
}

function writeSiderCollapsedPreference(collapsed: boolean) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(FV_SIDER_PREF_KEY, collapsed ? "true" : "false");
}

type NavKey =
  | "dashboard"
  | "accounts"
  | "positions"
  | "watchlists"
  | "sniffer"
  | "strategies"
  | "funds"
  | "sim"
  | "settings"
  | "tasks";

const NAV_ROUTES: Record<NavKey, string> = {
  dashboard: "/dashboard",
  accounts: "/accounts",
  positions: "/positions",
  watchlists: "/watchlists",
  sniffer: "/sniffer",
  strategies: "/strategies/compare",
  funds: "/funds",
  sim: "/sim",
  settings: "/settings",
  tasks: "/tasks",
};

export function AuthedLayout({
  title,
  subtitle,
  extra,
  children,
}: {
  title?: React.ReactNode;
  subtitle?: React.ReactNode;
  extra?: React.ReactNode;
  children: React.ReactNode;
}) {
  const router = useRouter();
  const pathname = usePathname();
  const screens = useBreakpoint();
  const isMobile = !screens.md; // < 768px
  const isTablet = Boolean(screens.md) && !screens.lg; // 768px~992px

  const { user, logout, loading } = useAuth();
  const [clientAuthed, setClientAuthed] = useState<boolean | null>(null);

  const [collapsed, setCollapsed] = useState<boolean>(true);
  const [mobileNavOpen, setMobileNavOpen] = useState(false);

  useEffect(() => {
    // 不能在 SSR 阶段依赖 localStorage；这里用 client-side effect 决定是否渲染受保护内容。
    setClientAuthed(isAuthenticated());
  }, []);

  useEffect(() => {
    if (loading) return;
    if (!isAuthenticated()) router.replace("/login");
  }, [loading, router]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    // 首次加载：桌面端读取用户偏好；移动端/平板默认折叠（更干净）。
    if (isMobile || isTablet) {
      setCollapsed(true);
      return;
    }
    setCollapsed(readSiderCollapsedPreference() ?? false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") return;
    if (isMobile) {
      setCollapsed(true);
      setMobileNavOpen(false);
      return;
    }
    if (isTablet) {
      setCollapsed(true);
      return;
    }
    // 桌面端：切换回桌面时，恢复偏好。
    setCollapsed(readSiderCollapsedPreference() ?? false);
  }, [isMobile, isTablet, screens.lg]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    if (!isMobile && !isTablet) writeSiderCollapsedPreference(collapsed);
  }, [collapsed, isMobile, isTablet]);

  const selectedKeys = useMemo<NavKey[]>(() => {
    if (!pathname) return [];
    if (pathname.startsWith("/watchlists")) return ["watchlists"];
    if (pathname.startsWith("/accounts")) return ["accounts"];
    if (pathname.startsWith("/positions")) return ["positions"];
    if (pathname.startsWith("/settings")) return ["settings"];
    if (pathname.startsWith("/dashboard")) return ["dashboard"];
    if (pathname.startsWith("/sniffer")) return ["sniffer"];
    if (pathname.startsWith("/sim")) return ["sim"];
    if (pathname.startsWith("/funds")) return ["funds"];
    if (pathname.startsWith("/tasks")) return ["tasks"];
    if (pathname.startsWith("/strategies")) return ["strategies"];
    return [];
  }, [pathname]);

  const primaryNavItems = useMemo(
    () => [
      { key: "dashboard", icon: <DashboardOutlined />, label: "仪表盘" },
      { key: "accounts", icon: <WalletOutlined />, label: "账户" },
      { key: "positions", icon: <FundOutlined />, label: "持仓" },
      { key: "watchlists", icon: <StarOutlined />, label: "自选" },
      { key: "sniffer", icon: <AppstoreOutlined />, label: "嗅探" },
      { key: "strategies", icon: <LineChartOutlined />, label: "策略" },
      { key: "funds", icon: <FundOutlined />, label: "基金" },
      { key: "sim", icon: <FundOutlined />, label: "模拟盘" },
      { key: "settings", icon: <SettingOutlined />, label: "设置" },
    ],
    []
  );

  const bottomNavItems = useMemo(() => [{ key: "tasks", icon: <UnorderedListOutlined />, label: "任务队列" }], []);

  const go = (key: string) => {
    const k = key as NavKey;
    const url = NAV_ROUTES[k];
    if (url) router.push(url);
    if (isMobile) setMobileNavOpen(false);
  };

  const renderNav = (opts?: { compact?: boolean }) => (
    <>
      <div className="fv-siderBrand">
        <Link href="/dashboard" style={{ color: "inherit" }} onClick={() => isMobile && setMobileNavOpen(false)}>
          Fundval
        </Link>
      </div>
      <div className="fv-siderMenu">
        <Menu
          theme="dark"
          mode="inline"
          selectedKeys={selectedKeys}
          items={primaryNavItems as any}
          onClick={(e) => go(String(e.key))}
          inlineCollapsed={Boolean(opts?.compact)}
        />
      </div>
      <div className="fv-siderBottom">
        <Menu
          theme="dark"
          mode="inline"
          selectedKeys={selectedKeys}
          items={bottomNavItems as any}
          onClick={(e) => go(String(e.key))}
          inlineCollapsed={Boolean(opts?.compact)}
        />
      </div>
    </>
  );

  return (
    <Layout className="fv-shell">
      {!isMobile ? (
        <Sider
          className="fv-sider"
          width={240}
          collapsedWidth={72}
          collapsible
          trigger={null}
          collapsed={collapsed}
          onCollapse={(v) => setCollapsed(v)}
        >
          {renderNav({ compact: collapsed })}
        </Sider>
      ) : null}

      <Layout className="fv-main">
        <Header className="fv-header">
          <div className="fv-headerLeft">
            <Button
              aria-label={isMobile ? "打开导航" : collapsed ? "展开侧边栏" : "收起侧边栏"}
              icon={isMobile ? <MenuUnfoldOutlined /> : collapsed ? <MenuUnfoldOutlined /> : <MenuFoldOutlined />}
              onClick={() => {
                if (isMobile) setMobileNavOpen(true);
                else setCollapsed((v) => !v);
              }}
              style={{ flexShrink: 0 }}
            />

            <div className="fv-headerTitleWrap">
              <div className="fv-titleRow">
                <div className="fv-title" title={typeof title === "string" ? title : undefined}>
                  {title ?? "Fundval"}
                </div>
                {subtitle ? <div className="fv-subtitle">{subtitle}</div> : null}
              </div>
            </div>

            {!isMobile && extra ? <div className="fv-headerExtra">{extra}</div> : null}
          </div>

          <div className="fv-headerRight">
            {!isMobile ? <Text type="secondary">{user?.username ? `你好，${user.username}` : ""}</Text> : null}
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

        <Content className="fv-content">
          {loading || clientAuthed !== true ? (
            <div className="fv-loading">
              <Spin />
            </div>
          ) : (
            <div className="fv-page fv-pagePad fv-pageBody">
              {isMobile && extra ? <div className="fv-mobileActions">{extra}</div> : null}
              {children}
            </div>
          )}
        </Content>
      </Layout>

      <Drawer
        placement="left"
        width={300}
        open={mobileNavOpen}
        onClose={() => setMobileNavOpen(false)}
        title={<span className="fv-drawerTitle">Fundval</span>}
        styles={{
          header: {
            background: "#001529",
            borderBottom: "1px solid rgba(255,255,255,0.08)",
          },
          body: { padding: 0, background: "#001529" },
        }}
      >
        <div className="fv-drawerNav">{renderNav({ compact: false })}</div>
      </Drawer>
    </Layout>
  );
}
