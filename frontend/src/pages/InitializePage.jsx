import { useState, useEffect } from 'react';
import { Form, Input, Button, Card, message, Switch } from 'antd';
import { useNavigate } from 'react-router-dom';
import { verifyBootstrapKey, initializeSystem } from '../api';

function InitializePage() {
  const navigate = useNavigate();
  const [step, setStep] = useState(1); // 1: 验证 key, 2: 创建管理员
  const [bootstrapKey, setBootstrapKey] = useState('');
  const [loading, setLoading] = useState(false);

  const onVerifyKey = async (values) => {
    setLoading(true);
    try {
      await verifyBootstrapKey(values.bootstrap_key);
      setBootstrapKey(values.bootstrap_key);
      setStep(2);
      message.success('密钥验证成功');
    } catch (error) {
      message.error(error.response?.data?.error || '密钥无效');
    } finally {
      setLoading(false);
    }
  };

  const onInitialize = async (values) => {
    setLoading(true);
    try {
      await initializeSystem({
        bootstrap_key: bootstrapKey,
        admin_username: values.admin_username,
        admin_password: values.admin_password,
        allow_register: values.allow_register,
      });
      message.success('系统初始化成功！');
      setTimeout(() => navigate('/login'), 1500);
    } catch (error) {
      message.error(error.response?.data?.error || '初始化失败');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', minHeight: '100vh', background: '#f0f2f5' }}>
      <Card title="系统初始化" style={{ width: 500 }}>
        {step === 1 ? (
          <Form onFinish={onVerifyKey} layout="vertical">
            <Form.Item
              label="Bootstrap Key"
              name="bootstrap_key"
              rules={[{ required: true, message: '请输入 Bootstrap Key' }]}
              extra="请从服务器日志中获取 Bootstrap Key"
            >
              <Input.TextArea rows={3} />
            </Form.Item>
            <Form.Item>
              <Button type="primary" htmlType="submit" loading={loading} block>
                验证密钥
              </Button>
            </Form.Item>
          </Form>
        ) : (
          <Form onFinish={onInitialize} layout="vertical" initialValues={{ allow_register: false }}>
            <Form.Item
              label="管理员用户名"
              name="admin_username"
              rules={[{ required: true, message: '请输入管理员用户名' }]}
            >
              <Input />
            </Form.Item>
            <Form.Item
              label="管理员密码"
              name="admin_password"
              rules={[
                { required: true, message: '请输入管理员密码' },
                { min: 8, message: '密码至少 8 位' },
              ]}
            >
              <Input.Password />
            </Form.Item>
            <Form.Item
              label="开放注册"
              name="allow_register"
              valuePropName="checked"
            >
              <Switch />
            </Form.Item>
            <Form.Item>
              <Button type="primary" htmlType="submit" loading={loading} block>
                完成初始化
              </Button>
            </Form.Item>
          </Form>
        )}
      </Card>
    </div>
  );
}

export default InitializePage;
