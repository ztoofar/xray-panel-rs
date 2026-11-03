import { useEffect, useMemo, useState } from "react";
import {
  Alert,
  App as AntApp,
  Button,
  Card,
  Empty,
  Form,
  Input,
  Modal,
  Popconfirm,
  Select,
  Space,
  Table,
  Tag,
  Typography,
} from "antd";
import type { TableProps } from "antd";
import {
  ArrowDownOutlined,
  ArrowUpOutlined,
  DeleteOutlined,
  EditOutlined,
  PlusOutlined,
  SaveOutlined,
} from "@ant-design/icons";
import { get, put } from "../api";
import type { Inbound, XrayTemplateResponse } from "../types";

type TargetType = "outbound" | "balancer";

interface RuleFormValues {
  targetType: TargetType;
  outboundTag?: string;
  balancerTag?: string;
  domain?: string[];
  ip?: string[];
  port?: string;
  sourcePort?: string;
  localPort?: string;
  inboundTag?: string[];
  protocol?: string[];
  network?: string[];
  user?: string[];
  sourceIP?: string[];
  localIP?: string[];
  vlessRoute?: string;
  process?: string[];
  localOS?: string[];
  attrs?: string;
  domainMatcher?: string;
  ruleTag?: string;
}

interface BalancerFormValues {
  tag: string;
  selector?: string[];
  strategy?: string;
  fallbackTag?: string;
}

interface BalancerRow {
  tag: string;
  selector?: string[];
  strategy?: string | { type?: string; settings?: Record<string, unknown> };
  fallbackTag?: string;
}

function strategyType(row: BalancerRow): string {
  if (typeof row.strategy === "string") return row.strategy;
  return row.strategy?.type ?? "random";
}

type RoutingRule = Record<string, unknown>;

const joinList = (value: unknown): string =>
  Array.isArray(value) ? value.filter((item) => typeof item === "string").join(",") : "";

const asList = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];

export default function RoutingPage() {
  const { message } = AntApp.useApp();
  const [form] = Form.useForm<RuleFormValues>();
  const [balancerForm] = Form.useForm<BalancerFormValues>();
  const [routing, setRouting] = useState<Record<string, unknown>>({
    domainStrategy: "AsIs",
    rules: [],
  });
  const [rules, setRules] = useState<RoutingRule[]>([]);
  const [balancers, setBalancers] = useState<BalancerRow[]>([]);
  const [domainStrategy, setDomainStrategy] = useState("AsIs");
  const [outboundTags, setOutboundTags] = useState<string[]>([]);
  const [inbounds, setInbounds] = useState<Inbound[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [balancerModalOpen, setBalancerModalOpen] = useState(false);
  const [editingBalancer, setEditingBalancer] = useState<number | null>(null);

  const targetType = (Form.useWatch("targetType", form) ?? "outbound") as TargetType;

  const load = async () => {
    setLoading(true);
    try {
      const [template, inboundList] = await Promise.all([
        get<XrayTemplateResponse>("/api/xray/template"),
        get<Inbound[]>("/api/inbounds"),
      ]);
      const nextRouting = (template.template.routing as Record<string, unknown>) ?? {
        domainStrategy: "AsIs",
        rules: [],
      };
      setRouting(nextRouting);
      setRules((nextRouting.rules as RoutingRule[]) ?? []);
      setBalancers((nextRouting.balancers as BalancerRow[]) ?? []);
      setDomainStrategy(String(nextRouting.domainStrategy ?? "AsIs"));
      const outbounds = (template.template.outbounds as Array<Record<string, unknown>>) ?? [];
      setOutboundTags(outbounds.map((item) => String(item.tag)));
      setInbounds(inboundList);
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

  const inboundOptions = useMemo(
    () =>
      inbounds.map((inbound) => ({
        value: inbound.tag,
        label: `${inbound.remark || inbound.tag} (${inbound.protocol})`,
      })),
    [inbounds],
  );

  const outboundOptions = useMemo(() => {
    const tags = new Set(outboundTags);
    rules.forEach((rule) => {
      const tag = rule.outboundTag;
      if (typeof tag === "string") tags.add(tag);
    });
    balancers.forEach((item) => tags.delete(item.tag));
    return Array.from(tags).map((tag) => ({ value: tag, label: tag }));
  }, [outboundTags, rules, balancers]);

  const balancerOptions = useMemo(
    () => balancers.map((item) => ({ value: item.tag, label: item.tag })),
    [balancers],
  );

  const openCreate = () => {
    setEditingIndex(null);
    form.setFieldsValue({
      targetType: balancers.length > 0 ? "outbound" : "outbound",
      outboundTag: outboundOptions[0]?.value ?? "direct",
      balancerTag: balancers[0]?.tag,
      domain: [],
      ip: [],
      port: "",
      sourcePort: "",
      localPort: "",
      inboundTag: [],
      protocol: [],
      network: [],
      user: [],
      sourceIP: [],
      localIP: [],
      vlessRoute: "",
      process: [],
      localOS: [],
      attrs: "",
      domainMatcher: "hybrid",
      ruleTag: "",
    });
    setModalOpen(true);
  };

  const openEdit = (index: number) => {
    const rule = rules[index];
    setEditingIndex(index);
    const isBalancer = typeof rule.balancerTag === "string" && rule.balancerTag.length > 0;
    form.setFieldsValue({
      targetType: isBalancer ? "balancer" : "outbound",
      outboundTag: isBalancer ? undefined : String(rule.outboundTag ?? ""),
      balancerTag: isBalancer ? String(rule.balancerTag) : undefined,
      domain: asList(rule.domain),
      ip: asList(rule.ip),
      port: typeof rule.port === "string" ? rule.port : joinList(rule.port),
      sourcePort:
        typeof rule.sourcePort === "string" ? rule.sourcePort : joinList(rule.sourcePort),
      localPort:
        typeof rule.localPort === "string" ? rule.localPort : joinList(rule.localPort),
      inboundTag: asList(rule.inboundTag),
      protocol: asList(rule.protocol),
      network: asList(rule.network),
      user: asList(rule.user),
      sourceIP: asList(rule.sourceIP),
      localIP: asList(rule.localIP),
      vlessRoute:
        typeof rule.vlessRoute === "string" || typeof rule.vlessRoute === "number"
          ? String(rule.vlessRoute)
          : "",
      process: asList(rule.process),
      localOS: asList(rule.localOS),
      attrs:
        rule.attrs && typeof rule.attrs === "object"
          ? JSON.stringify(rule.attrs)
          : typeof rule.attrs === "string"
            ? rule.attrs
            : "",
      domainMatcher: typeof rule.domainMatcher === "string" ? rule.domainMatcher : "hybrid",
      ruleTag: typeof rule.ruleTag === "string" ? rule.ruleTag : "",
    });
    setModalOpen(true);
  };

  const submit = (values: RuleFormValues) => {
    const rule: RoutingRule = { type: "field" };
    if (values.targetType === "balancer") {
      rule.balancerTag = values.balancerTag;
    } else {
      rule.outboundTag = values.outboundTag;
    }
    if (values.domain?.length) rule.domain = values.domain;
    if (values.ip?.length) rule.ip = values.ip;
    if (values.port?.trim()) rule.port = values.port.trim();
    if (values.sourcePort?.trim()) rule.sourcePort = values.sourcePort.trim();
    if (values.localPort?.trim()) rule.localPort = values.localPort.trim();
    if (values.inboundTag?.length) rule.inboundTag = values.inboundTag;
    if (values.protocol?.length) rule.protocol = values.protocol;
    if (values.network?.length) rule.network = values.network.join(",");
    if (values.user?.length) rule.user = values.user;
    if (values.sourceIP?.length) rule.sourceIP = values.sourceIP;
    if (values.localIP?.length) rule.localIP = values.localIP;
    if (values.vlessRoute?.trim()) rule.vlessRoute = values.vlessRoute.trim();
    if (values.process?.length) rule.process = values.process;
    if (values.localOS?.length) rule.localOS = values.localOS;
    if (values.attrs?.trim()) {
      try {
        const parsed = JSON.parse(values.attrs);
        if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
          throw new Error("attrs 必须是 JSON 对象");
        }
        rule.attrs = parsed;
      } catch (error) {
        form.setFields([{ name: "attrs", errors: [(error as Error).message] }]);
        return;
      }
    }
    if (values.domainMatcher) rule.domainMatcher = values.domainMatcher;
    if (values.ruleTag?.trim()) rule.ruleTag = values.ruleTag.trim();

    const next = [...rules];
    if (editingIndex === null) next.push(rule);
    else next[editingIndex] = rule;
    setRules(next);
    setModalOpen(false);
  };

  const openBalancerCreate = () => {
    setEditingBalancer(null);
    balancerForm.setFieldsValue({
      tag: `balancer-${balancers.length + 1}`,
      selector: [],
      strategy: "random",
      fallbackTag: "direct",
    });
    setBalancerModalOpen(true);
  };

  const openBalancerEdit = (index: number) => {
    const row = balancers[index];
    setEditingBalancer(index);
    balancerForm.setFieldsValue({
      tag: row.tag,
      selector: row.selector ?? [],
      strategy: strategyType(row),
      fallbackTag: row.fallbackTag ?? "",
    });
    setBalancerModalOpen(true);
  };

  const submitBalancer = (values: BalancerFormValues) => {
    const next = [...balancers];
    const row: BalancerRow = {
      tag: values.tag.trim(),
      selector: values.selector ?? [],
      strategy: { type: values.strategy || "random" },
      ...(values.fallbackTag?.trim() ? { fallbackTag: values.fallbackTag.trim() } : {}),
    };
    if (editingBalancer === null) next.push(row);
    else next[editingBalancer] = row;
    setBalancers(next);
    setBalancerModalOpen(false);
  };

  const removeBalancer = (index: number) => {
    const row = balancers[index];
    const used = rules.some((rule) => rule.balancerTag === row.tag);
    if (used) {
      message.warning(`balancer ${row.tag} 正被路由规则引用，请先修改规则`);
      return;
    }
    setBalancers(balancers.filter((_, itemIndex) => itemIndex !== index));
  };

  const move = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    if (target < 0 || target >= rules.length) return;
    const next = [...rules];
    [next[index], next[target]] = [next[target], next[index]];
    setRules(next);
  };

  const remove = (index: number) => {
    setRules(rules.filter((_, itemIndex) => itemIndex !== index));
  };

  const save = async () => {
    setSaving(true);
    try {
      await put("/api/xray/routing", {
        routing: {
          ...routing,
          domainStrategy,
          rules,
          ...(balancers.length > 0 ? { balancers } : {}),
        },
      });
      message.success("路由规则已保存，Xray 已重启");
      await load();
    } catch (error) {
      message.error((error as Error).message);
    } finally {
      setSaving(false);
    }
  };

  const columns: TableProps<RoutingRule>["columns"] = [
    {
      title: "目标",
      width: 140,
      render: (_, record) => {
        const balancer = record.balancerTag;
        if (typeof balancer === "string" && balancer) {
          return <Tag color="purple">balancer:{balancer}</Tag>;
        }
        return <Tag color="blue">{String(record.outboundTag ?? "—")}</Tag>;
      },
    },
    {
      title: "域名",
      render: (_, record) => {
        const items = asList(record.domain);
        return items.length > 0 ? (
          <span className="word-break">{items.join(", ")}</span>
        ) : (
          <span className="stat-note">—</span>
        );
      },
    },
    {
      title: "IP",
      width: 170,
      render: (_, record) => {
        const items = asList(record.ip);
        return items.length > 0 ? items.join(", ") : <span className="stat-note">—</span>;
      },
    },
    {
      title: "端口",
      width: 110,
      render: (_, record) =>
        [record.port, record.sourcePort, record.localPort]
          .filter((value) => typeof value === "string" && value)
          .join(" / ") || "—",
    },
    {
      title: "入站",
      width: 150,
      render: (_, record) => {
        const items = asList(record.inboundTag);
        return items.length > 0 ? items.join(", ") : "全部";
      },
    },
    {
      title: "协议 / 网络",
      width: 140,
      render: (_, record) => {
        const protocol = asList(record.protocol).join(",");
        const network = typeof record.network === "string" ? record.network : "";
        return [protocol, network].filter(Boolean).join(" / ") || "—";
      },
    },
    {
      title: "用户 / attrs",
      width: 140,
      render: (_, record) => {
        const user = asList(record.user).join(",");
        const attrs = typeof record.attrs === "string" ? record.attrs : "";
        return [user, attrs].filter(Boolean).join(" / ") || "—";
      },
    },
    {
      title: "tag",
      width: 120,
      render: (_, record) => String(record.ruleTag ?? "—"),
    },
    {
      title: "操作",
      width: 230,
      render: (_, _record, index) => (
        <Space size="small">
          <Button
            size="small"
            type="text"
            icon={<ArrowUpOutlined />}
            disabled={index === 0}
            onClick={() => move(index, -1)}
          />
          <Button
            size="small"
            type="text"
            icon={<ArrowDownOutlined />}
            disabled={index === rules.length - 1}
            onClick={() => move(index, 1)}
          />
          <Button size="small" type="link" icon={<EditOutlined />} onClick={() => openEdit(index)}>
            编辑
          </Button>
          <Popconfirm title="删除该规则？" onConfirm={() => remove(index)}>
            <Button size="small" type="link" danger icon={<DeleteOutlined />}>
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  const balancerColumns: TableProps<BalancerRow>["columns"] = [
    { title: "Tag", dataIndex: "tag", width: 160 },
    {
      title: "选择器",
      dataIndex: "selector",
      render: (value: string[] | undefined) => (value?.length ? value.join(", ") : "—"),
    },
    {
      title: "策略",
      dataIndex: "strategy",
      width: 130,
      render: (_, record) => strategyType(record),
    },
    {
      title: "fallbackTag",
      dataIndex: "fallbackTag",
      width: 140,
      render: (value: string | undefined) => value || "—",
    },
    {
      title: "操作",
      width: 150,
      render: (_, _record, index) => (
        <Space size="small">
          <Button size="small" type="link" icon={<EditOutlined />} onClick={() => openBalancerEdit(index)}>
            编辑
          </Button>
          <Popconfirm title="删除该 balancer？" onConfirm={() => removeBalancer(index)}>
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
          路由规则
        </Typography.Title>
        <Space wrap>
          <Select
            value={domainStrategy}
            style={{ width: 190 }}
            onChange={setDomainStrategy}
            options={[
              { value: "AsIs", label: "domainStrategy: AsIs" },
              { value: "IPIfNonMatch", label: "IPIfNonMatch" },
              { value: "IPOnDemand", label: "IPOnDemand" },
            ]}
          />
          <Button icon={<PlusOutlined />} onClick={openCreate}>
            新增规则
          </Button>
          <Button type="primary" icon={<SaveOutlined />} loading={saving} onClick={save}>
            保存
          </Button>
        </Space>
      </div>

      <Alert
        type="info"
        showIcon
        style={{ marginBottom: 16 }}
        message="规则按从上到下的顺序匹配。面板会自动在最前面插入 API 入站规则；这里的规则不要引用 api。"
      />

      <Card title="规则" style={{ marginBottom: 16 }}>
        <Table
          rowKey={(_, index) => String(index)}
          loading={loading}
          columns={columns}
          dataSource={rules}
          pagination={false}
          scroll={{ x: 1250 }}
          locale={{ emptyText: <Empty description="还没有路由规则" /> }}
        />
      </Card>

      <Card
        title="Balancers"
        extra={
          <Button size="small" icon={<PlusOutlined />} onClick={openBalancerCreate}>
            新增 balancer
          </Button>
        }
      >
        <Table
          rowKey="tag"
          columns={balancerColumns}
          dataSource={balancers}
          pagination={false}
          locale={{ emptyText: <Empty description="还没有 balancer（可选）" /> }}
        />
      </Card>

      <Modal
        open={modalOpen}
        title={editingIndex === null ? "新增路由规则" : "编辑路由规则"}
        width={760}
        onCancel={() => setModalOpen(false)}
        onOk={() => form.submit()}
        okText="确定"
        cancelText="取消"
        destroyOnHidden
      >
        <Form form={form} layout="vertical" onFinish={submit} requiredMark={false} preserve={false}>
          <Space style={{ width: "100%" }} align="start">
            <Form.Item name="targetType" label="目标类型" style={{ width: 150 }}>
              <Select
                options={[
                  { value: "outbound", label: "出站" },
                  { value: "balancer", label: "Balancer" },
                ]}
              />
            </Form.Item>
            {targetType === "balancer" ? (
              <Form.Item
                name="balancerTag"
                label="Balancer Tag"
                rules={[{ required: true, message: "请选择 balancer" }]}
                style={{ width: 260 }}
              >
                <Select options={balancerOptions} showSearch />
              </Form.Item>
            ) : (
              <Form.Item
                name="outboundTag"
                label="出站 Tag"
                rules={[{ required: true, message: "请选择出站" }]}
                style={{ width: 260 }}
              >
                <Select options={outboundOptions} showSearch />
              </Form.Item>
            )}
            <Form.Item name="ruleTag" label="规则 tag" style={{ width: 220 }}>
              <Input placeholder="可选" />
            </Form.Item>
          </Space>
          <Form.Item name="domain" label="域名（回车添加，支持 geosite: / regexp: / domain:）">
            <Select mode="tags" tokenSeparators={[",", "\n"]} placeholder="geosite:google" />
          </Form.Item>
          <Form.Item name="ip" label="IP（回车添加，支持 geoip: / geoip:!cn / CIDR）">
            <Select mode="tags" tokenSeparators={[",", "\n"]} placeholder="geoip:cn" />
          </Form.Item>
          <Space style={{ width: "100%" }} align="start">
            <Form.Item name="port" label="目标端口" style={{ width: 220 }}>
              <Input placeholder="443 或 1000-2000" />
            </Form.Item>
            <Form.Item name="sourcePort" label="来源端口" style={{ width: 220 }}>
              <Input placeholder="逗号分隔" />
            </Form.Item>
            <Form.Item name="localPort" label="本地端口" style={{ width: 220 }}>
              <Input placeholder="逗号分隔" />
            </Form.Item>
          </Space>
          <Form.Item name="inboundTag" label="匹配入站">
            <Select mode="multiple" options={inboundOptions} allowClear />
          </Form.Item>
          <Space style={{ width: "100%" }} align="start">
            <Form.Item name="protocol" label="协议" style={{ width: 300 }}>
              <Select
                mode="multiple"
                options={["http", "tls", "quic", "bittorrent"].map((value) => ({ value, label: value }))}
                allowClear
              />
            </Form.Item>
            <Form.Item name="network" label="网络" style={{ width: 220 }}>
              <Select
                mode="multiple"
                options={["tcp", "udp"].map((value) => ({ value, label: value }))}
                allowClear
              />
            </Form.Item>
            <Form.Item name="domainMatcher" label="域名匹配器" style={{ width: 180 }}>
              <Select
                options={[
                  { value: "hybrid", label: "hybrid" },
                  { value: "linear", label: "linear" },
                ]}
              />
            </Form.Item>
          </Space>
          <Space style={{ width: "100%" }} align="start">
            <Form.Item name="user" label="用户 email（回车添加）" style={{ width: 300 }}>
              <Select mode="tags" tokenSeparators={[",", "\n"]} />
            </Form.Item>
            <Form.Item name="sourceIP" label="来源 IP" style={{ width: 300 }}>
              <Select mode="tags" tokenSeparators={[",", "\n"]} placeholder="CIDR" />
            </Form.Item>
            <Form.Item name="localIP" label="本地 IP" style={{ width: 300 }}>
              <Select mode="tags" tokenSeparators={[",", "\n"]} placeholder="CIDR" />
            </Form.Item>
          </Space>
          <Space style={{ width: "100%" }} align="start">
            <Form.Item name="vlessRoute" label="vlessRoute" style={{ width: 160 }}>
              <Input placeholder="0-65535" />
            </Form.Item>
            <Form.Item name="process" label="进程名" style={{ width: 300 }}>
              <Select mode="tags" tokenSeparators={[",", "\n"]} />
            </Form.Item>
            <Form.Item name="localOS" label="localOS" style={{ width: 300 }}>
              <Select
                mode="tags"
                tokenSeparators={[",", "\n"]}
                options={["linux", "windows", "darwin", "android", "ios"].map((value) => ({
                  value,
                  label: value,
                }))}
              />
            </Form.Item>
          </Space>
          <Form.Item name="attrs" label="attrs（JSON 对象）">
            <Input placeholder='例如 {"auth:user": "admin"}' />
          </Form.Item>
        </Form>
      </Modal>

      <Modal
        open={balancerModalOpen}
        title={editingBalancer === null ? "新增 balancer" : "编辑 balancer"}
        width={620}
        onCancel={() => setBalancerModalOpen(false)}
        onOk={() => balancerForm.submit()}
        okText="确定"
        cancelText="取消"
        destroyOnHidden
      >
        <Form
          form={balancerForm}
          layout="vertical"
          onFinish={submitBalancer}
          requiredMark={false}
          preserve={false}
        >
          <Form.Item name="tag" label="Tag" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="selector" label="出站选择器（Tag）">
            <Select mode="tags" options={outboundOptions} tokenSeparators={[",", "\n"]} />
          </Form.Item>
          <Form.Item name="strategy" label="策略">
            <Select
              options={["random", "roundRobin", "leastPing", "leastLoad"].map((value) => ({
                value,
                label: value,
              }))}
            />
          </Form.Item>
          <Form.Item name="fallbackTag" label="fallbackTag">
            <Select allowClear options={outboundOptions} />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}
