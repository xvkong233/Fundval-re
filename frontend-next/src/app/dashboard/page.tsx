"use client";

import { Button, Card, Layout, Typography } from "antd";
import { useRouter } from "next/navigation";
import { useAuth } from "../../contexts/AuthContext";

const { Content } = Layout;
const { Title, Text, Paragraph } = Typography;

export default function DashboardPage() {
  const router = useRouter();
  const { user, logout } = useAuth();

  const handleLogout = () => {
    logout();
    router.push("/login");
  };

  return (
    <Layout style={{ minHeight: "100vh", background: "#f0f2f5" }}>
      <Content style={{ padding: 24, maxWidth: 960, margin: "0 auto", width: "100%" }}>
        <Card>
          <Title level={3} style={{ marginTop: 0 }}>
            仪表盘（开发中）
          </Title>
          <Paragraph type="secondary">
            已完成 Next.js 端的初始化/登录/注册最小闭环；下一步将迁移基金/持仓/账户等页面。
          </Paragraph>
          <div style={{ display: "flex", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
            <Text>
              当前用户：<b>{user?.username ?? "未知"}</b>
            </Text>
            <Button onClick={handleLogout}>退出登录</Button>
          </div>
        </Card>
      </Content>
    </Layout>
  );
}
