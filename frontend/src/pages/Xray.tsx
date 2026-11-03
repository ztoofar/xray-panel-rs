import { useEffect, useState } from "react";
import {
  Alert,
  App as AntApp,
  Button,
  Card,
  Col,
  Descriptions,
  Form,
  Input,
  Row,
  Select,
  Space,
  Switch,
  Tabs,
  Tag,
  Typography,
} from "antd";
import {
  CheckCircleOutlined,
  DeleteOutlined,
  PlusOutlined,
  ReloadOutlined,
  ThunderboltOutlined,
} from "@ant-design/icons";
import { get, post, put } from "../api";
import type { XrayStatus, XrayTemplateResponse } from "../types";
import { copyText, formatTime } from "../util";

interface StatusResponse {
  status: XrayStatus;
  configPath: string;
  apiAddress: string;
}

interface DnsServerRow {
  address?: string;
  domains?: string[];
  expectIPs?: string[];
}

interface DnsHostRow {
  domain?: string;
  values?: string[];
}

interface DnsFormValues {
  servers?: DnsServerRow[];
  hosts?: DnsHostRow[];
  queryStrategy?: string;
  disableFallback?: boolean;
}

interface LogFormValues {
  loglevel?: string;
  access?: string;
  error?: string;
  dnsLog?: boolean;
}

function dnsToForm(dns: Record<string, any>): DnsFormValues {
  return {
    servers: (dns.servers ?? []).map((server: any) =>
      typeof server === "string"
        ? { address: server }
        : {
            address: String(server.address ?? ""),
            domains: server.domains ?? [],
            expectIPs: server.expectIPs ?? [],
          },
    ),
    hosts: Object.entries(dns.hosts ?? {}).map(([domain, value]) => ({
      domain,
      values: Array.isArray(value) ? (value as string[]) : [String(value)],
    })),
    queryStrategy: dns.queryStrategy ?? "UseIP",
    disableFallback: Boolean(dns.disableFallback),
  };
}

function formToDns(values: DnsFormValues): Record<string, unknown> {
  const servers = (values.servers ?? [])
    .filter((server) => server.address?.trim())
    .map((server) => ({
      address: server.address?.trim(),
      ...(server.domains?.length ? { domains: server.domains } : {}),
      ...(server.expectIPs?.length ? { expectIPs: server.expectIPs } : {}),
    }));
  const hosts = Object.fromEntries(
    (values.hosts ?? [])
      .filter((host) => host.domain?.trim())
      .map((host) => [host.domain?.trim(), host.values ?? []]),
  );
  return {
    servers,
    ...(values.queryStrategy ? { queryStrategy: values.queryStrategy } : {}),
    ...(values.disableFallback ? { disableFallback: true } : {}),
    ...(Object.keys(hosts).length ? { hosts } : {}),
  };
}

export default function XrayPage() {
  const { message, modal } = AntApp.useApp();
  const [dnsForm] = Form.useForm<DnsFormValues>();
  const [logForm] = Form.useForm<LogFormValues>();
  const [status, setStatus] = useState<StatusResponse>();
  const [configText, setConfigText] = useState("");
  const [logs, setLogs] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  const load = async () => {
    setLoading(true);
    try {
      const [statusResponse, config, logResponse, template] = await Promise.all([
        get<StatusResponse>("/api/xray/status"),
        get<Record<string, unknown>>("/api/xray/config"),
        get<{ lines: string[] }>("/api/xray/logs?limit=300"),
        get<XrayTemplateResponse>("/api/xray/template"),
      ]);
      setStatus(statusResponse);
      setConfigText(JSON.stringify(config, null, 2));
      setLogs(logResponse.lines ?? []);
      dnsForm.setFieldsValue(dnsToForm((template.template.dns as Record<string, any>) ?? {}));
      const log = (template.template.log as Record<string, any>) ?? {};
      logForm.setFieldsValue({
        loglevel: log.loglevel ?? "warning",
        access: log.access ?? "",
        error: log.error ?? "",
        dnsLog: Boolean(log.dnsLog),
      });
    } catch (error) {
      message.error((error as Error).message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
    const timer = setInterval(() => {
      get<StatusResponse>("/api/xray/status")
        .then(setStatus)
        .catch(() => {});
    }, 5000);
    return () => clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const testConfig = async () => {
    try {
      await post("/api/xray/test");
      message.success("配置测试通过");
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const restart = () => {
    modal.confirm({
      title: "重启 Xray？",
      content: "会写入当前数据库生成的配置并重启 Xray，所有连接会短暂中断。",
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

  const saveDns = async () => {
    try {
      const values = await dnsForm.validateFields();
      await put("/api/xray/dns", { dns: formToDns(values) });
      message.success("DNS 已保存，Xray 已重启");
      await load();
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const saveLog = async () => {
    try {
      const values = await logForm.validateFields();
      await put("/api/xray/log", {
        log: {
          loglevel: values.loglevel || "warning",
          ...(values.access ? { access: values.access } : {}),
          ...(values.error ? { error: values.error } : {}),
          ...(values.dnsLog ? { dnsLog: true } : {}),
        },
      });
      message.success("日志设置已保存，Xray 已重启");
      await load();
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const current = status?.status;

  return (
    <div>
      <div className="page-title" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <Typography.Title level={4} style={{ margin: 0 }}>
          Xray
        </Typography.Title>
        <Space wrap>
          <Button icon={<ReloadOutlined />} onClick={load} loading={loading}>
            刷新
          </Button>
          <Button icon={<CheckCircleOutlined />} onClick={testConfig}>
            测试配置
          </Button>
          <Button icon={<ThunderboltOutlined />} onClick={restart}>
            重启 Xray
          </Button>
        </Space>
      </div>

      {current?.lastError && (
        <Alert
          type="warning"
          showIcon
          message="最近错误"
          description={<span className="word-break">{current.lastError}</span>}
          style={{ marginBottom: 16 }}
        />
      )}

      <Card style={{ marginBottom: 16 }}>
        <Descriptions
          bordered
          size="small"
          column={2}
          items={[
            {
              key: "state",
              label: "状态",
              children: current?.running ? <Tag color="success">运行中</Tag> : <Tag color="error">已停止</Tag>,
            },
            { key: "version", label: "版本", children: current?.version ?? "未知" },
            { key: "pid", label: "PID", children: current?.pid ?? "—" },
            { key: "restarts", label: "自动重启次数", children: current?.restarts ?? 0 },
            { key: "started", label: "启动时间", children: formatTime(current?.startedAt) },
            {
              key: "exit",
              label: "上次退出",
              children: current?.lastExitAt
                ? `${formatTime(current.lastExitAt)} (code ${current.lastExitCode ?? "?"})`
                : "—",
            },
            { key: "config", label: "配置文件", children: status?.configPath ?? "—" },
            { key: "api", label: "统计 API", children: status?.apiAddress ?? "—" },
          ]}
        />
      </Card>

      <Card>
        <Tabs
          items={[
            {
              key: "config",
              label: "生成的配置",
              children: (
                <div>
                  <Space style={{ marginBottom: 8 }}>
                    <Button
                      size="small"
                      onClick={() => copyText(configText).then(() => message.success("已复制"))}
                    >
                      复制 JSON
                    </Button>
                  </Space>
                  <pre className="code-block">{configText || "(加载中)"}</pre>
                </div>
              ),
            },
            {
              key: "logs",
              label: "运行日志",
              children: (
                <div>
                  <Space style={{ marginBottom: 8 }}>
                    <Button size="small" onClick={load}>
                      刷新日志
                    </Button>
                  </Space>
                  <pre className="code-block">{logs.length > 0 ? logs.join("\n") : "(暂无日志)"}</pre>
                </div>
              ),
            },
            {
              key: "dns",
              label: "DNS 设置",
              children: (
                <Form form={dnsForm} layout="vertical" initialValues={{ servers: [], hosts: [] }}>
                  <Row gutter={16}>
                    <Col span={8}>
                      <Form.Item name="queryStrategy" label="查询策略">
                        <Select
                          options={["UseIP", "UseIPv4", "UseIPv6", "AsIs"].map((value) => ({
                            value,
                            label: value,
                          }))}
                        />
                      </Form.Item>
                    </Col>
                    <Col span={8}>
                      <Form.Item name="disableFallback" label="禁用 fallback" valuePropName="checked">
                        <Switch />
                      </Form.Item>
                    </Col>
                  </Row>

                  <Typography.Title level={5}>DNS 服务器</Typography.Title>
                  <Form.List name="servers">
                    {(fields, { add, remove }) => (
                      <div>
                        {fields.map((field) => (
                          <Card key={field.key} size="small" style={{ marginBottom: 8 }}>
                            <Row gutter={12} align="middle">
                              <Col span={8}>
                                <Form.Item
                                  name={[field.name, "address"]}
                                  label="地址"
                                  rules={[{ required: true }]}
                                  style={{ marginBottom: 0 }}
                                >
                                  <Input placeholder="1.1.1.1 / https://dns.example/dns-query" />
                                </Form.Item>
                              </Col>
                              <Col span={7}>
                                <Form.Item
                                  name={[field.name, "domains"]}
                                  label="匹配域名（可留空）"
                                  style={{ marginBottom: 0 }}
                                >
                                  <Select mode="tags" placeholder="geosite:cn" />
                                </Form.Item>
                              </Col>
                              <Col span={7}>
                                <Form.Item
                                  name={[field.name, "expectIPs"]}
                                  label="期望 IP（可留空）"
                                  style={{ marginBottom: 0 }}
                                >
                                  <Select mode="tags" placeholder="geoip:cn" />
                                </Form.Item>
                              </Col>
                              <Col span={2} style={{ textAlign: "right" }}>
                                <Button danger type="text" icon={<DeleteOutlined />} onClick={() => remove(field.name)} />
                              </Col>
                            </Row>
                          </Card>
                        ))}
                        <Button
                          type="dashed"
                          block
                          icon={<PlusOutlined />}
                          onClick={() => add({ address: "", domains: [], expectIPs: [] })}
                        >
                          添加 DNS 服务器
                        </Button>
                      </div>
                    )}
                  </Form.List>

                  <Typography.Title level={5} style={{ marginTop: 20 }}>
                    Hosts
                  </Typography.Title>
                  <Form.List name="hosts">
                    {(fields, { add, remove }) => (
                      <div>
                        {fields.map((field) => (
                          <Row key={field.key} gutter={12} align="middle" style={{ marginBottom: 8 }}>
                            <Col span={9}>
                              <Form.Item
                                name={[field.name, "domain"]}
                                label="域名"
                                rules={[{ required: true }]}
                                style={{ marginBottom: 0 }}
                              >
                                <Input placeholder="example.com" />
                              </Form.Item>
                            </Col>
                            <Col span={13}>
                              <Form.Item
                                name={[field.name, "values"]}
                                label="解析结果"
                                style={{ marginBottom: 0 }}
                              >
                                <Select mode="tags" placeholder="1.2.3.4" />
                              </Form.Item>
                            </Col>
                            <Col span={2} style={{ textAlign: "right" }}>
                              <Button danger type="text" icon={<DeleteOutlined />} onClick={() => remove(field.name)} />
                            </Col>
                          </Row>
                        ))}
                        <Button
                          type="dashed"
                          block
                          icon={<PlusOutlined />}
                          onClick={() => add({ domain: "", values: [] })}
                        >
                          添加 Hosts
                        </Button>
                      </div>
                    )}
                  </Form.List>

                  <Button type="primary" style={{ marginTop: 16 }} onClick={saveDns}>
                    保存 DNS
                  </Button>
                </Form>
              ),
            },
            {
              key: "log",
              label: "日志设置",
              children: (
                <Form form={logForm} layout="vertical" style={{ maxWidth: 640 }}>
                  <Form.Item name="loglevel" label="日志级别">
                    <Select
                      options={["debug", "info", "warning", "error", "none"].map((value) => ({
                        value,
                        label: value,
                      }))}
                    />
                  </Form.Item>
                  <Form.Item name="access" label="访问日志路径（留空关闭）">
                    <Input placeholder="/var/log/xray/access.log" />
                  </Form.Item>
                  <Form.Item name="error" label="错误日志路径（留空使用面板日志）">
                    <Input placeholder="/var/log/xray/error.log" />
                  </Form.Item>
                  <Form.Item name="dnsLog" label="记录 DNS 日志" valuePropName="checked">
                    <Switch />
                  </Form.Item>
                  <Button type="primary" onClick={saveLog}>
                    保存日志设置
                  </Button>
                </Form>
              ),
            },
          ]}
        />
      </Card>
    </div>
  );
}
