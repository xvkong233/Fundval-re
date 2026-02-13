import { useState, useEffect } from 'react';
import { Card, Form, Input, Button, message, Space, Divider } from 'antd';
import { SaveOutlined, ReloadOutlined, CloudServerOutlined } from '@ant-design/icons';
import { isNativeApp } from '../App';

const SettingsPage = () => {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);
  const isNative = isNativeApp();

  // 从 localStorage 加载配置
  useEffect(() => {
    if (isNative) {
      const savedApiUrl = localStorage.getItem('apiBaseUrl') || '';
      form.setFieldsValue({
        apiBaseUrl: savedApiUrl
      });
    }
  }, [form, isNative]);

  // 保存配置
  const handleSave = async (values) => {
    setLoading(true);
    try {
      // 验证 URL 格式
      const url = values.apiBaseUrl.trim();
      if (!url.startsWith('http://') && !url.startsWith('https://')) {
        message.error('服务器地址必须以 http:// 或 https:// 开头');
        setLoading(false);
        return;
      }

      // 移除末尾的斜杠
      const cleanUrl = url.replace(/\/$/, '');

      // 测试连接
      const response = await fetch(`${cleanUrl}/api/health/`, {
        method: 'GET',
        headers: { 'Content-Type': 'application/json' },
      });

      if (response.ok) {
        // 保存到 localStorage
        localStorage.setItem('apiBaseUrl', cleanUrl);
        message.success('配置已保存，刷新页面后生效');
      } else {
        message.error('无法连接到服务器，请检查地址是否正确');
      }
    } catch (error) {
      message.error(`连接失败: ${error.message}`);
    } finally {
      setLoading(false);
    }
  };

  // 重置为默认值
  const handleReset = () => {
    form.setFieldsValue({
      apiBaseUrl: ''
    });
    message.info('已清空服务器配置');
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
      {isNative ? (
        <>
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
              extra="后端 API 服务器地址，例如：http://192.168.1.100:8000"
            >
              <Input
                prefix={<CloudServerOutlined />}
                placeholder="http://your-server:8000"
              />
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
                  icon={<ReloadOutlined />}
                  onClick={handleReset}
                >
                  清空配置
                </Button>
              </Space>
            </Form.Item>
          </Form>
        </>
      ) : (
        <div style={{ padding: '20px 0' }}>
          <p>Web 版本无需配置服务器地址。</p>
          <p>如需修改服务器，请使用桌面端或移动端应用。</p>
        </div>
      )}
    </Card>
  );
};

export default SettingsPage;
