import { Button, Layout, Menu, Space, Tag, Typography } from "antd";
import {
  ClusterOutlined,
  DashboardOutlined,
  ExportOutlined,
  LogoutOutlined,
  SettingOutlined,
  SwapOutlined,
  TeamOutlined,
  ThunderboltOutlined,
} from "@ant-design/icons";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { useAuth } from "../auth";

const { Sider, Header, Content } = Layout;

const items = [
  { key: "/", icon: <DashboardOutlined />, label: "仪表盘" },
  { key: "/inbounds", icon: <ClusterOutlined />, label: "入站管理" },
  { key: "/clients", icon: <TeamOutlined />, label: "客户端" },
  { key: "/outbound", icon: <ExportOutlined />, label: "出站管理" },
  { key: "/routing", icon: <SwapOutlined />, label: "路由规则" },
  { key: "/xray", icon: <ThunderboltOutlined />, label: "Xray" },
  { key: "/settings", icon: <SettingOutlined />, label: "面板设置" },
];

export default function AppLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const { username, logout } = useAuth();

  const selected =
    items.find(
      (item) => item.key !== "/" && location.pathname.startsWith(item.key),
    )?.key ?? "/";

  return (
    <Layout className="app-shell">
      <Sider width={208} theme="dark" breakpoint="lg" collapsedWidth={0}>
        <div className="brand">Xray Panel</div>
        <Menu
          theme="dark"
          mode="inline"
          selectedKeys={[selected]}
          items={items}
          onClick={({ key }) => navigate(key)}
        />
      </Sider>
      <Layout>
        <Header className="app-header">
          <Typography.Text strong>控制台</Typography.Text>
          <Space>
            <Tag color="blue">{username}</Tag>
            <Button
              icon={<LogoutOutlined />}
              onClick={async () => {
                await logout();
                navigate("/login", { replace: true });
              }}
            >
              退出
            </Button>
          </Space>
        </Header>
        <Content className="app-content">
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  );
}
