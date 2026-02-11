import { useState, useEffect } from 'react';
import { Card, Table, Input, Button, Space, message, Typography } from 'antd';
import { useNavigate } from 'react-router-dom';
import { SearchOutlined, EyeOutlined, StarOutlined, ReloadOutlined } from '@ant-design/icons';
import { Resizable } from 'react-resizable';
import 'react-resizable/css/styles.css';
import { fundsAPI } from '../api';

const { Text } = Typography;

// 可调整大小的表头组件
const ResizableTitle = (props) => {
  const { onResize, width, ...restProps } = props;

  if (!width) {
    return <th {...restProps} />;
  }

  return (
    <Resizable
      width={width}
      height={0}
      handle={
        <span
          className="react-resizable-handle"
          onClick={(e) => e.stopPropagation()}
        />
      }
      onResize={onResize}
      draggableOpts={{ enableUserSelectHack: false }}
    >
      <th {...restProps} />
    </Resizable>
  );
};

const FundsPage = () => {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [estimateLoading, setEstimateLoading] = useState(false);
  const [funds, setFunds] = useState([]);
  const [estimates, setEstimates] = useState({});
  const [total, setTotal] = useState(0);
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(1);
  const [lastUpdateTime, setLastUpdateTime] = useState(null);
  const pageSize = 10;

  // 列宽状态
  const [columnWidths, setColumnWidths] = useState({
    fund_code: 80,
    fund_name: window.innerWidth < 768 ? 150 : 200,
    latest_nav: 110,
    estimate_nav: 90,
    estimate_growth: 80,
    action: 80,
  });

  // 处理列宽调整
  const handleResize = (key) => (e, { size }) => {
    setColumnWidths((prev) => ({
      ...prev,
      [key]: size.width,
    }));
  };

  // 加载基金列表
  const loadFunds = async (searchValue = search, pageNum = page) => {
    setLoading(true);
    try {
      const response = await fundsAPI.list({
        search: searchValue,
        page: pageNum,
        page_size: pageSize,
      });
      setFunds(response.data.results);
      setTotal(response.data.count);

      // 加载估值数据
      await loadEstimates(response.data.results);
    } catch (error) {
      message.error('加载基金列表失败');
    } finally {
      setLoading(false);
    }
  };

  // 加载估值数据
  const loadEstimates = async (fundList) => {
    if (!fundList || fundList.length === 0) return;

    setEstimateLoading(true);
    try {
      const fundCodes = fundList.map((f) => f.fund_code);
      const response = await fundsAPI.batchEstimate(fundCodes);
      setEstimates(response.data);
      setLastUpdateTime(new Date());
    } catch (error) {
      console.error('获取估值数据失败:', error);
      message.error('获取估值数据失败');
    } finally {
      setEstimateLoading(false);
    }
  };

  // 刷新估值
  const handleRefreshEstimates = async () => {
    if (funds.length === 0) return;
    await loadEstimates(funds);
    message.success('估值数据已刷新');
  };

  useEffect(() => {
    loadFunds();
  }, []);

  const handleSearch = (value) => {
    setSearch(value);
    setPage(1);
    loadFunds(value, 1);
  };

  const handlePageChange = (pageNum) => {
    setPage(pageNum);
    loadFunds(search, pageNum);
  };

  const handleViewDetail = (fundCode) => {
    navigate(`/dashboard/funds/${fundCode}`);
  };

  const handleAddToWatchlist = (fund) => {
    message.info('加入自选功能待实现');
  };

  const columns = [
    {
      title: '代码',
      dataIndex: 'fund_code',
      key: 'fund_code',
      width: columnWidths.fund_code,
      responsive: ['sm'],
      resizable: true,
      onHeaderCell: (column) => ({
        width: column.width,
        onResize: handleResize('fund_code'),
      }),
    },
    {
      title: '基金名称',
      dataIndex: 'fund_name',
      key: 'fund_name',
      width: columnWidths.fund_name,
      ellipsis: true,
      resizable: true,
      onHeaderCell: (column) => ({
        width: column.width,
        onResize: handleResize('fund_name'),
      }),
    },
    {
      title: '最新净值',
      dataIndex: 'latest_nav',
      key: 'latest_nav',
      width: columnWidths.latest_nav,
      resizable: true,
      onHeaderCell: (column) => ({
        width: column.width,
        onResize: handleResize('latest_nav'),
      }),
      render: (nav, record) => {
        if (!nav) return '-';

        const date = record.latest_nav_date;
        const dateStr = date ? `(${date.slice(5)})` : '';

        return (
          <span style={{ whiteSpace: 'nowrap' }}>
            {Number(nav).toFixed(4)}
            <Text type="secondary" style={{ fontSize: '11px', marginLeft: '2px' }}>
              {dateStr}
            </Text>
          </span>
        );
      },
    },
    {
      title: '实时估值',
      dataIndex: 'fund_code',
      key: 'estimate_nav',
      width: columnWidths.estimate_nav,
      responsive: ['lg'],
      resizable: true,
      onHeaderCell: (column) => ({
        width: column.width,
        onResize: handleResize('estimate_nav'),
      }),
      render: (code) => {
        const estimate = estimates[code];
        if (!estimate) return '-';
        if (estimate.error) return <Text type="secondary">-</Text>;
        return estimate.estimate_nav ? Number(estimate.estimate_nav).toFixed(4) : '-';
      },
    },
    {
      title: '涨跌',
      dataIndex: 'fund_code',
      key: 'estimate_growth',
      width: columnWidths.estimate_growth,
      responsive: ['md'],
      resizable: true,
      onHeaderCell: (column) => ({
        width: column.width,
        onResize: handleResize('estimate_growth'),
      }),
      render: (code) => {
        const estimate = estimates[code];
        if (!estimate || !estimate.estimate_growth) return '-';

        const growth = parseFloat(estimate.estimate_growth);
        const color = growth >= 0 ? '#cf1322' : '#3f8600';
        const prefix = growth >= 0 ? '+' : '';

        return (
          <Text strong style={{ color, fontSize: '13px' }}>
            {prefix}{growth.toFixed(2)}%
          </Text>
        );
      },
    },
    {
      title: '操作',
      key: 'action',
      width: columnWidths.action,
      fixed: 'right',
      render: (_, record) => (
        <Space size="small">
          <Button
            type="link"
            size="small"
            icon={<EyeOutlined />}
            onClick={() => handleViewDetail(record.fund_code)}
          />
          <Button
            type="link"
            size="small"
            icon={<StarOutlined />}
            onClick={() => handleAddToWatchlist(record)}
          />
        </Space>
      ),
    },
  ];

  return (
    <Card
      title="基金列表"
      extra={
        <Space>
          {lastUpdateTime && (
            <Text type="secondary" style={{ fontSize: '12px' }}>
              估值更新时间: {lastUpdateTime.toLocaleTimeString()}
            </Text>
          )}
          <Button
            icon={<ReloadOutlined />}
            onClick={handleRefreshEstimates}
            loading={estimateLoading}
            size="small"
          >
            刷新估值
          </Button>
        </Space>
      }
    >
      <Space style={{ width: '100%', marginBottom: 16 }}>
        <Input.Search
          placeholder="搜索基金名称或代码"
          allowClear
          enterButton={<SearchOutlined />}
          size="large"
          onSearch={handleSearch}
          onChange={(e) => {
            if (!e.target.value) {
              handleSearch('');
            }
          }}
          style={{ width: '100%' }}
        />
      </Space>

      <Table
        columns={columns}
        dataSource={funds}
        rowKey="fund_code"
        loading={loading || estimateLoading}
        scroll={{ x: 'max-content' }}
        components={{
          header: {
            cell: ResizableTitle,
          },
        }}
        pagination={{
          current: page,
          pageSize: pageSize,
          total: total,
          onChange: handlePageChange,
          showSizeChanger: false,
          showTotal: (total) => `共 ${total} 条`,
          simple: window.innerWidth < 768,
        }}
      />
    </Card>
  );
};

export default FundsPage;
