import { useState, useEffect } from 'react';
import { Card, Form, Input, Button, message, Space } from 'antd';
import { SaveOutlined, ReloadOutlined } from '@ant-design/icons';

const SettingsPage = () => {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);

  // 从 localStorage 加载配置
  useEffect(() => {
    const savedApiUrl = localStorage.getItem('apiBaseUrl') || 'http://localhost:8000';
    form.setFieldsValue({
      apiBaseUrl: savedApiUrl
    });
  }, [form]);

  // 保存配置
  const handleSave = async (values) => {
    setLoading(true);
    try {
      // 验证 URL 格式
      const url = values.apiBaseUrl.trim();
      if (!url.startsWith('http://') && !url.startsWith('https://')) {
        message.error('服务器地址必须以 http:// 或 https:// 开头');
        return;
      }

      // 移除末尾的斜杠
      const cleanUrl = url.replace(/\/$/, '');

      // 保存到 localStorage
      localStorage.setItem('apiBaseUrl', cleanUrl);

      message.success('配置已保存，刷新页面后生效');
    } catch (error) {
      message.error('保存失败');
    } finally {
      setLoading(false);
    }
  };

  // 重置为默认值
  const handleReset = () => {
    form.setFieldsValue({
      apiBaseUrl: 'http://localhost:8000'
    });
    message.info('已重置为默认值');
  };

  // 测试连接
  const handleTest = async () => {
    const url = form.getFieldValue('apiBaseUrl');
    if (!url) {
      message.error('请输入服务器地址');
      return;
    }

    setLoading(true);
    try {
      const cleanUrl = url.trim().replace(/\/$/, '');
      const response = await fetch(`${cleanUrl}/health/`);

      if (response.ok) {
        message.success('连接成功');
      } else {
        message.error('连接失败：服务器返回错误');
      }
    } catch (error) {
      message.error('连接失败：无法访问服务器');
    } finally {
      setLoading(false);
    }
  };

  return (
    <Card title="系统设置">
      <Form
        form={form}
        layout="vertical"
        onFinish={handleSave}
        style={{ maxWidth: 600 }}
      >
        <Form.Item
          label="服务器地址"
          name="apiBaseUrl"
          rules={[
            { required: true, message: '请输入服务器地址' },
            {
              pattern: /^https?:\/\/.+/,
              message: '请输入有效的 URL（以 http:// 或 https:// 开头）'
            }
          ]}
          extra="后端 API 服务器地址，例如：http://localhost:8000"
        >
          <Input placeholder="http://localhost:8000" />
        </Form.Item>

        <Form.Item>
          <Space>
            <Button
              type="primary"
              htmlType="submit"
              icon={<SaveOutlined />}
              loading={loading}
            >
              保存配置
            </Button>
            <Button
              onClick={handleTest}
              loading={loading}
            >
              测试连接
            </Button>
            <Button
              icon={<ReloadOutlined />}
              onClick={handleReset}
            >
              重置默认
            </Button>
          </Space>
        </Form.Item>
      </Form>

      <Card
        type="inner"
        title="说明"
        style={{ marginTop: 24, backgroundColor: '#f5f5f5' }}
      >
        <ul style={{ margin: 0, paddingLeft: 20 }}>
          <li>修改服务器地址后需要刷新页面才能生效</li>
          <li>默认地址为 http://localhost:8000</li>
          <li>如果服务器部署在其他地址，请修改此配置</li>
          <li>建议先点击"测试连接"确认服务器可访问</li>
        </ul>
      </Card>
    </Card>
  );
};

export default SettingsPage;
