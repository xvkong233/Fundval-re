"use client";

import dynamic from "next/dynamic";
import {
  Button,
  Card,
  Descriptions,
  Empty,
  Space,
  Spin,
  Statistic,
  Table,
  Typography,
  message,
  theme,
} from "antd";
import { useEffect, useMemo, useState } from "react";
import { useParams } from "next/navigation";
import { AuthedLayout } from "../../../components/AuthedLayout";
import { getFundDetail, getFundEstimate, listNavHistory, syncNavHistory } from "../../../lib/api";
import { getDateRange, type TimeRange } from "../../../lib/dateRange";
import { buildNavChartOption } from "../../../lib/navChart";

const { Text } = Typography;

type NavRow = Record<string, any> & { nav_date?: string; unit_nav?: string; accum_nav?: string };

const ReactECharts = dynamic(() => import("echarts-for-react"), { ssr: false });

export default function FundDetailPage() {
  const params = useParams<{ fundCode: string }>();
  const fundCode = decodeURIComponent(params?.fundCode ?? "");

  const [loading, setLoading] = useState(true);
  const [fund, setFund] = useState<any | null>(null);
  const [estimate, setEstimate] = useState<any | null>(null);

  const [navLoading, setNavLoading] = useState(false);
  const [navHistory, setNavHistory] = useState<NavRow[]>([]);
  const [timeRange, setTimeRange] = useState<TimeRange>("1M");
  const [compactChart, setCompactChart] = useState(false);

  const { token } = theme.useToken();

  const title = useMemo(() => {
    if (!fund) return "基金详情";
    return (
      <Space direction="vertical" size={0}>
        <span>
          {fund.fund_name}（{fund.fund_code}）
        </span>
        <Text type="secondary" style={{ fontSize: 12 }}>
          {fund.fund_type || ""}
        </Text>
      </Space>
    );
  }, [fund]);

  const loadBase = async () => {
    setLoading(true);
    try {
      const [detailRes, estimateRes] = await Promise.all([
        getFundDetail(fundCode),
        getFundEstimate(fundCode).catch(() => null),
      ]);
      setFund(detailRes.data);
      setEstimate(estimateRes?.data ?? null);
    } catch {
      message.error("加载基金详情失败");
      setFund(null);
      setEstimate(null);
    } finally {
      setLoading(false);
    }
  };

  const syncAndLoadNav = async (range: TimeRange) => {
    setNavLoading(true);
    try {
      const now = new Date();
      const { startDate, endDate } = getDateRange(range, now);

      // 同步失败不阻断展示（与旧前端一致）
      try {
        await syncNavHistory([fundCode], startDate, endDate);
      } catch {
        // ignore
      }

      const params = range === "ALL" ? {} : { start_date: startDate };
      const res = await listNavHistory(fundCode, params);
      const rows = Array.isArray(res.data) ? (res.data as NavRow[]) : [];
      rows.sort((a, b) => String(a.nav_date).localeCompare(String(b.nav_date)));
      setNavHistory(rows);
    } catch {
      message.error("加载历史净值失败");
      setNavHistory([]);
    } finally {
      setNavLoading(false);
    }
  };

  useEffect(() => {
    void loadBase();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fundCode]);

  useEffect(() => {
    const update = () => setCompactChart(window.innerWidth < 768);
    update();
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

  if (loading) {
    return (
      <AuthedLayout title="基金详情">
        <Card>
          <div style={{ textAlign: "center", padding: "50px 0" }}>
            <Spin tip="加载中..." />
          </div>
        </Card>
      </AuthedLayout>
    );
  }

  if (!fund) {
    return (
      <AuthedLayout title="基金详情">
        <Card>
          <Empty description="基金不存在" />
        </Card>
      </AuthedLayout>
    );
  }

  const latestNav = fund.latest_nav ?? fund.yesterday_nav;
  const latestNavDate = fund.latest_nav_date ?? fund.yesterday_nav_date;

  return (
    <AuthedLayout title={title}>
      <Space direction="vertical" size="large" style={{ width: "100%" }}>
        <Card title="基金信息">
          <Descriptions column={{ xs: 1, sm: 2, md: 3 }}>
            <Descriptions.Item label="基金代码">{fund.fund_code}</Descriptions.Item>
            <Descriptions.Item label="基金名称">{fund.fund_name}</Descriptions.Item>
            <Descriptions.Item label="基金类型">{fund.fund_type || "-"}</Descriptions.Item>
          </Descriptions>

          <div style={{ marginTop: 16, display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 16 }}>
            <Statistic
              title="最新净值"
              value={latestNav || "-"}
              precision={latestNav ? 4 : 0}
              prefix={latestNav ? "¥" : ""}
              suffix={latestNavDate ? ` (${String(latestNavDate).slice(5)})` : ""}
            />
            <Statistic
              title="实时估值"
              value={estimate?.estimate_nav || estimate?.estimate_value || "-"}
              precision={estimate?.estimate_nav || estimate?.estimate_value ? 4 : 0}
              prefix={estimate?.estimate_nav || estimate?.estimate_value ? "¥" : ""}
            />
            <Statistic
              title="估算涨跌"
              value={estimate?.estimate_growth || estimate?.estimate_growth_rate || "-"}
              precision={estimate?.estimate_growth || estimate?.estimate_growth_rate ? 2 : 0}
              suffix={estimate?.estimate_growth || estimate?.estimate_growth_rate ? "%" : ""}
              valueStyle={{
                color:
                  Number(estimate?.estimate_growth ?? estimate?.estimate_growth_rate) >= 0 ? "#cf1322" : "#3f8600",
              }}
              prefix={
                Number(estimate?.estimate_growth ?? estimate?.estimate_growth_rate) >= 0 ? "+" : ""
              }
            />
          </div>
        </Card>

        <Card
          title="历史净值"
          extra={
            <Space wrap>
              {(["1W", "1M", "3M", "6M", "1Y", "ALL"] as TimeRange[]).map((range) => (
                <Button
                  key={range}
                  size="small"
                  type={timeRange === range ? "primary" : "default"}
                  onClick={() => setTimeRange(range)}
                >
                  {range === "ALL" ? "全部" : range === "1W" ? "1周" : range}
                </Button>
              ))}
              <Button
                size="small"
                loading={navLoading}
                onClick={() => void syncAndLoadNav(timeRange)}
              >
                同步并加载
              </Button>
            </Space>
          }
        >
          {navHistory.length > 0 ? (
            <div style={{ marginBottom: 16 }}>
              <ReactECharts
                option={buildNavChartOption(navHistory, { compact: compactChart, color: token.colorPrimary })}
                style={{ height: compactChart ? 300 : 400 }}
              />
            </div>
          ) : null}
          <Table<NavRow>
            rowKey={(r) => `${r.nav_date ?? ""}`}
            loading={navLoading}
            dataSource={navHistory}
            pagination={{ pageSize: 20 }}
            locale={{ emptyText: "暂无数据（可点击右上角“同步并加载”）" }}
            columns={[
              { title: "日期", dataIndex: "nav_date", width: 140 },
              { title: "单位净值", dataIndex: "unit_nav", render: (v: any) => (v ? Number(v).toFixed(4) : "-") },
              { title: "累计净值", dataIndex: "accum_nav", render: (v: any) => (v ? Number(v).toFixed(4) : "-") },
            ]}
          />
        </Card>
      </Space>
    </AuthedLayout>
  );
}

