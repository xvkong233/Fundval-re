"use client";

import { Button, Card, Input, Space, Typography, message } from "antd";
import { useState } from "react";
import { AuthedLayout } from "../../components/AuthedLayout";
import { createWatchlist, listWatchlists } from "../../lib/api";

const { Title, Paragraph, Text } = Typography;

export default function WatchlistsPage() {
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);

  const handleCreate = async () => {
    if (!name.trim()) {
      message.error("请输入自选列表名称");
      return;
    }
    setLoading(true);
    try {
      await createWatchlist(name.trim());
      message.success("创建成功");
      setName("");
    } catch (error: any) {
      const msg = error?.response?.data?.error || "创建失败";
      message.error(msg);
    } finally {
      setLoading(false);
    }
  };

  const handleRefresh = async () => {
    setLoading(true);
    try {
      const res = await listWatchlists();
      const count = Array.isArray(res.data) ? res.data.length : 0;
      message.success(`已加载 ${count} 个自选列表`);
    } catch (error: any) {
      const msg = error?.response?.data?.error || "加载失败";
      message.error(msg);
    } finally {
      setLoading(false);
    }
  };

  return (
    <AuthedLayout title="自选列表">
      <Card>
        <Title level={3} style={{ marginTop: 0 }}>
          自选列表（开发中）
        </Title>
        <Paragraph type="secondary">
          当前仅提供“创建自选列表”能力，用于支持基金列表页的“添加到自选”弹窗。
        </Paragraph>

        <Space direction="vertical" style={{ width: "100%" }} size="middle">
          <div>
            <Text type="secondary">新建自选列表</Text>
          </div>
          <Space wrap style={{ width: "100%" }}>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="例如：我的自选"
              style={{ width: 260 }}
            />
            <Button type="primary" loading={loading} onClick={() => void handleCreate()}>
              创建
            </Button>
            <Button loading={loading} onClick={() => void handleRefresh()}>
              刷新
            </Button>
          </Space>
        </Space>
      </Card>
    </AuthedLayout>
  );
}

