import { useEffect, useMemo, useState } from "react";
import {
  Alert,
  App as AntApp,
  Button,
  Card,
  Col,
  DatePicker,
  Empty,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
  Popover,
  Row,
  Select,
  Space,
  Switch,
  Table,
  Tag,
  Typography,
} from "antd";
import type { TableProps } from "antd";
import {
  DeleteOutlined,
  EditOutlined,
  LinkOutlined,
  PlusOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import dayjs, { type Dayjs } from "dayjs";
import { del, get, post, put } from "../api";
import type { Client, Inbound, LinksResponse, OnlineClient } from "../types";
import { copyText, formatBytes, formatTime } from "../util";

function randomAlnum(length: number): string {
  const alphabet = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
  let out = "";
  for (let i = 0; i < length; i += 1) out += alphabet[Math.floor(Math.random() * alphabet.length)];
  return out;
}

function randomBase64(bytes: number): string {
  const buffer = new Uint8Array(bytes);
  if (window.crypto?.getRandomValues) window.crypto.getRandomValues(buffer);
  else for (let i = 0; i < buffer.length; i += 1) buffer[i] = Math.floor(Math.random() * 256);
  return btoa(String.fromCharCode(...buffer));
}

function uuid(): string {
  const bytes = new Uint8Array(16);
  if (window.crypto?.getRandomValues) window.crypto.getRandomValues(bytes);
  else for (let i = 0; i < bytes.length; i += 1) bytes[i] = Math.floor(Math.random() * 256);
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function generated(inbounds: Inbound[]): Partial<ClientFormValues> {
  const protocols = new Set(inbounds.map((inbound) => inbound.protocol));
  const result: Partial<ClientFormValues> = {};
  if (protocols.has("vless") || protocols.has("vmess")) result.uuid = uuid();
  if (protocols.has("vless")) result.flow = "";
  if (protocols.has("shadowsocks")) {
    const inbound = inbounds.find((item) => item.protocol === "shadowsocks");
    const method = String((inbound?.settings as Record<string, unknown>)?.method ?? "");
    result.password = randomBase64(method.includes("256") ? 32 : 16);
  } else if (protocols.has("trojan")) {
    result.password = randomAlnum(20);
  }
  if (protocols.has("http") || protocols.has("mixed")) {
    result.user = `user${randomAlnum(4)}`;
    result.pass = randomAlnum(16);
  }
  return result;
}

function passwordFor(inbounds: Inbound[]): string {
  const ss = inbounds.find((inbound) => inbound.protocol === "shadowsocks");
  if (ss) {
    const method = String((ss.settings as Record<string, unknown>)?.method ?? "");
    return randomBase64(method.includes("256") ? 32 : 16);
  }
  return randomAlnum(20);
}

interface ClientFormValues {
  inboundIds: number[];
  email: string;
  enable: boolean;
  totalGb?: number;
  expiry?: Dayjs | null;
  uuid?: string;
  password?: string;
  flow?: string;
  user?: string;
  pass?: string;
  comment?: string;
  groupName?: string;
  limitIp?: number;
  limitHwid?: number;
  resetDays?: number;
}

export default function ClientsPage() {
  const { message, modal } = AntApp.useApp();
  const [form] = Form.useForm<ClientFormValues>();
  const [inbounds, setInbounds] = useState<Inbound[]>([]);
  const [clients, setClients] = useState<Client[]>([]);
  const [online, setOnline] = useState<OnlineClient[]>([]);
  const [filterInbound, setFilterInbound] = useState<number>();
  const [loading, setLoading] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Client | null>(null);
  const [selectedRowKeys, setSelectedRowKeys] = useState<number[]>([]);

  const inboundMap = useMemo(
    () => new Map(inbounds.map((inbound) => [inbound.id, inbound])),
    [inbounds],
  );
  const onlineMap = useMemo(
    () => new Map(online.map((item) => [item.email, item])),
    [online],
  );
  const sortedClients = useMemo(
    () =>
      [...clients].sort((left, right) => {
        const leftOnline = onlineMap.has(left.email) ? 1 : 0;
        const rightOnline = onlineMap.has(right.email) ? 1 : 0;
        if (leftOnline !== rightOnline) return rightOnline - leftOnline;
        return left.id - right.id;
      }),
    [clients, onlineMap],
  );
  const selectedInboundIds = Form.useWatch("inboundIds", form) ?? [];
  const selectedInbounds = useMemo(
    () =>
      selectedInboundIds
        .map((id: number) => inboundMap.get(id))
        .filter((inbound): inbound is Inbound => Boolean(inbound)),
    [selectedInboundIds, inboundMap],
  );
  const protocols = useMemo(
    () => new Set(selectedInbounds.map((inbound) => inbound.protocol)),
    [selectedInbounds],
  );
  const hasUuid = protocols.has("vless") || protocols.has("vmess");
  const hasVless = protocols.has("vless");
  const hasPassword = protocols.has("trojan") || protocols.has("shadowsocks");
  const hasAccount = protocols.has("http") || protocols.has("mixed");

  const loadInbounds = async () => {
    try {
      setInbounds(await get<Inbound[]>("/api/inbounds"));
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const loadClients = async (inboundId?: number) => {
    setLoading(true);
    try {
      const query = inboundId ? `?inboundId=${inboundId}` : "";
      setClients(await get<Client[]>(`/api/clients${query}`));
    } catch (error) {
      message.error((error as Error).message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void loadInbounds();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    void loadClients(filterInbound);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filterInbound]);

  useEffect(() => {
    const loadOnline = async () => {
      try {
        setOnline(await get<OnlineClient[]>("/api/clients/online"));
      } catch {
        // Online status is best-effort; keep the last known set.
      }
    };
    void loadOnline();
    const timer = setInterval(loadOnline, 5000);
    return () => clearInterval(timer);
  }, []);

  const openCreate = () => {
    if (inbounds.length === 0) {
      message.warning("请先创建入站");
      return;
    }
    const preferred = filterInbound
      ? inbounds.filter((item) => item.id === filterInbound)
      : [];
    const selected = preferred.length > 0 ? preferred : [inbounds[0]];
    setEditing(null);
    form.setFieldsValue({
      inboundIds: selected.map((item) => item.id),
      email: "",
      enable: true,
      totalGb: 0,
      expiry: null,
      comment: "",
      groupName: "",
      limitIp: 0,
      limitHwid: 0,
      resetDays: 0,
      ...generated(selected),
    });
    setModalOpen(true);
  };

  const openEdit = (client: Client) => {
    const ids =
      client.inboundIds && client.inboundIds.length > 0
        ? client.inboundIds
        : [client.inboundId];
    const creds = (client.settings ?? {}) as Record<string, any>;
    setEditing(client);
    form.setFieldsValue({
      inboundIds: ids,
      email: client.email,
      enable: client.enable,
      totalGb: client.total > 0 ? Number((client.total / 1024 ** 3).toFixed(2)) : 0,
      expiry: client.expiryTime ? dayjs.unix(client.expiryTime) : null,
      uuid: creds.id ?? "",
      password: creds.password ?? "",
      flow: creds.flow ?? "",
      user: creds.user ?? "",
      pass: creds.pass ?? "",
      comment: client.comment ?? "",
      groupName: client.groupName ?? "",
      limitIp: client.limitIp ?? 0,
      limitHwid: client.limitHwid ?? 0,
      resetDays: client.resetDays ?? 0,
    });
    setModalOpen(true);
  };

  const submit = async (values: ClientFormValues) => {
    try {
      const selected = values.inboundIds
        .map((id) => inboundMap.get(id))
        .filter((inbound): inbound is Inbound => Boolean(inbound));
      if (selected.length === 0) throw new Error("至少选择一个入站");
      const selectedProtocols = new Set(selected.map((inbound) => inbound.protocol));
      const settings: Record<string, unknown> = {};
      if (selectedProtocols.has("vless") || selectedProtocols.has("vmess")) {
        settings.id = values.uuid || uuid();
      }
      if (selectedProtocols.has("vless") && values.flow) {
        settings.flow = values.flow;
      }
      if (
        selectedProtocols.has("trojan") ||
        selectedProtocols.has("shadowsocks")
      ) {
        settings.password = values.password || passwordFor(selected);
      }
      if (selectedProtocols.has("http") || selectedProtocols.has("mixed")) {
        settings.user = values.user || values.email;
        settings.pass = values.pass || randomAlnum(16);
      }

      const payload = {
        inboundIds: values.inboundIds,
        email: values.email,
        enable: values.enable,
        total: Math.round((values.totalGb ?? 0) * 1024 ** 3),
        expiryTime: values.expiry ? values.expiry.unix() : 0,
        comment: values.comment ?? "",
        groupName: values.groupName ?? "",
        limitIp: values.limitIp ?? 0,
        limitHwid: values.limitHwid ?? 0,
        resetDays: values.resetDays ?? 0,
        settings,
      };
      if (editing) {
        await put(`/api/clients/${editing.id}`, payload);
      } else {
        await post("/api/clients", payload);
      }
      message.success("客户端已保存，Xray 已应用配置");
      setModalOpen(false);
      await loadClients(filterInbound);
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const toggle = async (client: Client, enable: boolean) => {
    try {
      await post(`/api/clients/${client.id}/toggle`, { enable });
      await loadClients(filterInbound);
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const remove = async (client: Client) => {
    try {
      await del(`/api/clients/${client.id}`);
      message.success("客户端已删除");
      await loadClients(filterInbound);
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const reset = (client: Client) => {
    modal.confirm({
      title: `重置 ${client.email} 的流量？`,
      content: "会同时重置 Xray 中该客户端的统计计数。",
      okText: "重置",
      cancelText: "取消",
      onOk: async () => {
        try {
          await post(`/api/clients/${client.id}/reset`);
          message.success("流量已重置");
          await loadClients(filterInbound);
        } catch (error) {
          message.error((error as Error).message);
        }
      },
    });
  };

  const bulkToggle = async (enable: boolean) => {
    if (selectedRowKeys.length === 0) return;
    const results = await Promise.allSettled(
      selectedRowKeys.map((id) => post(`/api/clients/${id}/toggle`, { enable })),
    );
    const failed = results.filter((result) => result.status === "rejected").length;
    if (failed > 0) message.warning(`${failed} 个客户端操作失败`);
    else message.success(`已${enable ? "启用" : "停用"} ${selectedRowKeys.length} 个客户端`);
    setSelectedRowKeys([]);
    await loadClients(filterInbound);
  };

  const bulkReset = () => {
    if (selectedRowKeys.length === 0) return;
    modal.confirm({
      title: `重置 ${selectedRowKeys.length} 个客户端的流量？`,
      okText: "重置",
      cancelText: "取消",
      onOk: async () => {
        const results = await Promise.allSettled(
          selectedRowKeys.map((id) => post(`/api/clients/${id}/reset`)),
        );
        const failed = results.filter((result) => result.status === "rejected").length;
        if (failed > 0) message.warning(`${failed} 个客户端重置失败`);
        else message.success("批量重置完成");
        setSelectedRowKeys([]);
        await loadClients(filterInbound);
      },
    });
  };

  const bulkDelete = () => {
    if (selectedRowKeys.length === 0) return;
    modal.confirm({
      title: `删除 ${selectedRowKeys.length} 个客户端？`,
      content: "该操作不可撤销。",
      okText: "删除",
      okButtonProps: { danger: true },
      cancelText: "取消",
      onOk: async () => {
        const results = await Promise.allSettled(
          selectedRowKeys.map((id) => del(`/api/clients/${id}`)),
        );
        const failed = results.filter((result) => result.status === "rejected").length;
        if (failed > 0) message.warning(`${failed} 个客户端删除失败`);
        else message.success("批量删除完成");
        setSelectedRowKeys([]);
        await loadClients(filterInbound);
      },
    });
  };

  const copyLink = async (client: Client) => {
    try {
      const inboundId = client.inboundIds?.[0] ?? client.inboundId;
      const response = await get<LinksResponse>(`/api/inbounds/${inboundId}/links`);
      const item = response.items.find((entry) => entry.email === client.email);
      if (!item?.link) throw new Error(item?.error ?? "该协议没有可用的分享链接");
      await copyText(item.link);
      message.success("链接已复制");
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const columns: TableProps<Client>["columns"] = [
    {
      title: "邮箱 / 标识",
      dataIndex: "email",
      render: (value: string) => {
        const entry = onlineMap.get(value);
        return (
          <Space size={8}>
            {entry && (
              <Popover
                title="在线详情"
                content={
                  <div style={{ whiteSpace: "pre-line", maxWidth: 320 }}>
                    {entry.ips.length > 0
                      ? entry.ips
                          .map(
                            (ip) =>
                              `${ip.ip} · ${new Date(ip.lastSeen * 1000).toLocaleTimeString()}`,
                          )
                          .join("\n")
                      : "在线"}
                  </div>
                }
              >
                <Tag color="green" style={{ cursor: "pointer" }}>
                  在线
                </Tag>
              </Popover>
            )}
            <Typography.Text strong>{value}</Typography.Text>
          </Space>
        );
      },
    },
    {
      title: "所属入站",
      width: 220,
      render: (_, record) => {
        const ids =
          record.inboundIds && record.inboundIds.length > 0
            ? record.inboundIds
            : [record.inboundId];
        const labels = ids
          .map((id) => inboundMap.get(id))
          .filter((inbound): inbound is Inbound => Boolean(inbound))
          .map((inbound) => `${inbound.remark || inbound.tag} (${inbound.protocol}:${inbound.port})`);
        return (
          <span style={{ whiteSpace: "pre-line" }}>
            {labels.length > 0 ? labels.join("\n") : ids.join(", ")}
          </span>
        );
      },
    },
    {
      title: "分组 / 备注",
      width: 180,
      render: (_, record) => (
        <div>
          {record.groupName ? <Tag>{record.groupName}</Tag> : null}
          <div className="stat-note">{record.comment || "—"}</div>
        </div>
      ),
    },
    {
      title: "凭据",
      width: 240,
      render: (_, record) => {
        const creds = (record.settings ?? {}) as Record<string, any>;
        const value = creds.id ?? creds.password ?? creds.user ?? "";
        return <Typography.Text code>{String(value).slice(0, 32)}</Typography.Text>;
      },
    },
    {
      title: "流量",
      width: 210,
      render: (_, record) => (
        <div>
          <div>
            ↑ {formatBytes(record.up)} / ↓ {formatBytes(record.down)}
          </div>
          <div className="stat-note">
            限额 {record.total > 0 ? formatBytes(record.total) : "不限"}
          </div>
        </div>
      ),
    },
    {
      title: "限制",
      width: 150,
      render: (_, record) => (
        <div className="stat-note" style={{ whiteSpace: "pre-line" }}>
          {`IP: ${record.limitIp > 0 ? record.limitIp : "不限"}`}
          {`\nHWID: ${record.limitHwid > 0 ? record.limitHwid : "不限"}`}
          {`\n重置: ${record.resetDays > 0 ? `${record.resetDays} 天` : "不重置"}`}
        </div>
      ),
    },
    { title: "到期", width: 160, render: (_, record) => formatTime(record.expiryTime) },
    {
      title: "启用",
      width: 80,
      render: (_, record) => (
        <Switch size="small" checked={record.enable} onChange={(checked) => toggle(record, checked)} />
      ),
    },
    {
      title: "操作",
      width: 320,
      render: (_, record) => (
        <Space size="small" wrap>
          <Button size="small" type="link" icon={<EditOutlined />} onClick={() => openEdit(record)}>
            编辑
          </Button>
          <Button size="small" type="link" icon={<ReloadOutlined />} onClick={() => reset(record)}>
            重置流量
          </Button>
          <Button size="small" type="link" icon={<LinkOutlined />} onClick={() => copyLink(record)}>
            复制链接
          </Button>
          <Popconfirm title="删除该客户端？" onConfirm={() => remove(record)}>
            <Button size="small" type="link" danger icon={<DeleteOutlined />}>
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div>
      <div className="page-title" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <Typography.Title level={4} style={{ margin: 0 }}>
          客户端
        </Typography.Title>
        <Space wrap>
          {selectedRowKeys.length > 0 && (
            <>
              <Typography.Text type="secondary">已选 {selectedRowKeys.length}</Typography.Text>
              <Button size="small" onClick={() => bulkToggle(true)}>
                批量启用
              </Button>
              <Button size="small" onClick={() => bulkToggle(false)}>
                批量停用
              </Button>
              <Button size="small" onClick={bulkReset}>
                批量重置
              </Button>
              <Button size="small" danger onClick={bulkDelete}>
                批量删除
              </Button>
            </>
          )}
          <Select
            allowClear
            placeholder="全部入站"
            style={{ width: 220 }}
            value={filterInbound}
            onChange={(value) => setFilterInbound(value)}
            options={inbounds.map((inbound) => ({
              value: inbound.id,
              label: `${inbound.remark || inbound.tag} (${inbound.protocol}:${inbound.port})`,
            }))}
          />
          <Button type="primary" icon={<PlusOutlined />} onClick={openCreate}>
            新建客户端
          </Button>
        </Space>
      </div>

      <Alert
        type="info"
        showIcon
        style={{ marginBottom: 16 }}
        message={`凭据由面板生成，可点击“生成”重新随机；当前在线 ${online.length} 个客户端，在线客户端会标绿并置顶。`}
      />

      <Card>
        <Table
          rowKey="id"
          loading={loading}
          columns={columns}
          dataSource={sortedClients}
          rowClassName={(record) => (onlineMap.has(record.email) ? "row-online" : "")}
          rowSelection={{
            selectedRowKeys,
            onChange: (keys) => setSelectedRowKeys(keys as number[]),
          }}
          pagination={false}
          scroll={{ x: 1150 }}
          locale={{ emptyText: <Empty description="暂无客户端" /> }}
        />
      </Card>

      <Modal
        open={modalOpen}
        title={editing ? "编辑客户端" : "新建客户端"}
        width={760}
        onCancel={() => setModalOpen(false)}
        onOk={() => form.submit()}
        okText="保存并应用"
        cancelText="取消"
        destroyOnHidden
      >
        <Form form={form} layout="vertical" onFinish={submit} requiredMark={false} preserve={false}>
          <Row gutter={16}>
            <Col span={12}>
              <Form.Item
                name="inboundIds"
                label="所属入站（可多选）"
                rules={[{ required: true, message: "至少选择一个入站" }]}
              >
                <Select
                  mode="multiple"
                  options={inbounds.map((inbound) => ({
                    value: inbound.id,
                    label: `${inbound.remark || inbound.tag} (${inbound.protocol}:${inbound.port})`,
                  }))}
                  onChange={(ids: number[]) => {
                    if (!editing) {
                      const next = ids
                        .map((id) => inboundMap.get(id))
                        .filter((inbound): inbound is Inbound => Boolean(inbound));
                      form.setFieldsValue(generated(next));
                    }
                  }}
                />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item
                name="email"
                label="邮箱 / 唯一标识"
                rules={[{ required: true, message: "请输入邮箱或唯一标识" }]}
              >
                <Input placeholder="user@example.com" />
              </Form.Item>
            </Col>
          </Row>

          <Row gutter={16}>
            {hasUuid && (
              <Col span={hasVless ? 16 : 24}>
                <Form.Item name="uuid" label="UUID">
                  <Input
                    addonAfter={
                      <Button
                        size="small"
                        type="link"
                        onClick={() => form.setFieldValue("uuid", uuid())}
                      >
                        生成
                      </Button>
                    }
                  />
                </Form.Item>
              </Col>
            )}
            {hasVless && (
              <Col span={8}>
                <Form.Item name="flow" label="Flow">
                  <Select
                    allowClear
                    options={[
                      { value: "", label: "无" },
                      { value: "xtls-rprx-vision", label: "xtls-rprx-vision" },
                    ]}
                  />
                </Form.Item>
              </Col>
            )}
            {hasPassword && (
              <Col span={12}>
                <Form.Item name="password" label="密码（Trojan / Shadowsocks）">
                  <Input
                    addonAfter={
                      <Button
                        size="small"
                        type="link"
                        onClick={() =>
                          form.setFieldValue("password", passwordFor(selectedInbounds))
                        }
                      >
                        生成
                      </Button>
                    }
                  />
                </Form.Item>
              </Col>
            )}
            {hasAccount && (
              <>
                <Col span={12}>
                  <Form.Item name="user" label="用户名">
                    <Input />
                  </Form.Item>
                </Col>
                <Col span={12}>
                  <Form.Item name="pass" label="密码">
                    <Input
                      addonAfter={
                        <Button
                          size="small"
                          type="link"
                          onClick={() => form.setFieldValue("pass", randomAlnum(16))}
                        >
                          生成
                        </Button>
                      }
                    />
                  </Form.Item>
                </Col>
              </>
            )}
          </Row>

          <Row gutter={16}>
            <Col span={8}>
              <Form.Item name="enable" label="启用" valuePropName="checked">
                <Switch />
              </Form.Item>
            </Col>
            <Col span={8}>
              <Form.Item name="totalGb" label="流量限额 (GB，0 不限)">
                <InputNumber min={0} step={1} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
            <Col span={8}>
              <Form.Item name="expiry" label="到期时间">
                <DatePicker showTime style={{ width: "100%" }} format="YYYY-MM-DD HH:mm" />
              </Form.Item>
            </Col>
          </Row>

          <Row gutter={16}>
            <Col span={12}>
              <Form.Item name="comment" label="备注">
                <Input.TextArea rows={2} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item name="groupName" label="分组">
                <Input placeholder="可选" />
              </Form.Item>
            </Col>
            <Col span={6}>
              <Form.Item name="limitIp" label="IP 限制（0 不限）">
                <InputNumber min={0} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
            <Col span={6}>
              <Form.Item name="limitHwid" label="HWID 限制（0 不限）">
                <InputNumber min={0} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
            <Col span={6}>
              <Form.Item name="resetDays" label="流量重置（天，0 不重置）">
                <InputNumber min={0} style={{ width: "100%" }} />
              </Form.Item>
            </Col>
          </Row>
        </Form>
      </Modal>
    </div>
  );
}
