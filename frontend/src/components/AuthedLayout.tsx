"use client";

import {
  AppstoreOutlined,
  BellOutlined,
  DashboardOutlined,
  ExperimentOutlined,
  FundOutlined,
  LineChartOutlined,
  MenuFoldOutlined,
  MenuUnfoldOutlined,
  SettingOutlined,
  StarOutlined,
  UnorderedListOutlined,
  WalletOutlined,
} from "@ant-design/icons";
import { Avatar, Button, Drawer, Grid, Layout, Menu, Spin } from "antd";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import React, { useEffect, useMemo, useState } from "react";
import { useAuth } from "../contexts/AuthContext";
import { isAuthenticated } from "../lib/auth";

const { Header, Sider, Content } = Layout;
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

const NAV_SECTIONS: { title: string; keys: NavKey[] }[] = [
  { title: "总览", keys: ["dashboard"] },
  { title: "资产", keys: ["accounts", "positions", "watchlists"] },
  { title: "研究", keys: ["funds", "sniffer", "strategies", "sim"] },
  { title: "系统", keys: ["tasks", "settings"] },
];

const NAV_META: Record<NavKey, { icon: React.ReactNode; label: string }> = {
  dashboard: { icon: <DashboardOutlined />, label: "仪表盘" },
  accounts: { icon: <WalletOutlined />, label: "账户" },
  positions: { icon: <FundOutlined />, label: "持仓" },
  watchlists: { icon: <StarOutlined />, label: "自选" },
  funds: { icon: <LineChartOutlined />, label: "基金" },
  sniffer: { icon: <AppstoreOutlined />, label: "嗅探" },
  strategies: { icon: <ExperimentOutlined />, label: "策略" },
  sim: { icon: <FundOutlined />, label: "模拟盘" },
  tasks: { icon: <UnorderedListOutlined />, label: "任务队列" },
  settings: { icon: <SettingOutlined />, label: "设置" },
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
  const isMobile = !screens.md;

  const { user, logout, loading } = useAuth();
  const [clientAuthed, setClientAuthed] = useState<boolean | null>(null);
  const [collapsed, setCollapsed] = useState(false);
  const [mobileNavOpen, setMobileNavOpen] = useState(false);

  useEffect(() => {
    setClientAuthed(isAuthenticated());
  }, []);

  useEffect(() => {
    if (loading) return;
    if (!isAuthenticated()) router.replace("/login");
  }, [loading, router]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    if (isMobile) {
      setCollapsed(true);
      setMobileNavOpen(false);
      return;
    }
    setCollapsed(readSiderCollapsedPreference() ?? false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isMobile]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    if (!isMobile) writeSiderCollapsedPreference(collapsed);
  }, [collapsed, isMobile]);

  const selectedKeys = useMemo<NavKey[]>(() => {
    if (!pathname) return [];
    for (const k of Object.keys(NAV_ROUTES) as NavKey[]) {
      if (k === "strategies" && pathname.startsWith("/strategies")) return [k];
      if (k !== "strategies" && (pathname === NAV_ROUTES[k] || pathname.startsWith(NAV_ROUTES[k] + "/")))
        return [k];
    }
    return [];
  }, [pathname]);

  const go = (key: string) => {
    const url = NAV_ROUTES[key as NavKey];
    if (url) router.push(url);
    if (isMobile) setMobileNavOpen(false);
  };

  const menuItems = useMemo(() => {
    const items: any[] = [];
    for (const sec of NAV_SECTIONS) {
      items.push({
        key: `section-${sec.title}`,
        type: "group",
        label: collapsed ? undefined : <span className="fv-siderSection">{sec.title}</span>,
        children: sec.keys.map((k) => ({
          key: k,
          icon: NAV_META[k].icon,
          label: NAV_META[k].label,
        })),
      });
    }
    return items;
  }, [collapsed]);

  const renderNav = () => (
    <>
      <Link
        href="/dashboard"
        className="fv-siderBrand"
        style={{ textDecoration: "none" }}
        onClick={() => isMobile && setMobileNavOpen(false)}
      >
        <span className="fv-siderBrandMark">F</span>
        {!collapsed && (
          <span>
            <span className="fv-siderBrandName">Fundval</span>
            <div className="fv-siderBrandSub">基金智能分析</div>
          </span>
        )}
      </Link>
      <div className="fv-siderMenu">
        <Menu
          mode="inline"
          selectedKeys={selectedKeys}
          items={menuItems}
          onClick={(e) => go(String(e.key))}
          inlineCollapsed={collapsed}
          style={{ borderRight: 0, background: "transparent" }}
        />
      </div>
      <div className="fv-siderUser">
        <Avatar style={{ background: "#4F46E5", flexShrink: 0 }}>
          {(user?.username ?? "U").slice(0, 1).toUpperCase()}
        </Avatar>
        {!collapsed && (
          <div style={{ minWidth: 0, flex: 1 }}>
            <div style={{ fontWeight: 600, fontSize: 13, overflow: "hidden", textOverflow: "ellipsis" }}>
              {user?.username ?? "用户"}
            </div>
            <Button
              type="link"
              size="small"
              style={{ padding: 0, height: "auto", fontSize: 12 }}
              onClick={() => {
                logout();
                router.push("/login");
              }}
            >
              退出登录
            </Button>
          </div>
        )}
      </div>
    </>
  );

  return (
    <Layout className="fv-shell">
      {!isMobile && (
        <Sider
          className="fv-sider"
          width={248}
          collapsedWidth={76}
          collapsible
          trigger={null}
          collapsed={collapsed}
        >
          {renderNav()}
        </Sider>
      )}

      <Layout className="fv-main">
        <Header className="fv-header">
          <div className="fv-headerLeft">
            <Button
              type="text"
              aria-label={isMobile ? "打开导航" : collapsed ? "展开侧边栏" : "收起侧边栏"}
              icon={isMobile ? <MenuUnfoldOutlined /> : collapsed ? <MenuUnfoldOutlined /> : <MenuFoldOutlined />}
              onClick={() => {
                if (isMobile) setMobileNavOpen(true);
                else setCollapsed((v) => !v);
              }}
            />
            <div style={{ minWidth: 0 }}>
              <div style={{ fontSize: 16, fontWeight: 700, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                {title ?? "Fundval"}
              </div>
              {subtitle ? (
                <div style={{ fontSize: 12, color: "var(--fv-muted)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                  {subtitle}
                </div>
              ) : null}
            </div>
          </div>
          <div className="fv-headerRight">
            {extra}
            <Button type="text" icon={<BellOutlined />} aria-label="通知" />
          </div>
        </Header>

        <Content className="fv-content">
          {loading || clientAuthed !== true ? (
            <div className="fv-loading">
              <Spin size="large" />
            </div>
          ) : (
            <div className="fv-page">{children}</div>
          )}
        </Content>
      </Layout>

      <Drawer
        placement="left"
        width={280}
        open={mobileNavOpen}
        onClose={() => setMobileNavOpen(false)}
        styles={{ body: { padding: 0 } }}
        title={null}
        closable={false}
      >
        <div className="fv-drawerNav">{renderNav()}</div>
      </Drawer>
    </Layout>
  );
}
