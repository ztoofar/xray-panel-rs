import { useEffect, useState } from "react";
import {
  Alert,
  App as AntApp,
  Button,
  Card,
  Col,
  DatePicker,
  Divider,
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
  Tabs,
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
import type { Client, Inbound, LinksResponse } from "../types";
import { formatBytes, formatTime } from "../util";

type Protocol = "vless" | "vmess" | "trojan" | "shadowsocks" | "http" | "mixed" | "wireguard";
type Network = "tcp" | "ws" | "grpc" | "httpupgrade" | "xhttp" | "kcp" | "quic" | "http";
type Security = "none" | "tls" | "reality";

const protocols = [
  { value: "vless", label: "VLESS" },
  { value: "vmess", label: "VMess" },
  { value: "trojan", label: "Trojan" },
  { value: "shadowsocks", label: "Shadowsocks" },
  { value: "http", label: "HTTP" },
  { value: "mixed", label: "Mixed" },
  { value: "wireguard", label: "WireGuard" },
];

const resetOptions = [
  { value: "never", label: "不重置" },
  { value: "hourly", label: "每小时" },
  { value: "daily", label: "每天" },
  { value: "weekly", label: "每周" },
  { value: "monthly", label: "每月" },
];

const ssMethods = [
  "2022-blake3-aes-128-gcm",
  "2022-blake3-aes-256-gcm",
  "chacha20-ietf-poly1305",
  "aes-256-gcm",
  "aes-128-gcm",
].map((value) => ({ value, label: value }));

const protocolColors: Record<string, string> = {
  vless: "blue",
  vmess: "geekblue",
  trojan: "purple",
  shadowsocks: "green",
  http: "orange",
  mixed: "gold",
};

interface ClientRow {
  email?: string;
  credential?: string;
  flow?: string;
  alterId?: number;
  security?: string;
  user?: string;
  pass?: string;
  enable?: boolean;
  totalGb?: number;
  expiry?: Dayjs | null;
  limitIp?: number;
  limitHwid?: number;
  resetDays?: number;
  comment?: string;
}

interface FallbackRow {
  name?: string;
  alpn?: string;
  path?: string;
  dest?: string;
  xver?: number;
}

interface WgPeerRow {
  privateKey?: string;
  publicKey?: string;
  allowedIPs?: string;
  keepAlive?: number;
  comment?: string;
}

interface InboundFormValues {
  remark?: string;
  protocol: Protocol;
  port: number;
  listen?: string;
  tag?: string;
  enable: boolean;
  totalGb?: number;
  expiry?: Dayjs | null;
  trafficReset: string;
  trafficResetDay?: number;
  method?: string;
  ssNetwork?: string;
  ivCheck?: boolean;
  clients?: ClientRow[];
  fallbacks?: FallbackRow[];
  wgSecretKey?: string;
  wgPublicKey?: string;
  wgMtu?: number;
  wgDns?: string;
  wgWorkers?: number;
  wgNoKernelTun?: boolean;
  wgSubnetIp?: string;
  wgSubnetCidr?: number;
  wgPeers?: WgPeerRow[];
  network: Network;
  security: Security;
  tlsServerName?: string;
  tlsAlpn?: string[];
  tlsFingerprint?: string;
  tlsAllowInsecure?: boolean;
  tlsCertFile?: string;
  tlsKeyFile?: string;
  tlsCertContent?: string;
  tlsKeyContent?: string;
  realityTarget?: string;
  realityServerNames?: string[];
  realityShortIds?: string[];
  realityPrivateKey?: string;
  realityPublicKey?: string;
  realitySpiderX?: string;
  tcpHeaderType?: string;
  tcpHost?: string;
  tcpPath?: string;
  wsPath?: string;
  wsHost?: string;
  wsHeartbeat?: number;
  grpcService?: string;
  grpcAuthority?: string;
  grpcMultiMode?: boolean;
  httpupgradePath?: string;
  httpupgradeHost?: string;
  xhttpPath?: string;
  xhttpHost?: string;
  xhttpMode?: string;
  kcpMtu?: number;
  kcpTti?: number;
  kcpUplink?: number;
  kcpDownlink?: number;
  kcpCongestion?: boolean;
  kcpReadBuffer?: number;
  kcpWriteBuffer?: number;
  kcpHeaderType?: string;
  kcpSeed?: string;
  quicKey?: string;
  quicHeaderType?: string;
  httpPath?: string;
  httpHost?: string;
  sockoptTcpFastOpen?: boolean;
  sockoptTcpNoDelay?: boolean;
  sockoptTcpKeepAlive?: boolean;
  sockoptTcpKeepAliveIdle?: number;
  sockoptTcpKeepAliveInterval?: number;
  sockoptTcpCongestion?: string;
  sockoptTcpMptcp?: boolean;
  sockoptAcceptProxyProtocol?: boolean;
  sockoptDomainStrategy?: string;
  sockoptDialerProxy?: string;
  sockoptMark?: number;
  sockoptInterface?: string;
  sockoptTproxy?: string;
  sniffEnabled: boolean;
  sniffDest?: string[];
  sniffDomainsExcluded?: string[];
  sniffMetadataOnly?: boolean;
  sniffRouteOnly?: boolean;
}

const defaultValues: InboundFormValues = {
  remark: "",
  protocol: "vless",
  port: 443,
  listen: "",
  tag: "",
  enable: true,
  totalGb: 0,
  expiry: null,
  trafficReset: "never",
  trafficResetDay: 1,
  method: "2022-blake3-aes-128-gcm",
  ssNetwork: "tcp,udp",
  ivCheck: false,
  clients: [],
  fallbacks: [],
  wgSecretKey: "",
  wgPublicKey: "",
  wgMtu: 1420,
  wgDns: "1.1.1.1, 1.0.0.1",
  wgWorkers: 2,
  wgNoKernelTun: false,
  wgSubnetIp: "",
  wgSubnetCidr: 24,
  wgPeers: [],
  network: "tcp",
  security: "none",
  tlsAlpn: ["h2", "http/1.1"],
  tlsFingerprint: "chrome",
  tlsAllowInsecure: false,
  realityServerNames: [],
  realityShortIds: [],
  realitySpiderX: "/",
  tcpHeaderType: "none",
  wsPath: "/",
  wsHeartbeat: 0,
  grpcMultiMode: false,
  httpupgradePath: "/",
  xhttpPath: "/",
  xhttpMode: "auto",
  kcpMtu: 1350,
  kcpTti: 50,
  kcpUplink: 5,
  kcpDownlink: 20,
  kcpCongestion: false,
  kcpReadBuffer: 2,
  kcpWriteBuffer: 2,
  kcpHeaderType: "none",
  quicHeaderType: "none",
  httpPath: "/",
  sockoptTcpFastOpen: false,
  sockoptTcpNoDelay: false,
  sockoptTcpKeepAlive: false,
  sockoptTcpKeepAliveIdle: 0,
  sockoptTcpKeepAliveInterval: 0,
  sockoptTcpMptcp: false,
  sockoptAcceptProxyProtocol: false,
  sniffEnabled: true,
  sniffDest: ["http", "tls", "quic"],
  sniffDomainsExcluded: [],
  sniffMetadataOnly: false,
  sniffRouteOnly: false,
};

function randomInt(max: number): number {
  const buffer = new Uint32Array(1);
  if (window.crypto?.getRandomValues) {
    window.crypto.getRandomValues(buffer);
    return buffer[0] % max;
  }
  return Math.floor(Math.random() * max);
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

function randomAlnum(length: number): string {
  const alphabet = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
  let out = "";
  for (let i = 0; i < length; i += 1) out += alphabet[randomInt(alphabet.length)];
  return out;
}

function randomBase64(bytes: number): string {
  const buffer = new Uint8Array(bytes);
  if (window.crypto?.getRandomValues) window.crypto.getRandomValues(buffer);
  else for (let i = 0; i < buffer.length; i += 1) buffer[i] = Math.floor(Math.random() * 256);
  return btoa(String.fromCharCode(...buffer));
}

function generatedClient(protocol: Protocol, method?: string): ClientRow {
  const email = `user-${randomAlnum(6)}@panel`;
  if (protocol === "vless") return { email, credential: uuid(), flow: "", enable: true };
  if (protocol === "vmess") return { email, credential: uuid(), alterId: 0, security: "auto", enable: true };
  if (protocol === "shadowsocks") {
    const bytes = method?.includes("256") ? 32 : 16;
    return { email, credential: randomBase64(bytes), enable: true };
  }
  if (protocol === "http" || protocol === "mixed") {
    return { email, user: `user${randomAlnum(4)}`, pass: randomAlnum(16), enable: true };
  }
  return { email, credential: randomAlnum(20), enable: true };
}

function buildStreamSettings(values: InboundFormValues): Record<string, unknown> {
  const stream: Record<string, unknown> = { network: values.network };
  if (values.network === "tcp") {
    if (values.tcpHeaderType === "http") {
      stream.tcpSettings = {
        header: {
          type: "http",
          request: {
            path: [values.tcpPath || "/"],
            headers: { Host: [values.tcpHost || ""] },
          },
        },
      };
    }
  } else if (values.network === "ws") {
    stream.wsSettings = {
      path: values.wsPath || "/",
      ...(values.wsHost ? { headers: { Host: values.wsHost } } : {}),
      ...(values.wsHeartbeat ? { heartbeatPeriod: values.wsHeartbeat } : {}),
    };
  } else if (values.network === "grpc") {
    stream.grpcSettings = {
      serviceName: values.grpcService || "",
      ...(values.grpcAuthority ? { authority: values.grpcAuthority } : {}),
      multiMode: Boolean(values.grpcMultiMode),
    };
  } else if (values.network === "httpupgrade") {
    stream.httpupgradeSettings = {
      path: values.httpupgradePath || "/",
      ...(values.httpupgradeHost ? { host: values.httpupgradeHost } : {}),
    };
  } else if (values.network === "xhttp") {
    stream.xhttpSettings = {
      path: values.xhttpPath || "/",
      ...(values.xhttpHost ? { host: values.xhttpHost } : {}),
      mode: values.xhttpMode || "auto",
    };
  } else if (values.network === "kcp") {
    stream.kcpSettings = {
      mtu: values.kcpMtu ?? 1350,
      tti: values.kcpTti ?? 50,
      uplinkCapacity: values.kcpUplink ?? 5,
      downlinkCapacity: values.kcpDownlink ?? 20,
      congestion: Boolean(values.kcpCongestion),
      readBufferSize: values.kcpReadBuffer ?? 2,
      writeBufferSize: values.kcpWriteBuffer ?? 2,
      header: { type: values.kcpHeaderType || "none" },
      ...(values.kcpSeed ? { seed: values.kcpSeed } : {}),
    };
  } else if (values.network === "quic") {
    stream.quicSettings = {
      ...(values.quicKey ? { key: values.quicKey } : {}),
      header: { type: values.quicHeaderType || "none" },
    };
  } else if (values.network === "http") {
    stream.httpSettings = {
      path: values.httpPath || "/",
      ...(values.httpHost ? { host: [values.httpHost] } : {}),
    };
  }

  if (values.security === "tls") {
    stream.security = "tls";
    stream.tlsSettings = {
      ...(values.tlsServerName ? { serverName: values.tlsServerName } : {}),
      ...(values.tlsAlpn?.length ? { alpn: values.tlsAlpn } : {}),
      ...(values.tlsAllowInsecure ? { allowInsecure: true } : {}),
      ...(values.tlsCertFile || values.tlsKeyFile || values.tlsCertContent || values.tlsKeyContent
        ? {
            certificates: [
              {
                ...(values.tlsCertFile ? { certificateFile: values.tlsCertFile } : {}),
                ...(values.tlsKeyFile ? { keyFile: values.tlsKeyFile } : {}),
                ...(values.tlsCertContent ? { certificate: values.tlsCertContent } : {}),
                ...(values.tlsKeyContent ? { key: values.tlsKeyContent } : {}),
              },
            ],
          }
        : {}),
      settings: {
        fingerprint: values.tlsFingerprint || "chrome",
        ...(values.tlsAllowInsecure ? { allowInsecure: true } : {}),
      },
    };
  } else if (values.security === "reality") {
    stream.security = "reality";
    stream.realitySettings = {
      ...(values.realityTarget ? { target: values.realityTarget } : {}),
      ...(values.realityServerNames?.length ? { serverNames: values.realityServerNames } : {}),
      ...(values.realityShortIds?.length ? { shortIds: values.realityShortIds } : {}),
      ...(values.realityPrivateKey ? { privateKey: values.realityPrivateKey } : {}),
      settings: {
        ...(values.realityPublicKey ? { publicKey: values.realityPublicKey } : {}),
        fingerprint: values.tlsFingerprint || "chrome",
        ...(values.realitySpiderX ? { spiderX: values.realitySpiderX } : {}),
        ...(values.tlsServerName ? { serverName: values.tlsServerName } : {}),
      },
    };
  } else {
    stream.security = "none";
  }

  const sockopt: Record<string, unknown> = {};
  if (values.sockoptTcpFastOpen) sockopt.tcpFastOpen = true;
  if (values.sockoptTcpNoDelay) sockopt.tcpNoDelay = true;
  if (values.sockoptTcpKeepAlive) sockopt.tcpKeepAlive = true;
  if (values.sockoptTcpKeepAliveIdle) sockopt.tcpKeepAliveIdle = values.sockoptTcpKeepAliveIdle;
  if (values.sockoptTcpKeepAliveInterval) {
    sockopt.tcpKeepAliveInterval = values.sockoptTcpKeepAliveInterval;
  }
  if (values.sockoptTcpCongestion) sockopt.tcpCongestion = values.sockoptTcpCongestion;
  if (values.sockoptTcpMptcp) sockopt.tcpMptcp = true;
  if (values.sockoptAcceptProxyProtocol) sockopt.acceptProxyProtocol = true;
  if (values.sockoptDomainStrategy) sockopt.domainStrategy = values.sockoptDomainStrategy;
  if (values.sockoptDialerProxy) sockopt.dialerProxy = values.sockoptDialerProxy;
  if (values.sockoptMark) sockopt.mark = values.sockoptMark;
  if (values.sockoptInterface) sockopt.interface = values.sockoptInterface;
  if (values.sockoptTproxy) sockopt.tproxy = values.sockoptTproxy;
  if (Object.keys(sockopt).length > 0) stream.sockopt = sockopt;

  return stream;
}

export default function InboundsPage() {
  const { message } = AntApp.useApp();
  const [form] = Form.useForm<InboundFormValues>();
  const [inbounds, setInbounds] = useState<Inbound[]>([]);
  const [loading, setLoading] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Inbound | null>(null);
  const [links, setLinks] = useState<LinksResponse>();
  const [linksOpen, setLinksOpen] = useState(false);

  const protocol = (Form.useWatch("protocol", form) ?? "vless") as Protocol;
  const method = Form.useWatch("method", form);
  const network = Form.useWatch("network", form);
  const security = Form.useWatch("security", form);

  const load = async () => {
    setLoading(true);
    try {
      setInbounds(await get<Inbound[]>("/api/inbounds"));
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

  const openCreate = () => {
    setEditing(null);
    form.setFieldsValue({ ...defaultValues });
    setModalOpen(true);
  };

  const openEdit = async (record: Inbound) => {
    setEditing(record);
    let clients: Client[] = [];
    try {
      clients = await get<Client[]>(`/api/clients?inboundId=${record.id}`);
    } catch {
      clients = [];
    }
    const settings = (record.settings ?? {}) as Record<string, any>;
    const stream = (record.streamSettings ?? {}) as Record<string, any>;
    const tls = (stream.tlsSettings ?? {}) as Record<string, any>;
    const reality = (stream.realitySettings ?? {}) as Record<string, any>;
    const realitySettingsInner = (reality.settings ?? {}) as Record<string, any>;
    const certificate = tls.certificates?.[0] ?? {};
    const sockopt = (stream.sockopt ?? {}) as Record<string, any>;
    const sniff = (record.sniffing ?? {}) as Record<string, any>;
    form.setFieldsValue({
      ...defaultValues,
      remark: record.remark,
      protocol: record.protocol as Protocol,
      port: record.port,
      listen: record.listen,
      tag: record.tag,
      enable: record.enable,
      totalGb: record.total > 0 ? Number((record.total / 1024 ** 3).toFixed(2)) : 0,
      expiry: record.expiryTime ? dayjs.unix(record.expiryTime) : null,
      trafficReset: record.trafficReset,
      trafficResetDay: record.trafficResetDay,
      wgSecretKey: settings.secretKey ?? "",
      wgPublicKey: "",
      wgMtu: settings.mtu ?? 1420,
      wgDns: settings.dns ?? "1.1.1.1, 1.0.0.1",
      wgWorkers: settings.workers ?? 2,
      wgNoKernelTun: Boolean(settings.noKernelTun),
      wgSubnetIp: settings.subnetIp ?? "",
      wgSubnetCidr: settings.subnetCidr ?? 24,
      wgPeers: (settings.peers ?? []).map((peer: any) => ({
        privateKey: peer.privateKey ?? "",
        publicKey: peer.publicKey ?? "",
        allowedIPs: Array.isArray(peer.allowedIPs) ? peer.allowedIPs.join(",") : "",
        keepAlive: peer.keepAlive ?? 0,
        comment: peer.comment ?? "",
      })),
      method: settings.method ?? "2022-blake3-aes-128-gcm",
      ssNetwork: settings.network ?? "tcp,udp",
      ivCheck: Boolean(settings.ivCheck),
      fallbacks: (settings.fallbacks ?? []).map((fallback: any) => ({
        name: fallback.name ?? "",
        alpn: fallback.alpn ?? "",
        path: fallback.path ?? "",
        dest: fallback.dest ?? "",
        xver: fallback.xver ?? 0,
      })),
      clients: clients.map((client) => {
        const creds = (client.settings ?? {}) as Record<string, any>;
        return {
          email: client.email,
          credential:
            record.protocol === "vless" || record.protocol === "vmess"
              ? creds.id ?? ""
              : creds.password ?? "",
          flow: creds.flow ?? "",
          alterId: creds.alterId ?? 0,
          security: creds.security ?? "auto",
          user: creds.user ?? "",
          pass: creds.pass ?? "",
          enable: client.enable,
          totalGb: client.total > 0 ? Number((client.total / 1024 ** 3).toFixed(2)) : 0,
          expiry: client.expiryTime ? dayjs.unix(client.expiryTime) : null,
          limitIp: client.limitIp ?? 0,
          limitHwid: client.limitHwid ?? 0,
          resetDays: client.resetDays ?? 0,
          comment: client.comment ?? "",
        };
      }),
      network: (stream.network as Network) ?? "tcp",
      security: (stream.security as Security) ?? "none",
      tlsServerName: tls.serverName ?? realitySettingsInner.serverName ?? "",
      tlsAlpn: tls.alpn ?? ["h2", "http/1.1"],
      tlsFingerprint:
        tls.settings?.fingerprint ?? realitySettingsInner.fingerprint ?? "chrome",
      tlsAllowInsecure: Boolean(tls.allowInsecure || tls.settings?.allowInsecure),
      tlsCertFile: certificate.certificateFile ?? "",
      tlsKeyFile: certificate.keyFile ?? "",
      tlsCertContent: certificate.certificate ?? "",
      tlsKeyContent: certificate.key ?? "",
      realityTarget: reality.target ?? "",
      realityServerNames: reality.serverNames ?? [],
      realityShortIds: reality.shortIds ?? [],
      realityPrivateKey: reality.privateKey ?? "",
      realityPublicKey: realitySettingsInner.publicKey ?? "",
      realitySpiderX: realitySettingsInner.spiderX ?? "/",
      tcpHeaderType: stream.tcpSettings?.header?.type ?? "none",
      tcpHost: stream.tcpSettings?.header?.request?.headers?.Host?.[0] ?? "",
      tcpPath: stream.tcpSettings?.header?.request?.path?.[0] ?? "",
      wsPath: stream.wsSettings?.path ?? "/",
      wsHost: stream.wsSettings?.headers?.Host ?? "",
      wsHeartbeat: stream.wsSettings?.heartbeatPeriod ?? 0,
      grpcService: stream.grpcSettings?.serviceName ?? "",
      grpcAuthority: stream.grpcSettings?.authority ?? "",
      grpcMultiMode: Boolean(stream.grpcSettings?.multiMode),
      httpupgradePath: stream.httpupgradeSettings?.path ?? "/",
      httpupgradeHost: stream.httpupgradeSettings?.host ?? "",
      xhttpPath: stream.xhttpSettings?.path ?? "/",
      xhttpHost: stream.xhttpSettings?.host ?? "",
      xhttpMode: stream.xhttpSettings?.mode ?? "auto",
      kcpMtu: stream.kcpSettings?.mtu ?? 1350,
      kcpTti: stream.kcpSettings?.tti ?? 50,
      kcpUplink: stream.kcpSettings?.uplinkCapacity ?? 5,
      kcpDownlink: stream.kcpSettings?.downlinkCapacity ?? 20,
      kcpCongestion: Boolean(stream.kcpSettings?.congestion),
      kcpReadBuffer: stream.kcpSettings?.readBufferSize ?? 2,
      kcpWriteBuffer: stream.kcpSettings?.writeBufferSize ?? 2,
      kcpHeaderType: stream.kcpSettings?.header?.type ?? "none",
      kcpSeed: stream.kcpSettings?.seed ?? "",
      quicKey: stream.quicSettings?.key ?? "",
      quicHeaderType: stream.quicSettings?.header?.type ?? "none",
      httpPath: stream.httpSettings?.path ?? "/",
      httpHost: stream.httpSettings?.host?.[0] ?? "",
      sockoptTcpFastOpen: Boolean(sockopt.tcpFastOpen),
      sockoptTcpNoDelay: Boolean(sockopt.tcpNoDelay),
      sockoptTcpKeepAlive: Boolean(sockopt.tcpKeepAlive),
      sockoptTcpKeepAliveIdle: sockopt.tcpKeepAliveIdle ?? 0,
      sockoptTcpKeepAliveInterval: sockopt.tcpKeepAliveInterval ?? 0,
      sockoptTcpCongestion: sockopt.tcpCongestion ?? "",
      sockoptTcpMptcp: Boolean(sockopt.tcpMptcp),
      sockoptAcceptProxyProtocol: Boolean(sockopt.acceptProxyProtocol),
      sockoptDomainStrategy: sockopt.domainStrategy ?? "",
      sockoptDialerProxy: sockopt.dialerProxy ?? "",
      sockoptMark: sockopt.mark ?? 0,
      sockoptInterface: sockopt.interface ?? "",
      sockoptTproxy: sockopt.tproxy ?? "",
      sniffEnabled: Boolean(sniff.enabled),
      sniffDest: sniff.destOverride ?? ["http", "tls", "quic"],
      sniffDomainsExcluded: sniff.domainsExcluded ?? [],
      sniffMetadataOnly: Boolean(sniff.metadataOnly),
      sniffRouteOnly: Boolean(sniff.routeOnly),
    });
    setModalOpen(true);
  };

  const onProtocolChange = (next: Protocol) => {
    form.setFieldsValue({
      method: next === "shadowsocks" ? "2022-blake3-aes-128-gcm" : form.getFieldValue("method"),
      clients: [],
      fallbacks: [],
      security: next === "shadowsocks" ? "none" : form.getFieldValue("security"),
    });
  };

  const generateRealityKey = async () => {
    try {
      const keys = await post<{ privateKey: string; publicKey: string }>("/api/xray/new-x25519");
      form.setFieldsValue({
        realityPrivateKey: keys.privateKey,
        realityPublicKey: keys.publicKey,
      });
      message.success("REALITY 密钥已生成");
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const generateWireguardKey = async () => {
    try {
      const keys = await post<{ privateKey: string; publicKey: string }>("/api/xray/new-x25519");
      form.setFieldsValue({ wgSecretKey: keys.privateKey, wgPublicKey: keys.publicKey });
      message.success("WireGuard 服务端密钥已生成");
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const generatePeerKey = async (index: number) => {
    try {
      const keys = await post<{ privateKey: string; publicKey: string }>("/api/xray/new-x25519");
      const current = [...(form.getFieldValue("wgPeers") ?? [])];
      current[index] = {
        ...(current[index] ?? {}),
        privateKey: keys.privateKey,
        publicKey: keys.publicKey,
      };
      form.setFieldValue("wgPeers", current);
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const addClient = () => {
    const current = form.getFieldValue("clients") ?? [];
    form.setFieldValue("clients", [...current, generatedClient(protocol, method)]);
  };

  const regenerate = (index: number) => {
    const current = [...(form.getFieldValue("clients") ?? [])];
    const row = current[index] ?? {};
    const fresh = generatedClient(protocol, method);
    current[index] = {
      ...row,
      credential: fresh.credential,
      user: fresh.user,
      pass: fresh.pass,
    };
    form.setFieldValue("clients", current);
  };

  const buildSettings = (values: InboundFormValues): Record<string, unknown> => {
    const rows = (values.clients ?? []).filter(
      (row) => row.email?.trim() || row.user?.trim(),
    );
    const clients = rows.map((row) => {
      const email = row.email?.trim() || row.user?.trim() || "user";
      const extras = {
        totalGB: Math.round((row.totalGb ?? 0) * 1024 ** 3),
        expiryTime: row.expiry ? row.expiry.unix() : 0,
        limitIp: row.limitIp ?? 0,
        limitHwid: row.limitHwid ?? 0,
        reset: row.resetDays ?? 0,
        comment: row.comment ?? "",
        ...(row.enable === false ? { enable: false } : {}),
      };
      if (values.protocol === "vless") {
        return {
          email,
          id: row.credential || uuid(),
          ...(row.flow ? { flow: row.flow } : {}),
          ...extras,
        };
      }
      if (values.protocol === "vmess") {
        return {
          email,
          id: row.credential || uuid(),
          alterId: row.alterId ?? 0,
          security: row.security || "auto",
          ...extras,
        };
      }
      if (values.protocol === "shadowsocks") {
        const bytes = values.method?.includes("256") ? 32 : 16;
        return {
          email,
          password: row.credential || randomBase64(bytes),
          ...extras,
        };
      }
      if (values.protocol === "http" || values.protocol === "mixed") {
        return {
          user: row.user?.trim() || email,
          pass: row.pass || randomAlnum(16),
          ...extras,
        };
      }
      return {
        email,
        password: row.credential || randomAlnum(20),
        ...extras,
      };
    });
    const fallbacks = (values.fallbacks ?? [])
      .filter((fallback) => fallback.dest?.trim() || fallback.name?.trim())
      .map((fallback) => ({
        name: fallback.name || undefined,
        alpn: fallback.alpn || undefined,
        path: fallback.path || undefined,
        dest: fallback.dest ?? "",
        xver: fallback.xver ?? 0,
      }));

    if (values.protocol === "wireguard") {
      return {
        secretKey: values.wgSecretKey?.trim() || "",
        mtu: values.wgMtu ?? 1420,
        workers: values.wgWorkers ?? 2,
        noKernelTun: Boolean(values.wgNoKernelTun),
        ...(values.wgDns?.trim() ? { dns: values.wgDns.trim() } : {}),
        ...(values.wgSubnetIp?.trim() ? { subnetIp: values.wgSubnetIp.trim() } : {}),
        ...(values.wgSubnetCidr ? { subnetCidr: values.wgSubnetCidr } : {}),
        peers: (values.wgPeers ?? [])
          .filter((peer) => peer.publicKey?.trim())
          .map((peer) => ({
            ...(peer.privateKey?.trim() ? { privateKey: peer.privateKey.trim() } : {}),
            publicKey: peer.publicKey?.trim(),
            allowedIPs: (peer.allowedIPs ?? "")
              .split(/[,\n]/)
              .map((item) => item.trim())
              .filter(Boolean),
            ...(peer.keepAlive ? { keepAlive: peer.keepAlive } : {}),
            ...(peer.comment?.trim() ? { comment: peer.comment.trim() } : {}),
          })),
      };
    }
    if (values.protocol === "vless") {
      return { decryption: "none", clients, ...(fallbacks.length ? { fallbacks } : {}) };
    }
    if (values.protocol === "vmess") {
      return { clients };
    }
    if (values.protocol === "trojan") {
      return { clients, ...(fallbacks.length ? { fallbacks } : {}) };
    }
    if (values.protocol === "shadowsocks") {
      return {
        method: values.method || "2022-blake3-aes-128-gcm",
        network: values.ssNetwork || "tcp,udp",
        ivCheck: Boolean(values.ivCheck),
        clients,
      };
    }
    return { accounts: clients };
  };

  const submit = async (values: InboundFormValues) => {
    try {
      const payload = {
        remark: values.remark ?? "",
        protocol: values.protocol,
        port: values.port,
        listen: values.listen ?? "",
        tag: values.tag?.trim() || undefined,
        enable: values.enable,
        total: Math.round((values.totalGb ?? 0) * 1024 ** 3),
        expiryTime: values.expiry ? values.expiry.unix() : 0,
        trafficReset: values.trafficReset,
        trafficResetDay: values.trafficResetDay ?? 1,
        settings: buildSettings(values),
        streamSettings: buildStreamSettings(values),
        sniffing: values.sniffEnabled
          ? {
              enabled: true,
              destOverride: values.sniffDest ?? [],
              ...(values.sniffDomainsExcluded?.length
                ? { domainsExcluded: values.sniffDomainsExcluded }
                : {}),
              ...(values.sniffMetadataOnly ? { metadataOnly: true } : {}),
              ...(values.sniffRouteOnly ? { routeOnly: true } : {}),
            }
          : { enabled: false },
      };
      if (editing) {
        await put(`/api/inbounds/${editing.id}`, payload);
      } else {
        await post("/api/inbounds", payload);
      }
      message.success("入站已保存，Xray 已应用配置");
      setModalOpen(false);
      await load();
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const toggle = async (record: Inbound, enable: boolean) => {
    try {
      await post(`/api/inbounds/${record.id}/toggle`, { enable });
      await load();
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const remove = async (record: Inbound) => {
    try {
      await del(`/api/inbounds/${record.id}`);
      message.success("入站已删除");
      await load();
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const openLinks = async (record: Inbound) => {
    try {
      setLinks(await get<LinksResponse>(`/api/inbounds/${record.id}/links`));
      setLinksOpen(true);
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const columns: TableProps<Inbound>["columns"] = [
    {
      title: "备注",
      dataIndex: "remark",
      render: (_, record) => (
        <div>
          <Typography.Text strong>{record.remark || "(未命名)"}</Typography.Text>
          <div className="stat-note">{record.tag}</div>
        </div>
      ),
    },
    {
      title: "协议",
      dataIndex: "protocol",
      width: 120,
      render: (value: string) => <Tag color={protocolColors[value]}>{value.toUpperCase()}</Tag>,
    },
    {
      title: "监听",
      width: 160,
      render: (_, record) => `${record.listen || "0.0.0.0"}:${record.port}`,
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
    { title: "客户端", width: 90, render: (_, record) => record.clientCount ?? 0 },
    {
      title: "在线",
      width: 110,
      render: (_, record) => {
        const count = record.onlineCount ?? 0;
        if (count === 0) return <Typography.Text type="secondary">0</Typography.Text>;
        return (
          <Popover
            title="在线客户端"
            content={
              <div style={{ maxWidth: 380 }}>
                {(record.onlineClients ?? []).map((client) => (
                  <div key={client.email} style={{ marginBottom: 6 }}>
                    <Typography.Text strong>{client.email}</Typography.Text>
                    <div className="stat-note" style={{ whiteSpace: "pre-line" }}>
                      {client.ips.length > 0
                        ? client.ips
                            .map(
                              (ip) =>
                                `${ip.ip} · ${new Date(ip.lastSeen * 1000).toLocaleTimeString()}`,
                            )
                            .join("\n")
                        : "在线"}
                    </div>
                  </div>
                ))}
              </div>
            }
          >
            <Tag color="green" style={{ cursor: "pointer" }}>
              {count} 在线
            </Tag>
          </Popover>
        );
      },
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
      width: 220,
      render: (_, record) => (
        <Space size="small">
          <Button size="small" type="link" icon={<EditOutlined />} onClick={() => openEdit(record)}>
            编辑
          </Button>
          <Button size="small" type="link" icon={<LinkOutlined />} onClick={() => openLinks(record)}>
            链接
          </Button>
          <Popconfirm title="删除该入站及其客户端？" onConfirm={() => remove(record)}>
            <Button size="small" type="link" danger icon={<DeleteOutlined />}>
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  const clientLabel =
    protocol === "vless" || protocol === "vmess"
      ? "UUID"
      : protocol === "http" || protocol === "mixed"
        ? ""
        : "密码";

  return (
    <div>
      <div className="page-title" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <Typography.Title level={4} style={{ margin: 0 }}>
          入站管理
        </Typography.Title>
        <Button type="primary" icon={<PlusOutlined />} onClick={openCreate}>
          新建入站
        </Button>
      </div>

      <Card>
        <Table
          rowKey="id"
          loading={loading}
          columns={columns}
          dataSource={inbounds}
          pagination={false}
          scroll={{ x: 1250 }}
          locale={{ emptyText: <Empty description="还没有入站，点击右上角新建" /> }}
        />
      </Card>

      <Modal
        open={modalOpen}
        title={editing ? "编辑入站" : "新建入站"}
        width={980}
        onCancel={() => setModalOpen(false)}
        onOk={() => form.submit()}
        okText="保存并应用"
        cancelText="取消"
        confirmLoading={loading}
        destroyOnHidden
      >
        <Form
          form={form}
          layout="vertical"
          onFinish={submit}
          initialValues={defaultValues}
          requiredMark={false}
          preserve={false}
        >
          <Tabs
            items={[
              {
                key: "basic",
                label: "基础",
                forceRender: true,
                children: (
                  <Row gutter={16}>
                    <Col span={8}>
                      <Form.Item name="remark" label="备注">
                        <Input placeholder="例如：VLESS-443" />
                      </Form.Item>
                    </Col>
                    <Col span={8}>
                      <Form.Item name="protocol" label="协议" rules={[{ required: true }]}>
                        <Select options={protocols} onChange={(value) => onProtocolChange(value as Protocol)} />
                      </Form.Item>
                    </Col>
                    <Col span={8}>
                      <Form.Item name="port" label="端口" rules={[{ required: true }]}>
                        <InputNumber min={1} max={65535} style={{ width: "100%" }} />
                      </Form.Item>
                    </Col>
                    <Col span={8}>
                      <Form.Item name="listen" label="监听地址">
                        <Input placeholder="留空 = 0.0.0.0" />
                      </Form.Item>
                    </Col>
                    <Col span={8}>
                      <Form.Item name="tag" label="Tag">
                        <Input placeholder="留空自动生成" />
                      </Form.Item>
                    </Col>
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
                    <Col span={8}>
                      <Form.Item name="trafficReset" label="流量重置">
                        <Select options={resetOptions} />
                      </Form.Item>
                    </Col>
                    <Col span={8}>
                      <Form.Item shouldUpdate noStyle>
                        {() =>
                          form.getFieldValue("trafficReset") === "monthly" ? (
                            <Form.Item name="trafficResetDay" label="每月重置日">
                              <InputNumber min={1} max={31} style={{ width: "100%" }} />
                            </Form.Item>
                          ) : null
                        }
                      </Form.Item>
                    </Col>
                  </Row>
                ),
              },
              {
                key: "protocol",
                label: "协议设置",
                forceRender: true,
                children: (
                  <div>
                    {protocol === "shadowsocks" && (
                      <Row gutter={16}>
                        <Col span={8}>
                          <Form.Item name="method" label="加密方式">
                            <Select options={ssMethods} />
                          </Form.Item>
                        </Col>
                        <Col span={8}>
                          <Form.Item name="ssNetwork" label="网络">
                            <Select
                              options={[
                                { value: "tcp", label: "tcp" },
                                { value: "udp", label: "udp" },
                                { value: "tcp,udp", label: "tcp,udp" },
                              ]}
                            />
                          </Form.Item>
                        </Col>
                        <Col span={8}>
                          <Form.Item name="ivCheck" label="IV 检查" valuePropName="checked">
                            <Switch />
                          </Form.Item>
                        </Col>
                      </Row>
                    )}
                    <Alert
                      type="info"
                      showIcon
                      style={{ marginBottom: 12 }}
                      message={
                        protocol === "wireguard"
                          ? "WireGuard 使用 peer 公钥作为客户端；服务端 secretKey 留空会导致配置校验失败。"
                          : protocol === "vless" || protocol === "vmess"
                            ? "客户端 UUID 留空时自动生成；VMess 可单独设置 alterId/security。"
                            : protocol === "shadowsocks"
                              ? "2022 系列密码必须是合法 base64；传统加密仅第一个客户端生效。"
                              : "客户端密码/账号留空时自动生成。"
                      }
                    />
                    {protocol === "wireguard" && (
                      <>
                        <Row gutter={16}>
                          <Col span={10}>
                            <Form.Item name="wgSecretKey" label="secretKey（服务端私钥）" rules={[{ required: true }]}>
                              <Input
                                addonAfter={
                                  <Button size="small" type="link" onClick={generateWireguardKey}>
                                    生成密钥对
                                  </Button>
                                }
                              />
                            </Form.Item>
                          </Col>
                          <Col span={7}>
                            <Form.Item name="wgPublicKey" label="publicKey（服务端公钥）">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={7}>
                            <Form.Item name="wgMtu" label="MTU">
                              <InputNumber min={1} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="wgDns" label="DNS">
                              <Input placeholder="1.1.1.1, 1.0.0.1" />
                            </Form.Item>
                          </Col>
                          <Col span={5}>
                            <Form.Item name="wgWorkers" label="workers">
                              <InputNumber min={1} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={5}>
                            <Form.Item name="wgSubnetIp" label="subnetIp">
                              <Input placeholder="10.0.0.0" />
                            </Form.Item>
                          </Col>
                          <Col span={3}>
                            <Form.Item name="wgSubnetCidr" label="CIDR">
                              <InputNumber min={1} max={32} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={3}>
                            <Form.Item name="wgNoKernelTun" label="noKernelTun" valuePropName="checked">
                              <Switch size="small" />
                            </Form.Item>
                          </Col>
                        </Row>
                        <Form.List name="wgPeers">
                          {(fields, { add, remove: removePeer }) => (
                            <div>
                              {fields.map((field) => (
                                <Card key={field.key} size="small" style={{ marginBottom: 8 }}>
                                  <Row gutter={12} align="middle">
                                    <Col span={9}>
                                      <Form.Item name={[field.name, "publicKey"]} label="publicKey" rules={[{ required: true }]} style={{ marginBottom: 0 }}>
                                        <Input
                                          addonAfter={
                                            <Button size="small" type="link" onClick={() => generatePeerKey(field.name)}>
                                              生成
                                            </Button>
                                          }
                                        />
                                      </Form.Item>
                                    </Col>
                                    <Col span={7}>
                                      <Form.Item name={[field.name, "privateKey"]} label="privateKey（面板保存）" style={{ marginBottom: 0 }}>
                                        <Input />
                                      </Form.Item>
                                    </Col>
                                    <Col span={4}>
                                      <Form.Item name={[field.name, "allowedIPs"]} label="allowedIPs" style={{ marginBottom: 0 }}>
                                        <Input placeholder="10.0.0.2/32" />
                                      </Form.Item>
                                    </Col>
                                    <Col span={2}>
                                      <Form.Item name={[field.name, "keepAlive"]} label="keepAlive" style={{ marginBottom: 0 }}>
                                        <InputNumber min={0} style={{ width: "100%" }} />
                                      </Form.Item>
                                    </Col>
                                    <Col span={2}>
                                      <Button danger type="text" icon={<DeleteOutlined />} onClick={() => removePeer(field.name)} />
                                    </Col>
                                  </Row>
                                  <Form.Item name={[field.name, "comment"]} label="备注" style={{ marginBottom: 0, marginTop: 8 }}>
                                    <Input />
                                  </Form.Item>
                                </Card>
                              ))}
                              <Button type="dashed" block icon={<PlusOutlined />} onClick={() => add({ keepAlive: 0 })}>
                                添加 peer
                              </Button>
                            </div>
                          )}
                        </Form.List>
                      </>
                    )}
                    {protocol !== "wireguard" && (<Form.List name="clients">
                      {(fields, { add, remove: removeClient }) => (
                        <div>
                          <Space direction="vertical" style={{ width: "100%" }} size="middle">
                            {fields.map((field) => (
                              <Card key={field.key} size="small">
                                <Row gutter={12} align="middle">
                                  <Col span={6}>
                                    <Form.Item
                                      name={[field.name, "email"]}
                                      label="邮箱 / 标识"
                                      rules={[{ required: true, message: "请输入邮箱或标识" }]}
                                      style={{ marginBottom: 0 }}
                                    >
                                      <Input placeholder="user@example.com" />
                                    </Form.Item>
                                  </Col>
                                  {protocol === "http" || protocol === "mixed" ? (
                                    <>
                                      <Col span={5}>
                                        <Form.Item name={[field.name, "user"]} label="用户名" style={{ marginBottom: 0 }}>
                                          <Input />
                                        </Form.Item>
                                      </Col>
                                      <Col span={6}>
                                        <Form.Item name={[field.name, "pass"]} label="密码" style={{ marginBottom: 0 }}>
                                          <Input />
                                        </Form.Item>
                                      </Col>
                                    </>
                                  ) : (
                                    <>
                                      <Col span={7}>
                                        <Form.Item
                                          name={[field.name, "credential"]}
                                          label={clientLabel}
                                          style={{ marginBottom: 0 }}
                                        >
                                          <Input
                                            addonAfter={
                                              <Button size="small" type="link" onClick={() => regenerate(field.name)}>
                                                生成
                                              </Button>
                                            }
                                          />
                                        </Form.Item>
                                      </Col>
                                      {protocol === "vless" && (
                                        <Col span={4}>
                                          <Form.Item name={[field.name, "flow"]} label="Flow" style={{ marginBottom: 0 }}>
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
                                      {protocol === "vmess" && (
                                        <>
                                          <Col span={3}>
                                            <Form.Item name={[field.name, "alterId"]} label="alterId" style={{ marginBottom: 0 }}>
                                              <InputNumber min={0} style={{ width: "100%" }} />
                                            </Form.Item>
                                          </Col>
                                          <Col span={4}>
                                            <Form.Item name={[field.name, "security"]} label="security" style={{ marginBottom: 0 }}>
                                              <Select
                                                options={["auto", "aes-128-gcm", "chacha20-poly1305", "none", "zero"].map(
                                                  (value) => ({ value, label: value }),
                                                )}
                                              />
                                            </Form.Item>
                                          </Col>
                                        </>
                                      )}
                                    </>
                                  )}
                                  {protocol !== "http" && protocol !== "mixed" && (
                                    <Col span={3}>
                                      <Form.Item
                                        name={[field.name, "enable"]}
                                        label="启用"
                                        valuePropName="checked"
                                        style={{ marginBottom: 0 }}
                                      >
                                        <Switch size="small" />
                                      </Form.Item>
                                    </Col>
                                  )}
                                  <Col span={2} style={{ textAlign: "right" }}>
                                    <Button danger type="text" icon={<DeleteOutlined />} onClick={() => removeClient(field.name)} />
                                  </Col>
                                </Row>
                                <Row gutter={12} style={{ marginTop: 10 }}>
                                  <Col span={4}>
                                    <Form.Item name={[field.name, "totalGb"]} label="限额 GB" style={{ marginBottom: 0 }}>
                                      <InputNumber min={0} style={{ width: "100%" }} />
                                    </Form.Item>
                                  </Col>
                                  <Col span={5}>
                                    <Form.Item name={[field.name, "expiry"]} label="到期" style={{ marginBottom: 0 }}>
                                      <DatePicker showTime style={{ width: "100%" }} format="YYYY-MM-DD HH:mm" />
                                    </Form.Item>
                                  </Col>
                                  <Col span={3}>
                                    <Form.Item name={[field.name, "limitIp"]} label="IP 限制" style={{ marginBottom: 0 }}>
                                      <InputNumber min={0} style={{ width: "100%" }} />
                                    </Form.Item>
                                  </Col>
                                  <Col span={3}>
                                    <Form.Item name={[field.name, "limitHwid"]} label="HWID" style={{ marginBottom: 0 }}>
                                      <InputNumber min={0} style={{ width: "100%" }} />
                                    </Form.Item>
                                  </Col>
                                  <Col span={3}>
                                    <Form.Item name={[field.name, "resetDays"]} label="重置天" style={{ marginBottom: 0 }}>
                                      <InputNumber min={0} style={{ width: "100%" }} />
                                    </Form.Item>
                                  </Col>
                                  <Col span={6}>
                                    <Form.Item name={[field.name, "comment"]} label="备注" style={{ marginBottom: 0 }}>
                                      <Input />
                                    </Form.Item>
                                  </Col>
                                </Row>
                              </Card>
                            ))}
                          </Space>
                          <Button type="dashed" block icon={<PlusOutlined />} style={{ marginTop: 12 }} onClick={addClient}>
                            添加客户端（自动生成 ID/密码）
                          </Button>
                        </div>
                      )}
                    </Form.List>)}

                    {(protocol === "vless" || protocol === "trojan") && (
                      <div style={{ marginTop: 20 }}>
                        <Divider>Fallbacks</Divider>
                        <Form.List name="fallbacks">
                          {(fields, { add, remove: removeFallback }) => (
                            <div>
                              {fields.map((field) => (
                                <Row key={field.key} gutter={12} align="middle" style={{ marginBottom: 8 }}>
                                  <Col span={4}>
                                    <Form.Item name={[field.name, "name"]} label="name" style={{ marginBottom: 0 }}>
                                      <Input />
                                    </Form.Item>
                                  </Col>
                                  <Col span={4}>
                                    <Form.Item name={[field.name, "alpn"]} label="alpn" style={{ marginBottom: 0 }}>
                                      <Input />
                                    </Form.Item>
                                  </Col>
                                  <Col span={4}>
                                    <Form.Item name={[field.name, "path"]} label="path" style={{ marginBottom: 0 }}>
                                      <Input />
                                    </Form.Item>
                                  </Col>
                                  <Col span={5}>
                                    <Form.Item name={[field.name, "dest"]} label="dest" style={{ marginBottom: 0 }}>
                                      <Input placeholder="端口或路径" />
                                    </Form.Item>
                                  </Col>
                                  <Col span={3}>
                                    <Form.Item name={[field.name, "xver"]} label="xver" style={{ marginBottom: 0 }}>
                                      <InputNumber min={0} style={{ width: "100%" }} />
                                    </Form.Item>
                                  </Col>
                                  <Col span={2} style={{ textAlign: "right" }}>
                                    <Button danger type="text" icon={<DeleteOutlined />} onClick={() => removeFallback(field.name)} />
                                  </Col>
                                </Row>
                              ))}
                              <Button type="dashed" block icon={<PlusOutlined />} onClick={() => add({ xver: 0 })}>
                                添加 fallback
                              </Button>
                            </div>
                          )}
                        </Form.List>
                      </div>
                    )}
                  </div>
                ),
              },
              {
                key: "transport",
                label: "传输与安全",
                forceRender: true,
                children: (
                  <div>
                    <Row gutter={16}>
                      <Col span={8}>
                        <Form.Item name="network" label="传输方式">
                          <Select
                            options={["tcp", "ws", "grpc", "httpupgrade", "xhttp", "kcp", "quic", "http"].map(
                              (value) => ({ value, label: value }),
                            )}
                          />
                        </Form.Item>
                      </Col>
                      <Col span={8}>
                        <Form.Item name="security" label="安全">
                          <Select
                            options={[
                              { value: "none", label: "无" },
                              { value: "tls", label: "TLS" },
                              { value: "reality", label: "REALITY" },
                            ]}
                          />
                        </Form.Item>
                      </Col>
                      <Col span={8}>
                        <Form.Item name="tlsFingerprint" label="Fingerprint">
                          <Select
                            options={["chrome", "firefox", "safari", "ios", "android", "edge", "random"].map(
                              (value) => ({ value, label: value }),
                            )}
                          />
                        </Form.Item>
                      </Col>

                      {network === "tcp" && (
                        <>
                          <Col span={8}>
                            <Form.Item name="tcpHeaderType" label="TCP header 类型">
                              <Select
                                options={[
                                  { value: "none", label: "none" },
                                  { value: "http", label: "http" },
                                ]}
                              />
                            </Form.Item>
                          </Col>
                          {form.getFieldValue("tcpHeaderType") === "http" && (
                            <>
                              <Col span={8}>
                                <Form.Item name="tcpHost" label="HTTP Host">
                                  <Input />
                                </Form.Item>
                              </Col>
                              <Col span={8}>
                                <Form.Item name="tcpPath" label="HTTP Path">
                                  <Input />
                                </Form.Item>
                              </Col>
                            </>
                          )}
                        </>
                      )}

                      {network === "ws" && (
                        <>
                          <Col span={8}>
                            <Form.Item name="wsPath" label="WebSocket 路径">
                              <Input placeholder="/ws" />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="wsHost" label="WebSocket Host">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="wsHeartbeat" label="heartbeatPeriod">
                              <InputNumber min={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                        </>
                      )}

                      {network === "grpc" && (
                        <>
                          <Col span={8}>
                            <Form.Item name="grpcService" label="ServiceName">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="grpcAuthority" label="Authority">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="grpcMultiMode" label="multiMode" valuePropName="checked">
                              <Switch />
                            </Form.Item>
                          </Col>
                        </>
                      )}

                      {network === "httpupgrade" && (
                        <>
                          <Col span={12}>
                            <Form.Item name="httpupgradePath" label="路径">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={12}>
                            <Form.Item name="httpupgradeHost" label="Host">
                              <Input />
                            </Form.Item>
                          </Col>
                        </>
                      )}

                      {network === "xhttp" && (
                        <>
                          <Col span={8}>
                            <Form.Item name="xhttpPath" label="路径">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="xhttpHost" label="Host">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="xhttpMode" label="mode">
                              <Select
                                options={["auto", "packet-up", "stream-up", "stream-one"].map((value) => ({
                                  value,
                                  label: value,
                                }))}
                              />
                            </Form.Item>
                          </Col>
                        </>
                      )}

                      {network === "kcp" && (
                        <>
                          <Col span={6}>
                            <Form.Item name="kcpMtu" label="MTU">
                              <InputNumber min={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="kcpTti" label="TTI">
                              <InputNumber min={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="kcpUplink" label="上行容量">
                              <InputNumber min={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="kcpDownlink" label="下行容量">
                              <InputNumber min={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="kcpCongestion" label="拥塞控制" valuePropName="checked">
                              <Switch />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="kcpReadBuffer" label="读缓冲">
                              <InputNumber min={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="kcpWriteBuffer" label="写缓冲">
                              <InputNumber min={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="kcpHeaderType" label="header 类型">
                              <Select
                                options={["none", "srtp", "utp", "wechat-video", "dtls", "wireguard"].map(
                                  (value) => ({ value, label: value }),
                                )}
                              />
                            </Form.Item>
                          </Col>
                          <Col span={12}>
                            <Form.Item name="kcpSeed" label="seed">
                              <Input />
                            </Form.Item>
                          </Col>
                        </>
                      )}

                      {network === "quic" && (
                        <>
                          <Col span={12}>
                            <Form.Item name="quicKey" label="QUIC key">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={12}>
                            <Form.Item name="quicHeaderType" label="header 类型">
                              <Select
                                options={["none", "srtp", "utp", "wechat-video", "dtls", "wireguard"].map(
                                  (value) => ({ value, label: value }),
                                )}
                              />
                            </Form.Item>
                          </Col>
                        </>
                      )}

                      {network === "http" && (
                        <>
                          <Col span={12}>
                            <Form.Item name="httpPath" label="HTTP/2 路径">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={12}>
                            <Form.Item name="httpHost" label="HTTP/2 Host">
                              <Input />
                            </Form.Item>
                          </Col>
                        </>
                      )}
                    </Row>

                    {security === "tls" && (
                      <>
                        <Divider>TLS</Divider>
                        <Row gutter={16}>
                          <Col span={8}>
                            <Form.Item name="tlsServerName" label="serverName / SNI">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="tlsAlpn" label="ALPN">
                              <Select mode="tags" options={["h2", "http/1.1", "h3"].map((value) => ({ value, label: value }))} />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="tlsAllowInsecure" label="allowInsecure" valuePropName="checked">
                              <Switch />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="tlsCertFile" label="证书文件路径">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="tlsKeyFile" label="私钥文件路径">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="tlsCertContent" label="证书内容">
                              <Input.TextArea rows={3} />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="tlsKeyContent" label="私钥内容">
                              <Input.TextArea rows={3} />
                            </Form.Item>
                          </Col>
                        </Row>
                      </>
                    )}

                    {security === "reality" && (
                      <>
                        <Divider>REALITY</Divider>
                        <Row gutter={16}>
                          <Col span={8}>
                            <Form.Item name="realityTarget" label="target">
                              <Input placeholder="example.com:443" />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="realityServerNames" label="serverNames">
                              <Select mode="tags" />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="realityShortIds" label="shortIds">
                              <Select mode="tags" />
                            </Form.Item>
                          </Col>
                          <Col span={12}>
                            <Form.Item name="realityPrivateKey" label="privateKey">
                              <Input
                                addonAfter={
                                  <Button size="small" type="link" onClick={generateRealityKey}>
                                    生成密钥对
                                  </Button>
                                }
                              />
                            </Form.Item>
                          </Col>
                          <Col span={12}>
                            <Form.Item name="realityPublicKey" label="publicKey">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="tlsServerName" label="SNI / serverName">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="realitySpiderX" label="spiderX">
                              <Input />
                            </Form.Item>
                          </Col>
                        </Row>
                      </>
                    )}

                    <Divider>Sockopt</Divider>
                    <Row gutter={16}>
                      <Col span={4}>
                        <Form.Item name="sockoptTcpFastOpen" label="tcpFastOpen" valuePropName="checked">
                          <Switch size="small" />
                        </Form.Item>
                      </Col>
                      <Col span={4}>
                        <Form.Item name="sockoptTcpNoDelay" label="tcpNoDelay" valuePropName="checked">
                          <Switch size="small" />
                        </Form.Item>
                      </Col>
                      <Col span={4}>
                        <Form.Item name="sockoptTcpKeepAlive" label="tcpKeepAlive" valuePropName="checked">
                          <Switch size="small" />
                        </Form.Item>
                      </Col>
                      <Col span={4}>
                        <Form.Item name="sockoptTcpMptcp" label="tcpMptcp" valuePropName="checked">
                          <Switch size="small" />
                        </Form.Item>
                      </Col>
                      <Col span={4}>
                        <Form.Item name="sockoptAcceptProxyProtocol" label="acceptProxyProtocol" valuePropName="checked">
                          <Switch size="small" />
                        </Form.Item>
                      </Col>
                      <Col span={4}>
                        <Form.Item name="sockoptTcpKeepAliveIdle" label="keepAliveIdle">
                          <InputNumber min={0} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                      <Col span={4}>
                        <Form.Item name="sockoptTcpKeepAliveInterval" label="keepAliveInterval">
                          <InputNumber min={0} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                      <Col span={5}>
                        <Form.Item name="sockoptTcpCongestion" label="tcpCongestion">
                          <Input />
                        </Form.Item>
                      </Col>
                      <Col span={5}>
                        <Form.Item name="sockoptDomainStrategy" label="domainStrategy">
                          <Select
                            allowClear
                            options={["AsIs", "UseIP", "UseIPv4", "UseIPv6"].map((value) => ({
                              value,
                              label: value,
                            }))}
                          />
                        </Form.Item>
                      </Col>
                      <Col span={5}>
                        <Form.Item name="sockoptDialerProxy" label="dialerProxy">
                          <Input />
                        </Form.Item>
                      </Col>
                      <Col span={4}>
                        <Form.Item name="sockoptMark" label="mark">
                          <InputNumber min={0} style={{ width: "100%" }} />
                        </Form.Item>
                      </Col>
                      <Col span={6}>
                        <Form.Item name="sockoptInterface" label="interface">
                          <Input />
                        </Form.Item>
                      </Col>
                      <Col span={4}>
                        <Form.Item name="sockoptTproxy" label="tproxy">
                          <Select
                            allowClear
                            options={["off", "redirect", "tproxy"].map((value) => ({ value, label: value }))}
                          />
                        </Form.Item>
                      </Col>
                    </Row>
                  </div>
                ),
              },
              {
                key: "sniffing",
                label: "嗅探",
                forceRender: true,
                children: (
                  <Row gutter={16}>
                    <Col span={6}>
                      <Form.Item name="sniffEnabled" label="启用嗅探" valuePropName="checked">
                        <Switch />
                      </Form.Item>
                    </Col>
                    <Col span={6}>
                      <Form.Item name="sniffMetadataOnly" label="metadataOnly" valuePropName="checked">
                        <Switch />
                      </Form.Item>
                    </Col>
                    <Col span={6}>
                      <Form.Item name="sniffRouteOnly" label="routeOnly" valuePropName="checked">
                        <Switch />
                      </Form.Item>
                    </Col>
                    <Col span={12}>
                      <Form.Item name="sniffDest" label="覆盖目标">
                        <Select mode="multiple" options={["http", "tls", "quic"].map((value) => ({ value, label: value }))} />
                      </Form.Item>
                    </Col>
                    <Col span={12}>
                      <Form.Item name="sniffDomainsExcluded" label="排除域名">
                        <Select mode="tags" />
                      </Form.Item>
                    </Col>
                  </Row>
                ),
              },
            ]}
          />
        </Form>
      </Modal>

      <Modal
        open={linksOpen}
        title="客户端分享链接"
        width={760}
        onCancel={() => setLinksOpen(false)}
        footer={<Button onClick={() => setLinksOpen(false)}>关闭</Button>}
      >
        {links && links.items.length > 0 ? (
          <div>
            <Typography.Paragraph type="secondary">使用地址：{links.address}</Typography.Paragraph>
            {links.items.map((item) => (
              <div key={item.clientId} style={{ marginBottom: 12 }}>
                <Typography.Text strong>{item.email}</Typography.Text>
                {item.error ? (
                  <Typography.Paragraph type="danger" style={{ marginBottom: 0 }}>
                    {item.error}
                  </Typography.Paragraph>
                ) : (
                  <Typography.Paragraph copyable={{ text: item.link }} className="word-break" style={{ marginBottom: 0 }}>
                    <code>{item.link}</code>
                  </Typography.Paragraph>
                )}
              </div>
            ))}
          </div>
        ) : (
          <Empty description="该入站还没有可用客户端" />
        )}
      </Modal>
    </div>
  );
}
