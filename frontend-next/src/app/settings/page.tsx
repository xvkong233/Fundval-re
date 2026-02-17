"use client";

import { Button, Card, Space, Typography } from "antd";
import Link from "next/link";
import { AuthedLayout } from "../../components/AuthedLayout";

const { Paragraph, Text, Title } = Typography;

export default function SettingsPage() {
  return (
    <AuthedLayout title="设置">
      <Card>
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
    </AuthedLayout>
  );
}

