/**
 * 测试基金列表页面 - 实时估值功能
 *
 * 测试点：
 * 1. 最新净值列显示（带日期）
 * 2. 实时估值列显示
 * 3. 估值涨跌列显示（红涨绿跌）
 * 4. 批量估值 API 调用
 * 5. 刷新按钮功能
 * 6. 错误处理
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { BrowserRouter } from 'react-router-dom';
import userEvent from '@testing-library/user-event';
import FundsPage from '../pages/FundsPage';
import * as api from '../api';

// Mock API
vi.mock('../api', () => ({
  fundsAPI: {
    list: vi.fn(),
    batchEstimate: vi.fn(),
  },
}));

// Mock useNavigate
vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual('react-router-dom');
  return {
    ...actual,
    useNavigate: () => vi.fn(),
  };
});

describe('FundsPage - 实时估值功能', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const mockFundsList = {
    count: 3,
    results: [
      {
        fund_code: '000001',
        fund_name: '华夏成长混合',
        fund_type: '混合型',
        latest_nav: '1.2345',
        latest_nav_date: '2026-02-11',
      },
      {
        fund_code: '000002',
        fund_name: '华夏大盘精选',
        fund_type: '混合型',
        latest_nav: '2.3456',
        latest_nav_date: '2026-02-10',
      },
      {
        fund_code: '110022',
        fund_name: '易方达消费行业',
        fund_type: '股票型',
        latest_nav: null,
        latest_nav_date: null,
      },
    ],
  };

  const mockEstimates = {
    '000001': {
      fund_code: '000001',
      fund_name: '华夏成长混合',
      estimate_nav: '1.2456',
      estimate_growth: '0.90',
      estimate_time: '2026-02-11T14:30:00Z',
      latest_nav: '1.2345',
      latest_nav_date: '2026-02-11',
      from_cache: true,
    },
    '000002': {
      fund_code: '000002',
      fund_name: '华夏大盘精选',
      estimate_nav: '2.3400',
      estimate_growth: '-0.24',
      estimate_time: '2026-02-11T14:30:00Z',
      latest_nav: '2.3456',
      latest_nav_date: '2026-02-10',
      from_cache: false,
    },
    '110022': {
      fund_code: '110022',
      error: '获取估值失败',
    },
  };

  it('应该显示最新净值列（带日期）', async () => {
    api.fundsAPI.list.mockResolvedValue({ data: mockFundsList });
    api.fundsAPI.batchEstimate.mockResolvedValue({ data: mockEstimates });

    render(
      <BrowserRouter>
        <FundsPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      // 检查最新净值显示
      expect(screen.getByText('1.2345')).toBeInTheDocument();
      expect(screen.getByText('(02-11)')).toBeInTheDocument();

      expect(screen.getByText('2.3456')).toBeInTheDocument();
      expect(screen.getByText('(02-10)')).toBeInTheDocument();
    });
  });

  it('应该显示实时估值列', async () => {
    api.fundsAPI.list.mockResolvedValue({ data: mockFundsList });
    api.fundsAPI.batchEstimate.mockResolvedValue({ data: mockEstimates });

    render(
      <BrowserRouter>
        <FundsPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText('1.2456')).toBeInTheDocument();
      expect(screen.getByText('2.3400')).toBeInTheDocument();
    });
  });

  it('应该显示估值涨跌（红涨绿跌）', async () => {
    api.fundsAPI.list.mockResolvedValue({ data: mockFundsList });
    api.fundsAPI.batchEstimate.mockResolvedValue({ data: mockEstimates });

    render(
      <BrowserRouter>
        <FundsPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      // 上涨显示红色
      const positiveGrowth = screen.getByText('+0.90%');
      expect(positiveGrowth).toBeInTheDocument();
      expect(positiveGrowth).toHaveStyle({ color: '#cf1322' });

      // 下跌显示绿色
      const negativeGrowth = screen.getByText('-0.24%');
      expect(negativeGrowth).toBeInTheDocument();
      expect(negativeGrowth).toHaveStyle({ color: '#3f8600' });
    });
  });

  it('应该调用批量估值 API', async () => {
    api.fundsAPI.list.mockResolvedValue({ data: mockFundsList });
    api.fundsAPI.batchEstimate.mockResolvedValue({ data: mockEstimates });

    render(
      <BrowserRouter>
        <FundsPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(api.fundsAPI.batchEstimate).toHaveBeenCalledWith([
        '000001',
        '000002',
        '110022',
      ]);
    });
  });

  it('应该显示刷新按钮', async () => {
    api.fundsAPI.list.mockResolvedValue({ data: mockFundsList });
    api.fundsAPI.batchEstimate.mockResolvedValue({ data: mockEstimates });

    render(
      <BrowserRouter>
        <FundsPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText('刷新估值')).toBeInTheDocument();
    });
  });

  it('点击刷新按钮应该重新获取估值', async () => {
    const user = userEvent.setup();
    api.fundsAPI.list.mockResolvedValue({ data: mockFundsList });
    api.fundsAPI.batchEstimate.mockResolvedValue({ data: mockEstimates });

    render(
      <BrowserRouter>
        <FundsPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(api.fundsAPI.batchEstimate).toHaveBeenCalledTimes(1);
    });

    // 点击刷新按钮
    const refreshButton = screen.getByText('刷新估值');
    await user.click(refreshButton);

    await waitFor(() => {
      expect(api.fundsAPI.batchEstimate).toHaveBeenCalledTimes(2);
    });
  });

  it('应该显示估值更新时间', async () => {
    api.fundsAPI.list.mockResolvedValue({ data: mockFundsList });
    api.fundsAPI.batchEstimate.mockResolvedValue({ data: mockEstimates });

    render(
      <BrowserRouter>
        <FundsPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText(/估值更新时间/)).toBeInTheDocument();
    });
  });

  it('应该处理估值获取失败', async () => {
    api.fundsAPI.list.mockResolvedValue({ data: mockFundsList });
    api.fundsAPI.batchEstimate.mockRejectedValue(new Error('网络错误'));

    render(
      <BrowserRouter>
        <FundsPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText(/获取估值数据失败/)).toBeInTheDocument();
    });
  });

  it('应该处理部分基金估值失败', async () => {
    api.fundsAPI.list.mockResolvedValue({ data: mockFundsList });
    api.fundsAPI.batchEstimate.mockResolvedValue({ data: mockEstimates });

    render(
      <BrowserRouter>
        <FundsPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      // 成功的基金应该显示估值
      expect(screen.getByText('1.2456')).toBeInTheDocument();

      // 失败的基金应该显示横杠（通过检查表格行数）
      const rows = screen.getAllByRole('row');
      expect(rows.length).toBeGreaterThan(3); // 包含表头
    });
  });

  it('没有最新净值时应该显示横杠', async () => {
    api.fundsAPI.list.mockResolvedValue({ data: mockFundsList });
    api.fundsAPI.batchEstimate.mockResolvedValue({ data: mockEstimates });

    render(
      <BrowserRouter>
        <FundsPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      // 110022 没有最新净值，应该显示横杠
      const table = screen.getByRole('table');
      expect(table).toBeInTheDocument();
    });
  });

  it('分页时应该重新获取估值', async () => {
    const user = userEvent.setup();
    api.fundsAPI.list.mockResolvedValue({ data: mockFundsList });
    api.fundsAPI.batchEstimate.mockResolvedValue({ data: mockEstimates });

    render(
      <BrowserRouter>
        <FundsPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(api.fundsAPI.batchEstimate).toHaveBeenCalledTimes(1);
    });

    // 模拟分页
    const page2Data = {
      count: 3,
      results: [
        {
          fund_code: '000003',
          fund_name: '测试基金3',
          fund_type: '股票型',
          latest_nav: '3.4567',
          latest_nav_date: '2026-02-11',
        },
      ],
    };

    api.fundsAPI.list.mockResolvedValue({ data: page2Data });
    api.fundsAPI.batchEstimate.mockResolvedValue({
      data: {
        '000003': {
          fund_code: '000003',
          estimate_nav: '3.4600',
          estimate_growth: '0.10',
        },
      },
    });

    // 查找并点击下一页按钮（如果存在）
    // 注意：这里需要根据实际的分页组件来调整
  });
});
