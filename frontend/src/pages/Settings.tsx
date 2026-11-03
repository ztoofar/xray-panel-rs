import { useEffect, useState } from "react";
import {
  App as AntApp,
  Button,
  Card,
  Col,
  Form,
  Input,
  InputNumber,
  Row,
  Space,
  Typography,
  Upload,
} from "antd";
import { DownloadOutlined, UploadOutlined } from "@ant-design/icons";
import { get, post, put } from "../api";
import { useAuth } from "../auth";
import type { SettingsData } from "../types";

interface SettingsFormValues {
  panelName: string;
  publicAddress: string;
  trafficPollSecs: number;
}

interface CredentialsFormValues {
  username: string;
  currentPassword: string;
  newPassword?: string;
  confirmPassword?: string;
}

interface CredentialsResponse {
  ok: boolean;
  username: string;
  csrf: string;
}

export default function SettingsPage() {
  const { message, modal } = AntApp.useApp();
  const { login, username, csrf } = useAuth();
  const [form] = Form.useForm<SettingsFormValues>();
  const [credentialsForm] = Form.useForm<CredentialsFormValues>();
  const [loading, setLoading] = useState(false);
  const [panelListen, setPanelListen] = useState("");
  const [restoreFile, setRestoreFile] = useState<File | null>(null);

  const load = async () => {
    setLoading(true);
    try {
      const data = await get<SettingsData>("/api/settings");
      form.setFieldsValue({
        panelName: data.panelName,
        publicAddress: data.publicAddress,
        trafficPollSecs: data.trafficPollSecs,
      });
      credentialsForm.setFieldsValue({ username: username ?? "" });
      setPanelListen(data.panelListen);
    } catch (error) {
      message.error((error as Error).message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const saveSettings = async (values: SettingsFormValues) => {
    try {
      await put("/api/settings", {
        panelName: values.panelName,
        publicAddress: values.publicAddress ?? "",
        trafficPollSecs: values.trafficPollSecs,
      });
      message.success("设置已保存");
      await load();
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const downloadBackup = async () => {
    try {
      const response = await fetch("/api/server/db", { credentials: "same-origin" });
      if (!response.ok) throw new Error(`下载失败 (${response.status})`);
      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `xray-panel-${new Date()
        .toISOString()
        .slice(0, 19)
        .replace(/[:T]/g, "-")}.db`;
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      message.success("备份已下载");
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const restoreBackup = () => {
    if (!restoreFile) return;
    modal.confirm({
      title: "恢复数据库备份？",
      content: "当前用户、设置、入站、客户端都会被备份文件覆盖，完成后需要重新登录。",
      okText: "恢复",
      okType: "danger",
      cancelText: "取消",
      onOk: async () => {
        try {
          const data = new FormData();
          data.append("db", restoreFile);
          const response = await fetch("/api/server/db/restore", {
            method: "POST",
            body: data,
            credentials: "same-origin",
            headers: csrf ? { "X-CSRF-Token": csrf } : {},
          });
          const payload = await response.json().catch(() => null);
          if (!response.ok) {
            throw new Error(payload?.error ?? `恢复失败 (${response.status})`);
          }
          message.success(
            payload?.warning
              ? `恢复完成，但 Xray 应用失败：${payload.warning}`
              : "数据库已恢复",
          );
          setTimeout(() => window.location.reload(), 1200);
        } catch (error) {
          message.error((error as Error).message);
        }
      },
    });
  };

  const saveCredentials = async (values: CredentialsFormValues) => {
    try {
      const response = await post<CredentialsResponse>("/api/credentials", {
        username: values.username,
        currentPassword: values.currentPassword,
        newPassword: values.newPassword || undefined,
      });
      login(response.username, response.csrf);
      message.success("账号信息已更新，其他会话已失效");
      credentialsForm.setFieldsValue({
        username: response.username,
        currentPassword: "",
        newPassword: "",
        confirmPassword: "",
      });
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  return (
    <div>
      <div className="page-title">
        <Typography.Title level={4} style={{ margin: 0 }}>
          面板设置
        </Typography.Title>
      </div>

      <Row gutter={[16, 16]}>
        <Col xs={24} xl={14}>
          <Card title="常规设置" loading={loading}>
            <Form form={form} layout="vertical" onFinish={saveSettings} requiredMark={false}>
              <Row gutter={16}>
                <Col span={12}>
                  <Form.Item name="panelName" label="面板名称" rules={[{ required: true }]}>
                    <Input maxLength={64} />
                  </Form.Item>
                </Col>
                <Col span={12}>
                  <Form.Item
                    name="publicAddress"
                    label="公网地址"
                    extra="用于生成分享链接，只填主机名，例如 vpn.example.com"
                  >
                    <Input placeholder="留空则使用浏览器访问的 Host" />
                  </Form.Item>
                </Col>
                <Col span={12}>
                  <Form.Item
                    name="trafficPollSecs"
                    label="流量统计间隔（秒）"
                    extra="范围 5-300，小内存机器建议 10-30 秒"
                  >
                    <InputNumber min={5} max={300} style={{ width: "100%" }} />
                  </Form.Item>
                </Col>
              </Row>
              <Typography.Paragraph type="secondary">
                出站、路由、DNS、日志已改为独立页面，全部通过表单配置，不需要手写 JSON。
                需要直接编辑 Xray 配置文件时，打开 <a href="/raw-config">原始配置编辑器</a>。
              </Typography.Paragraph>
              <Button type="primary" htmlType="submit">
                保存设置
              </Button>
            </Form>
          </Card>

          <Card title="数据库备份与恢复" style={{ marginTop: 16 }}>
            <Space direction="vertical" size="middle" style={{ width: "100%" }}>
              <Button icon={<DownloadOutlined />} onClick={downloadBackup}>
                下载 .db 备份
              </Button>
              <Upload
                accept=".db"
                maxCount={1}
                beforeUpload={(file) => {
                  setRestoreFile(file);
                  return false;
                }}
                onRemove={() => setRestoreFile(null)}
              >
                <Button icon={<UploadOutlined />}>选择 .db 备份文件</Button>
              </Upload>
              {restoreFile && (
                <Typography.Text type="secondary">已选择：{restoreFile.name}</Typography.Text>
              )}
              <Button danger disabled={!restoreFile} onClick={restoreBackup}>
                恢复备份
              </Button>
              <Typography.Text type="secondary">
                恢复会覆盖当前用户、设置、入站和客户端，完成后需要重新登录。
              </Typography.Text>
            </Space>
          </Card>
        </Col>

        <Col xs={24} xl={10}>
          <Card title="管理员账号">
            <Form
              form={credentialsForm}
              layout="vertical"
              onFinish={saveCredentials}
              requiredMark={false}
            >
              <Form.Item
                name="username"
                label="用户名"
                rules={[{ required: true, message: "请输入用户名" }]}
              >
                <Input maxLength={64} />
              </Form.Item>
              <Form.Item
                name="currentPassword"
                label="当前密码"
                rules={[{ required: true, message: "请输入当前密码" }]}
              >
                <Input.Password />
              </Form.Item>
              <Form.Item name="newPassword" label="新密码（留空不修改）">
                <Input.Password minLength={8} />
              </Form.Item>
              <Form.Item
                name="confirmPassword"
                label="确认新密码"
                dependencies={["newPassword"]}
                rules={[
                  ({ getFieldValue }) => ({
                    validator(_, value) {
                      const next = getFieldValue("newPassword");
                      if (!next || value === next) {
                        return Promise.resolve();
                      }
                      return Promise.reject(new Error("两次输入的密码不一致"));
                    },
                  }),
                ]}
              >
                <Input.Password />
              </Form.Item>
              <Button type="primary" htmlType="submit">
                更新账号
              </Button>
              <Typography.Paragraph type="secondary" style={{ marginTop: 12 }}>
                修改后会注销其他已登录会话。当前监听：{panelListen || "—"}，默认
                0.0.0.0:2053，可通过环境变量 PANEL_LISTEN 改（例如 127.0.0.1:2053
                仅本机）；面板不提供 SSL 证书功能。数据库路径、Xray
                路径同样通过环境变量配置。
              </Typography.Paragraph>
            </Form>
          </Card>
        </Col>
      </Row>
    </div>
  );
}
