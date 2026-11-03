import { useEffect, useState } from "react";
import {
  Alert,
  App as AntApp,
  Button,
  Card,
  Col,
  Modal,
  Row,
  Space,
  Statistic,
  Typography,
} from "antd";
import { ReloadOutlined, ThunderboltOutlined } from "@ant-design/icons";
import { get, post } from "../api";
import type { DashboardData } from "../types";
import { formatBytes, formatUptime } from "../util";
import TrafficSparkline from "../components/TrafficSparkline";

export default function DashboardPage() {
  const { message, modal } = AntApp.useApp();
  const [data, setData] = useState<DashboardData>();
  const [loading, setLoading] = useState(false);
  const [logs, setLogs] = useState<string[]>([]);
  const [logsOpen, setLogsOpen] = useState(false);

  const load = async () => {
    setLoading(true);
    try {
      setData(await get<DashboardData>("/api/dashboard"));
    } catch (error) {
      message.error((error as Error).message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
    const timer = setInterval(() => {
      get<DashboardData>("/api/dashboard")
        .then(setData)
        .catch(() => {});
    }, 5000);
    return () => clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const restart = () => {
    modal.confirm({
      title: "重启 Xray？",
      content: "重启会短暂中断所有代理连接。",
      okText: "重启",
      cancelText: "取消",
      onOk: async () => {
        try {
          await post("/api/xray/restart");
          message.success("Xray 已重启");
          await load();
        } catch (error) {
          message.error((error as Error).message);
        }
      },
    });
  };

  const openLogs = async () => {
    try {
      const response = await get<{ lines: string[] }>(
        "/api/xray/logs?limit=300",
      );
      setLogs(response.lines ?? []);
      setLogsOpen(true);
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  if (!data) {
    return <Card loading={loading} />;
  }

  return (
    <div>
      <div className="page-title" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <Typography.Title level={4} style={{ margin: 0 }}>
          仪表盘
        </Typography.Title>
        <Space wrap>
          <Button icon={<ReloadOutlined />} onClick={load} loading={loading}>
            刷新
          </Button>
          <Button onClick={openLogs}>Xray 日志</Button>
          <Button icon={<ThunderboltOutlined />} onClick={restart}>
            重启 Xray
          </Button>
        </Space>
      </div>

      {data.xray.lastError && (
        <Alert
          type="warning"
          showIcon
          message="Xray 最近错误"
          description={<span className="word-break">{data.xray.lastError}</span>}
          style={{ marginBottom: 16 }}
        />
      )}

      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Xray 状态"
              value={data.xray.running ? "运行中" : "已停止"}
              valueStyle={{ color: data.xray.running ? "#3f8600" : "#cf1322" }}
            />
            <div className="stat-note">
              {data.xray.version ?? "版本未知"} · 重启 {data.xray.restarts} 次
            </div>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic title="CPU" value={`${data.system.cpuPercent.toFixed(1)}%`} />
            <div className="stat-note">
              负载 {data.system.load1.toFixed(2)} / {data.system.load5.toFixed(2)} /{" "}
              {data.system.load15.toFixed(2)}
            </div>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic title="内存" value={`${data.system.memPercent.toFixed(1)}%`} />
            <div className="stat-note">
              {formatBytes(data.system.memUsed)} / {formatBytes(data.system.memTotal)}
            </div>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic title="系统运行时间" value={formatUptime(data.system.uptime)} />
            <div className="stat-note">{data.system.hostname || "localhost"}</div>
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={12} md={6}>
          <Card>
            <Statistic title="入站" value={data.inbounds} />
          </Card>
        </Col>
        <Col xs={12} md={6}>
          <Card>
            <Statistic title="客户端" value={data.clients} />
            <div className="stat-note">
              启用 {data.enabledClients} · 在线 {data.onlineClients}
            </div>
          </Card>
        </Col>
        <Col xs={12} md={6}>
          <Card>
            <Statistic title="累计上传" value={formatBytes(data.up)} />
          </Card>
        </Col>
        <Col xs={12} md={6}>
          <Card>
            <Statistic title="累计下载" value={formatBytes(data.down)} />
          </Card>
        </Col>
      </Row>

      <Card title="累计流量趋势" style={{ marginTop: 16 }}>
        <TrafficSparkline points={data.history} />
        <div className="legend">
          <span>
            <span className="legend-dot" style={{ background: "#1677ff" }} />
            上传 {formatBytes(data.up)}
          </span>
          <span>
            <span className="legend-dot" style={{ background: "#52c41a" }} />
            下载 {formatBytes(data.down)}
          </span>
          <span>面板版本 {data.version}</span>
        </div>
      </Card>

      <Modal
        open={logsOpen}
        title="Xray 日志（最近 300 行）"
        width={820}
        onCancel={() => setLogsOpen(false)}
        footer={<Button onClick={() => setLogsOpen(false)}>关闭</Button>}
      >
        <pre className="code-block">
          {logs.length > 0 ? logs.join("\n") : "(暂无日志)"}
        </pre>
      </Modal>
    </div>
  );
}
