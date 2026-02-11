import { Button, Result } from 'antd';
import { useNavigate } from 'react-router-dom';

function DashboardPage() {
  const navigate = useNavigate();

  const handleLogout = () => {
    localStorage.clear();
    navigate('/login');
  };

  return (
    <div style={{ padding: '50px' }}>
      <Result
        status="success"
        title="欢迎使用 Fundval 基金估值系统"
        subTitle="系统初始化完成，功能开发中..."
        extra={[
          <Button type="primary" key="logout" onClick={handleLogout}>
            退出登录
          </Button>,
        ]}
      />
    </div>
  );
}

export default DashboardPage;
