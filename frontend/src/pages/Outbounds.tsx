import { useEffect, useMemo, useState } from "react";
import {
  Alert,
  App as AntApp,
  Button,
  Card,
  Col,
  Divider,
  Empty,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
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
  PlusOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import { get, post, put } from "../api";
import type { OutboundTraffic, XrayTemplateResponse } from "../types";
import { formatBytes } from "../util";

type Network = "tcp" | "ws" | "grpc" | "httpupgrade" | "xhttp" | "kcp" | "quic" | "http";
type Security = "none" | "tls" | "reality";

const protocols = [
  "freedom",
  "blackhole",
  "socks",
  "http",
  "shadowsocks",
  "vmess",
  "vless",
  "trojan",
  "wireguard",
  "dns",
].map((value) => ({ value, label: value }));

const serverProtocols = new Set(["socks", "http", "shadowsocks", "trojan"]);
const vnextProtocols = new Set(["vmess", "vless"]);
const streamProtocols = new Set(["vmess", "vless", "trojan", "shadowsocks"]);
const muxProtocols = new Set(["vmess", "vless", "trojan", "shadowsocks", "socks", "http"]);

const ssMethods = [
  "2022-blake3-aes-128-gcm",
  "2022-blake3-aes-256-gcm",
  "chacha20-ietf-poly1305",
  "aes-256-gcm",
  "aes-128-gcm",
].map((value) => ({ value, label: value }));

function randomAlnum(length: number): string {
  const alphabet = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
  let out = "";
  for (let i = 0; i < length; i += 1) {
    out += alphabet[Math.floor(Math.random() * alphabet.length)];
  }
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

interface ServerRow {
  address?: string;
  port?: number;
  method?: string;
  password?: string;
  username?: string;
  uot?: boolean;
}

interface UserRow {
  id?: string;
  flow?: string;
  alterId?: number;
  security?: string;
}

interface VnextRow {
  address?: string;
  port?: number;
  users?: UserRow[];
}

interface PeerRow {
  publicKey?: string;
  endpoint?: string;
  allowedIPs?: string;
  keepAlive?: number;
}

interface OutboundFormValues {
  tag: string;
  protocol: string;
  sendThrough?: string;
  servers?: ServerRow[];
  vnext?: VnextRow[];
  peers?: PeerRow[];
  freedomDomainStrategy?: string;
  redirect?: string;
  blackholeResponse?: string;
  dnsNetwork?: string;
  dnsAddress?: string;
  dnsPort?: number;
  wgSecretKey?: string;
  wgAddress?: string;
  wgMtu?: number;
  wgReserved?: string;
  wgWorkers?: number;
  wgDomainStrategy?: string;
  wgNoise?: string;
  ssUot?: boolean;
  network: Network;
  security: Security;
  tlsServerName?: string;
  tlsAlpn?: string[];
  tlsFingerprint?: string;
  tlsAllowInsecure?: boolean;
  realityPublicKey?: string;
  realityShortId?: string;
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
  muxEnabled?: boolean;
  muxConcurrency?: number;
  muxXudpConcurrency?: number;
  muxXudpProxyUDP443?: string;
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
}

const defaults: OutboundFormValues = {
  tag: "proxy-out",
  protocol: "socks",
  sendThrough: "",
  servers: [{ address: "127.0.0.1", port: 1080, username: "", password: "" }],
  vnext: [{ address: "", port: 443, users: [{ id: "", flow: "" }] }],
  peers: [{ publicKey: "", endpoint: "", allowedIPs: "", keepAlive: 0 }],
  freedomDomainStrategy: "AsIs",
  redirect: "",
  blackholeResponse: "none",
  dnsNetwork: "udp",
  dnsAddress: "1.1.1.1",
  dnsPort: 53,
  wgSecretKey: "",
  wgAddress: "10.0.0.2/32",
  wgMtu: 1420,
  wgReserved: "",
  wgWorkers: 2,
  wgDomainStrategy: "ForceIP",
  wgNoise: "",
  ssUot: false,
  network: "tcp",
  security: "none",
  tlsAlpn: ["h2", "http/1.1"],
  tlsFingerprint: "chrome",
  tlsAllowInsecure: false,
  realityShortId: "",
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
  muxEnabled: false,
  muxConcurrency: 8,
  muxXudpConcurrency: 8,
  muxXudpProxyUDP443: "reject",
  sockoptTcpFastOpen: false,
  sockoptTcpNoDelay: false,
  sockoptTcpKeepAlive: false,
  sockoptTcpKeepAliveIdle: 0,
  sockoptTcpKeepAliveInterval: 0,
  sockoptTcpMptcp: false,
  sockoptAcceptProxyProtocol: false,
};

function splitList(value: string | undefined): string[] {
  return (value ?? "")
    .split(/[,\n]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function buildStreamSettings(values: OutboundFormValues): Record<string, unknown> {
  const stream: Record<string, unknown> = { network: values.network };
  if (values.network === "tcp" && values.tcpHeaderType === "http") {
    stream.tcpSettings = {
      header: {
        type: "http",
        request: {
          path: [values.tcpPath || "/"],
          headers: { Host: [values.tcpHost || ""] },
        },
      },
    };
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
      settings: {
        fingerprint: values.tlsFingerprint || "chrome",
        ...(values.tlsAllowInsecure ? { allowInsecure: true } : {}),
      },
    };
  } else if (values.security === "reality") {
    stream.security = "reality";
    stream.realitySettings = {
      ...(values.tlsServerName ? { serverName: values.tlsServerName } : {}),
      ...(values.tlsFingerprint ? { fingerprint: values.tlsFingerprint } : {}),
      ...(values.realityPublicKey ? { publicKey: values.realityPublicKey } : {}),
      ...(values.realityShortId ? { shortId: values.realityShortId } : {}),
      ...(values.realitySpiderX ? { spiderX: values.realitySpiderX } : {}),
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

function buildOutbound(values: OutboundFormValues): Record<string, unknown> {
  const tag = values.tag?.trim();
  if (!tag) throw new Error("Tag 不能为空");
  const protocol = values.protocol;
  const base: Record<string, unknown> = {
    tag,
    protocol,
    ...(values.sendThrough?.trim() ? { sendThrough: values.sendThrough.trim() } : {}),
  };

  let settings: Record<string, unknown>;
  if (protocol === "freedom") {
    settings = {
      domainStrategy: values.freedomDomainStrategy || "AsIs",
      ...(values.redirect?.trim() ? { redirect: values.redirect.trim() } : {}),
    };
  } else if (protocol === "blackhole") {
    settings = { response: { type: values.blackholeResponse || "none" } };
  } else if (protocol === "dns") {
    settings = {
      network: values.dnsNetwork || "udp",
      address: values.dnsAddress?.trim() || "1.1.1.1",
      port: values.dnsPort ?? 53,
    };
  } else if (protocol === "wireguard") {
    const peers = (values.peers ?? []).filter((peer) => peer.publicKey?.trim());
    if (!peers.length) throw new Error("WireGuard 至少需要一个 peer publicKey");
    settings = {
      secretKey: values.wgSecretKey?.trim() || "",
      address: splitList(values.wgAddress),
      mtu: values.wgMtu ?? 1420,
      ...(values.wgReserved?.trim()
        ? { reserved: splitList(values.wgReserved).map((item) => Number(item) || 0) }
        : {}),
      workers: values.wgWorkers ?? 2,
      domainStrategy: values.wgDomainStrategy || "ForceIP",
      ...(values.wgNoise?.trim() ? { noise: values.wgNoise.trim() } : {}),
      peers: peers.map((peer) => ({
        publicKey: peer.publicKey?.trim(),
        endpoint: peer.endpoint?.trim() || "",
        allowedIPs: splitList(peer.allowedIPs).length ? splitList(peer.allowedIPs) : ["0.0.0.0/0", "::/0"],
        ...(peer.keepAlive ? { keepAlive: peer.keepAlive } : {}),
      })),
    };
  } else if (vnextProtocols.has(protocol)) {
    const vnext = (values.vnext ?? []).filter((server) => server.address?.trim());
    if (vnext.length === 0) throw new Error("至少需要一个服务器");
    settings = {
      vnext: vnext.map((server) => ({
        address: server.address?.trim(),
        port: server.port ?? 443,
        users: (server.users ?? [])
          .filter((user) => user.id?.trim())
          .map((user) =>
            protocol === "vmess"
              ? {
                  id: user.id?.trim(),
                  alterId: user.alterId ?? 0,
                  security: user.security || "auto",
                }
              : {
                  id: user.id?.trim(),
                  encryption: "none",
                  ...(user.flow ? { flow: user.flow } : {}),
                },
          ),
      })),
    };
  } else if (protocol === "socks" || protocol === "http") {
    const servers = (values.servers ?? []).filter((server) => server.address?.trim());
    if (servers.length === 0) throw new Error("至少需要一个服务器");
    settings = {
      servers: servers.map((server) => ({
        address: server.address?.trim(),
        port: server.port ?? 1080,
        ...(server.username?.trim()
          ? { users: [{ user: server.username.trim(), pass: server.password ?? "" }] }
          : {}),
      })),
    };
  } else if (protocol === "shadowsocks") {
    const servers = (values.servers ?? []).filter((server) => server.address?.trim());
    if (servers.length === 0) throw new Error("至少需要一个服务器");
    settings = {
      servers: servers.map((server) => ({
        address: server.address?.trim(),
        port: server.port ?? 8388,
        method: server.method || "2022-blake3-aes-128-gcm",
        password: server.password || randomBase64(16),
        uot: Boolean(server.uot),
      })),
    };
  } else if (protocol === "trojan") {
    const servers = (values.servers ?? []).filter((server) => server.address?.trim());
    if (servers.length === 0) throw new Error("至少需要一个服务器");
    settings = {
      servers: servers.map((server) => ({
        address: server.address?.trim(),
        port: server.port ?? 443,
        password: server.password || randomAlnum(20),
      })),
    };
  } else {
    throw new Error(`不支持的协议: ${protocol}`);
  }

  const result: Record<string, unknown> = { ...base, settings };
  if (streamProtocols.has(protocol)) {
    result.streamSettings = buildStreamSettings(values);
  }
  if (muxProtocols.has(protocol) && values.muxEnabled) {
    result.mux = {
      enabled: true,
      concurrency: values.muxConcurrency ?? 8,
      ...(protocol !== "socks" && protocol !== "http"
        ? {
            xudpConcurrency: values.muxXudpConcurrency ?? 8,
            xudpProxyUDP443: values.muxXudpProxyUDP443 || "reject",
          }
        : {}),
    };
  }
  return result;
}

function inferForm(outbound: Record<string, unknown>): Partial<OutboundFormValues> {
  const settings = (outbound.settings ?? {}) as Record<string, any>;
  const stream = (outbound.streamSettings ?? {}) as Record<string, any>;
  const tls = (stream.tlsSettings ?? {}) as Record<string, any>;
  const reality = (stream.realitySettings ?? {}) as Record<string, any>;
  const protocol = String(outbound.protocol ?? "socks");
  const mux = (outbound.mux ?? {}) as Record<string, any>;
  const sockopt = (stream.sockopt ?? {}) as Record<string, any>;

  const servers: ServerRow[] = (settings.servers ?? []).map((server: any) => ({
    address: String(server.address ?? ""),
    port: Number(server.port ?? 0) || undefined,
    method: String(server.method ?? "2022-blake3-aes-128-gcm"),
    password: String(server.password ?? ""),
    username: String(server.users?.[0]?.user ?? ""),
    uot: Boolean(server.uot),
  }));
  const vnext: VnextRow[] = (settings.vnext ?? []).map((server: any) => ({
    address: String(server.address ?? ""),
    port: Number(server.port ?? 0) || undefined,
    users: (server.users ?? []).map((user: any) => ({
      id: String(user.id ?? ""),
      flow: String(user.flow ?? ""),
      alterId: Number(user.alterId ?? 0),
      security: String(user.security ?? "auto"),
    })),
  }));
  const peers: PeerRow[] = (settings.peers ?? []).map((peer: any) => ({
    publicKey: String(peer.publicKey ?? ""),
    endpoint: String(peer.endpoint ?? ""),
    allowedIPs: Array.isArray(peer.allowedIPs) ? peer.allowedIPs.join(",") : "",
    keepAlive: Number(peer.keepAlive ?? 0),
  }));

  return {
    ...defaults,
    tag: String(outbound.tag ?? ""),
    protocol,
    sendThrough: String(outbound.sendThrough ?? ""),
    servers: servers.length > 0 ? servers : defaults.servers,
    vnext: vnext.length > 0 ? vnext : defaults.vnext,
    peers: peers.length > 0 ? peers : defaults.peers,
    freedomDomainStrategy: String(settings.domainStrategy ?? "AsIs"),
    redirect: String(settings.redirect ?? ""),
    blackholeResponse: String(settings.response?.type ?? "none"),
    dnsNetwork: String(settings.network ?? "udp"),
    dnsAddress: String(settings.address ?? "1.1.1.1"),
    dnsPort: Number(settings.port ?? 53),
    wgSecretKey: String(settings.secretKey ?? ""),
    wgAddress: Array.isArray(settings.address) ? settings.address.join(",") : "10.0.0.2/32",
    wgMtu: Number(settings.mtu ?? 1420),
    wgReserved: Array.isArray(settings.reserved) ? settings.reserved.join(",") : "",
    wgWorkers: Number(settings.workers ?? 2),
    wgDomainStrategy: String(settings.domainStrategy ?? "ForceIP"),
    wgNoise: String(settings.noise ?? ""),
    ssUot: Boolean(settings.servers?.[0]?.uot),
    network: (stream.network as Network) ?? "tcp",
    security: (stream.security as Security) ?? "none",
    tlsServerName: String(tls.serverName ?? reality.serverName ?? ""),
    tlsAlpn: Array.isArray(tls.alpn) ? tls.alpn : ["h2", "http/1.1"],
    tlsFingerprint: String(tls.settings?.fingerprint ?? reality.fingerprint ?? "chrome"),
    tlsAllowInsecure: Boolean(tls.allowInsecure || tls.settings?.allowInsecure),
    realityPublicKey: String(reality.publicKey ?? ""),
    realityShortId: String(reality.shortId ?? ""),
    realitySpiderX: String(reality.spiderX ?? "/"),
    tcpHeaderType: String(stream.tcpSettings?.header?.type ?? "none"),
    tcpHost: String(stream.tcpSettings?.header?.request?.headers?.Host?.[0] ?? ""),
    tcpPath: String(stream.tcpSettings?.header?.request?.path?.[0] ?? ""),
    wsPath: String(stream.wsSettings?.path ?? "/"),
    wsHost: String(stream.wsSettings?.headers?.Host ?? ""),
    wsHeartbeat: Number(stream.wsSettings?.heartbeatPeriod ?? 0),
    grpcService: String(stream.grpcSettings?.serviceName ?? ""),
    grpcAuthority: String(stream.grpcSettings?.authority ?? ""),
    grpcMultiMode: Boolean(stream.grpcSettings?.multiMode),
    httpupgradePath: String(stream.httpupgradeSettings?.path ?? "/"),
    httpupgradeHost: String(stream.httpupgradeSettings?.host ?? ""),
    xhttpPath: String(stream.xhttpSettings?.path ?? "/"),
    xhttpHost: String(stream.xhttpSettings?.host ?? ""),
    xhttpMode: String(stream.xhttpSettings?.mode ?? "auto"),
    kcpMtu: Number(stream.kcpSettings?.mtu ?? 1350),
    kcpTti: Number(stream.kcpSettings?.tti ?? 50),
    kcpUplink: Number(stream.kcpSettings?.uplinkCapacity ?? 5),
    kcpDownlink: Number(stream.kcpSettings?.downlinkCapacity ?? 20),
    kcpCongestion: Boolean(stream.kcpSettings?.congestion),
    kcpReadBuffer: Number(stream.kcpSettings?.readBufferSize ?? 2),
    kcpWriteBuffer: Number(stream.kcpSettings?.writeBufferSize ?? 2),
    kcpHeaderType: String(stream.kcpSettings?.header?.type ?? "none"),
    kcpSeed: String(stream.kcpSettings?.seed ?? ""),
    quicKey: String(stream.quicSettings?.key ?? ""),
    quicHeaderType: String(stream.quicSettings?.header?.type ?? "none"),
    httpPath: String(stream.httpSettings?.path ?? "/"),
    httpHost: String(stream.httpSettings?.host?.[0] ?? ""),
    muxEnabled: Boolean(mux.enabled),
    muxConcurrency: Number(mux.concurrency ?? 8),
    muxXudpConcurrency: Number(mux.xudpConcurrency ?? 8),
    muxXudpProxyUDP443: String(mux.xudpProxyUDP443 ?? "reject"),
    sockoptTcpFastOpen: Boolean(sockopt.tcpFastOpen),
    sockoptTcpNoDelay: Boolean(sockopt.tcpNoDelay),
    sockoptTcpKeepAlive: Boolean(sockopt.tcpKeepAlive),
    sockoptTcpKeepAliveIdle: Number(sockopt.tcpKeepAliveIdle ?? 0),
    sockoptTcpKeepAliveInterval: Number(sockopt.tcpKeepAliveInterval ?? 0),
    sockoptTcpCongestion: String(sockopt.tcpCongestion ?? ""),
    sockoptTcpMptcp: Boolean(sockopt.tcpMptcp),
    sockoptAcceptProxyProtocol: Boolean(sockopt.acceptProxyProtocol),
    sockoptDomainStrategy: String(sockopt.domainStrategy ?? ""),
    sockoptDialerProxy: String(sockopt.dialerProxy ?? ""),
    sockoptMark: Number(sockopt.mark ?? 0),
    sockoptInterface: String(sockopt.interface ?? ""),
    sockoptTproxy: String(sockopt.tproxy ?? ""),
  };
}

export default function OutboundsPage() {
  const { message, modal } = AntApp.useApp();
  const [form] = Form.useForm<OutboundFormValues>();
  const [outbounds, setOutbounds] = useState<Record<string, unknown>[]>([]);
  const [traffic, setTraffic] = useState<Record<string, OutboundTraffic>>({});
  const [loading, setLoading] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);

  const protocol = Form.useWatch("protocol", form) ?? "socks";
  const network = Form.useWatch("network", form);
  const security = Form.useWatch("security", form);

  const load = async () => {
    setLoading(true);
    try {
      const data = await get<XrayTemplateResponse>("/api/xray/template");
      const list = (data.template.outbounds as Record<string, unknown>[]) ?? [];
      setOutbounds(list);
      setTraffic(
        Object.fromEntries((data.traffic ?? []).map((item) => [item.tag, item])),
      );
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
    setEditingIndex(null);
    form.setFieldsValue({ ...defaults });
    setModalOpen(true);
  };

  const openEdit = (index: number) => {
    const outbound = outbounds[index];
    setEditingIndex(index);
    form.setFieldsValue(inferForm(outbound));
    setModalOpen(true);
  };

  const submit = async (values: OutboundFormValues) => {
    try {
      const outbound = buildOutbound(values);
      const next = [...outbounds];
      if (editingIndex === null) {
        if (next.some((item) => item.tag === outbound.tag)) {
          throw new Error(`tag 已存在: ${String(outbound.tag)}`);
        }
        next.push(outbound);
      } else {
        next[editingIndex] = outbound;
      }
      await put("/api/xray/outbounds", { outbounds: next });
      message.success("出站已保存，Xray 已重启");
      setModalOpen(false);
      await load();
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const remove = async (index: number) => {
    const next = outbounds.filter((_, itemIndex) => itemIndex !== index);
    try {
      await put("/api/xray/outbounds", { outbounds: next });
      message.success("出站已删除");
      await load();
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const resetTraffic = async (tag: string) => {
    try {
      await post(`/api/xray/outbounds/${encodeURIComponent(tag)}/reset`);
      message.success("出站流量已重置");
      await load();
    } catch (error) {
      message.error((error as Error).message);
    }
  };

  const resetAll = () => {
    modal.confirm({
      title: "重置所有出站流量？",
      okText: "重置",
      cancelText: "取消",
      onOk: async () => {
        try {
          await post("/api/xray/outbounds/reset");
          message.success("出站流量已重置");
          await load();
        } catch (error) {
          message.error((error as Error).message);
        }
      },
    });
  };

  const outboundAddress = (outbound: Record<string, unknown>): string => {
    const settings = (outbound.settings ?? {}) as Record<string, any>;
    const server = settings.servers?.[0] ?? settings.vnext?.[0] ?? settings.peers?.[0];
    if (!server) return "";
    if (server.endpoint) return String(server.endpoint);
    return `${server.address ?? ""}:${server.port ?? ""}`;
  };

  const totals = useMemo(
    () => ({
      up: Object.values(traffic).reduce((sum, item) => sum + item.up, 0),
      down: Object.values(traffic).reduce((sum, item) => sum + item.down, 0),
    }),
    [traffic],
  );

  const columns: TableProps<Record<string, unknown>>["columns"] = [
    {
      title: "Tag",
      dataIndex: "tag",
      render: (value: string) => <Typography.Text strong>{value}</Typography.Text>,
    },
    {
      title: "协议",
      dataIndex: "protocol",
      width: 130,
      render: (value: string) => <Tag color="geekblue">{value}</Tag>,
    },
    {
      title: "地址",
      width: 200,
      render: (_, record) => outboundAddress(record) || "—",
    },
    {
      title: "流量",
      width: 210,
      render: (_, record) => {
        const item = traffic[String(record.tag)];
        return item ? (
          <span>
            ↑ {formatBytes(item.up)} / ↓ {formatBytes(item.down)}
          </span>
        ) : (
          <span className="stat-note">暂无统计</span>
        );
      },
    },
    {
      title: "操作",
      width: 250,
      render: (_, _record, index) => (
        <Space size="small">
          <Button size="small" type="link" icon={<EditOutlined />} onClick={() => openEdit(index)}>
            编辑
          </Button>
          <Button
            size="small"
            type="link"
            icon={<ReloadOutlined />}
            onClick={() => resetTraffic(String(outbounds[index].tag))}
          >
            重置流量
          </Button>
          <Popconfirm
            title="删除该出站？"
            onConfirm={() => remove(index)}
            disabled={outbounds.length <= 1}
          >
            <Button
              size="small"
              type="link"
              danger
              icon={<DeleteOutlined />}
              disabled={outbounds.length <= 1}
            >
              删除
            </Button>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  const randomSecret = () => randomBase64(32);

  return (
    <div>
      <div className="page-title" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <Typography.Title level={4} style={{ margin: 0 }}>
          出站管理
        </Typography.Title>
        <Space wrap>
          <Button icon={<ReloadOutlined />} onClick={load} loading={loading}>
            刷新
          </Button>
          <Button onClick={resetAll}>重置全部流量</Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={openCreate}>
            新增出站
          </Button>
        </Space>
      </div>

      <Alert
        type="info"
        showIcon
        style={{ marginBottom: 16 }}
        message={`共 ${outbounds.length} 个出站，累计 ↑ ${formatBytes(totals.up)} / ↓ ${formatBytes(totals.down)}。所有字段均可在表单中选择或添加。`}
      />

      <Card>
        <Table
          rowKey="tag"
          loading={loading}
          columns={columns}
          dataSource={outbounds}
          pagination={false}
          locale={{ emptyText: <Empty description="还没有出站" /> }}
        />
      </Card>

      <Modal
        open={modalOpen}
        title={editingIndex === null ? "新增出站" : "编辑出站"}
        width={1000}
        onCancel={() => setModalOpen(false)}
        onOk={() => form.submit()}
        okText="保存并应用"
        cancelText="取消"
        destroyOnHidden
      >
        <Form
          form={form}
          layout="vertical"
          onFinish={submit}
          initialValues={defaults}
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
                      <Form.Item name="tag" label="Tag" rules={[{ required: true }]}>
                        <Input placeholder="例如 proxy" />
                      </Form.Item>
                    </Col>
                    <Col span={8}>
                      <Form.Item name="protocol" label="协议" rules={[{ required: true }]}>
                        <Select options={protocols} />
                      </Form.Item>
                    </Col>
                    <Col span={8}>
                      <Form.Item name="sendThrough" label="sendThrough">
                        <Input placeholder="可选，本机出口 IP" />
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
                    {protocol === "freedom" && (
                      <Row gutter={16}>
                        <Col span={10}>
                          <Form.Item name="freedomDomainStrategy" label="domainStrategy">
                            <Select
                              options={["AsIs", "UseIP", "UseIPv4", "UseIPv6", "UseIPv4v6"].map(
                                (value) => ({ value, label: value }),
                              )}
                            />
                          </Form.Item>
                        </Col>
                        <Col span={14}>
                          <Form.Item name="redirect" label="redirect">
                            <Input placeholder="可选，例如 127.0.0.1:8080" />
                          </Form.Item>
                        </Col>
                      </Row>
                    )}

                    {protocol === "blackhole" && (
                      <Form.Item name="blackholeResponse" label="response.type">
                        <Select
                          options={[
                            { value: "none", label: "none" },
                            { value: "http", label: "http" },
                          ]}
                        />
                      </Form.Item>
                    )}

                    {protocol === "dns" && (
                      <Row gutter={16}>
                        <Col span={8}>
                          <Form.Item name="dnsNetwork" label="network">
                            <Select
                              options={["udp", "tcp"].map((value) => ({ value, label: value }))}
                            />
                          </Form.Item>
                        </Col>
                        <Col span={10}>
                          <Form.Item name="dnsAddress" label="DNS 地址">
                            <Input />
                          </Form.Item>
                        </Col>
                        <Col span={6}>
                          <Form.Item name="dnsPort" label="端口">
                            <InputNumber min={1} max={65535} style={{ width: "100%" }} />
                          </Form.Item>
                        </Col>
                      </Row>
                    )}

                    {protocol === "wireguard" && (
                      <div>
                        <Row gutter={16}>
                          <Col span={12}>
                            <Form.Item name="wgSecretKey" label="secretKey">
                              <Input
                                addonAfter={
                                  <Button size="small" type="link" onClick={() => form.setFieldValue("wgSecretKey", randomSecret())}>
                                    生成
                                  </Button>
                                }
                              />
                            </Form.Item>
                          </Col>
                          <Col span={12}>
                            <Form.Item name="wgAddress" label="address（逗号分隔）">
                              <Input placeholder="10.0.0.2/32" />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="wgMtu" label="mtu">
                              <InputNumber min={1} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="wgWorkers" label="workers">
                              <InputNumber min={1} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="wgReserved" label="reserved">
                              <Input placeholder="0,0,0" />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="wgDomainStrategy" label="domainStrategy">
                              <Select
                                options={["ForceIP", "ForceIPv4", "ForceIPv6", "ForceIPv4v6"].map(
                                  (value) => ({ value, label: value }),
                                )}
                              />
                            </Form.Item>
                          </Col>
                          <Col span={12}>
                            <Form.Item name="wgNoise" label="noise">
                              <Input placeholder="可选" />
                            </Form.Item>
                          </Col>
                        </Row>
                        <Form.List name="peers">
                          {(fields, { add, remove: removePeer }) => (
                            <div>
                              {fields.map((field) => (
                                <Space key={field.key} align="start" wrap style={{ marginBottom: 8 }}>
                                  <Form.Item
                                    name={[field.name, "publicKey"]}
                                    label="publicKey"
                                    rules={[{ required: true }]}
                                  >
                                    <Input style={{ width: 220 }} />
                                  </Form.Item>
                                  <Form.Item name={[field.name, "endpoint"]} label="endpoint">
                                    <Input style={{ width: 180 }} placeholder="host:port" />
                                  </Form.Item>
                                  <Form.Item name={[field.name, "allowedIPs"]} label="allowedIPs">
                                    <Input style={{ width: 180 }} placeholder="0.0.0.0/0,::/0" />
                                  </Form.Item>
                                  <Form.Item name={[field.name, "keepAlive"]} label="keepAlive">
                                    <InputNumber min={0} style={{ width: 100 }} />
                                  </Form.Item>
                                  <Button
                                    danger
                                    type="text"
                                    icon={<DeleteOutlined />}
                                    style={{ marginTop: 30 }}
                                    onClick={() => removePeer(field.name)}
                                  />
                                </Space>
                              ))}
                              <Button type="dashed" block icon={<PlusOutlined />} onClick={() => add({ allowedIPs: "0.0.0.0/0,::/0" })}>
                                添加 peer
                              </Button>
                            </div>
                          )}
                        </Form.List>
                      </div>
                    )}

                    {vnextProtocols.has(protocol) && (
                      <Form.List name="vnext">
                        {(fields, { add, remove: removeServer }) => (
                          <div>
                            {fields.map((field) => (
                              <Card
                                key={field.key}
                                size="small"
                                title="服务器"
                                extra={
                                  <Button
                                    danger
                                    size="small"
                                    type="text"
                                    icon={<DeleteOutlined />}
                                    onClick={() => removeServer(field.name)}
                                  />
                                }
                                style={{ marginBottom: 12 }}
                              >
                                <Row gutter={12}>
                                  <Col span={14}>
                                    <Form.Item name={[field.name, "address"]} label="地址" rules={[{ required: true }]}>
                                      <Input placeholder="example.com" />
                                    </Form.Item>
                                  </Col>
                                  <Col span={10}>
                                    <Form.Item name={[field.name, "port"]} label="端口" rules={[{ required: true }]}>
                                      <InputNumber min={1} max={65535} style={{ width: "100%" }} />
                                    </Form.Item>
                                  </Col>
                                </Row>
                                <Form.List name={[field.name, "users"]}>
                                  {(users, { add: addUser, remove: removeUser }) => (
                                    <div>
                                      {users.map((userField) => (
                                        <Row key={userField.key} gutter={12} align="middle">
                                          <Col span={9}>
                                            <Form.Item name={[userField.name, "id"]} label="UUID" rules={[{ required: true }]}>
                                              <Input
                                                addonAfter={
                                                  <Button
                                                    size="small"
                                                    type="link"
                                                    onClick={() =>
                                                      form.setFieldValue(
                                                        ["vnext", field.name, "users", userField.name, "id"],
                                                        uuid(),
                                                      )
                                                    }
                                                  >
                                                    生成
                                                  </Button>
                                                }
                                              />
                                            </Form.Item>
                                          </Col>
                                          {protocol === "vless" && (
                                            <Col span={6}>
                                              <Form.Item name={[userField.name, "flow"]} label="Flow">
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
                                              <Col span={4}>
                                                <Form.Item name={[userField.name, "alterId"]} label="alterId">
                                                  <InputNumber min={0} style={{ width: "100%" }} />
                                                </Form.Item>
                                              </Col>
                                              <Col span={5}>
                                                <Form.Item name={[userField.name, "security"]} label="security">
                                                  <Select
                                                    options={["auto", "aes-128-gcm", "chacha20-poly1305", "none", "zero"].map(
                                                      (value) => ({ value, label: value }),
                                                    )}
                                                  />
                                                </Form.Item>
                                              </Col>
                                            </>
                                          )}
                                          <Col span={2}>
                                            <Button danger type="text" icon={<DeleteOutlined />} onClick={() => removeUser(userField.name)} />
                                          </Col>
                                        </Row>
                                      ))}
                                      <Button
                                        type="dashed"
                                        block
                                        icon={<PlusOutlined />}
                                        onClick={() => addUser({ id: uuid(), flow: "", alterId: 0, security: "auto" })}
                                      >
                                        添加用户
                                      </Button>
                                    </div>
                                  )}
                                </Form.List>
                              </Card>
                            ))}
                            <Button
                              type="dashed"
                              block
                              icon={<PlusOutlined />}
                              onClick={() => add({ address: "", port: 443, users: [{ id: uuid(), flow: "" }] })}
                            >
                              添加服务器
                            </Button>
                          </div>
                        )}
                      </Form.List>
                    )}

                    {serverProtocols.has(protocol) && (
                      <Form.List name="servers">
                        {(fields, { add, remove: removeServer }) => (
                          <div>
                            {fields.map((field) => (
                              <Card
                                key={field.key}
                                size="small"
                                title="服务器"
                                extra={
                                  <Button
                                    danger
                                    size="small"
                                    type="text"
                                    icon={<DeleteOutlined />}
                                    onClick={() => removeServer(field.name)}
                                  />
                                }
                                style={{ marginBottom: 12 }}
                              >
                                <Row gutter={12}>
                                  <Col span={10}>
                                    <Form.Item name={[field.name, "address"]} label="地址" rules={[{ required: true }]}>
                                      <Input placeholder="example.com" />
                                    </Form.Item>
                                  </Col>
                                  <Col span={5}>
                                    <Form.Item name={[field.name, "port"]} label="端口" rules={[{ required: true }]}>
                                      <InputNumber min={1} max={65535} style={{ width: "100%" }} />
                                    </Form.Item>
                                  </Col>
                                  {protocol === "socks" || protocol === "http" ? (
                                    <>
                                      <Col span={4}>
                                        <Form.Item name={[field.name, "username"]} label="用户名">
                                          <Input />
                                        </Form.Item>
                                      </Col>
                                      <Col span={4}>
                                        <Form.Item name={[field.name, "password"]} label="密码">
                                          <Input.Password />
                                        </Form.Item>
                                      </Col>
                                    </>
                                  ) : protocol === "shadowsocks" ? (
                                    <>
                                      <Col span={4}>
                                        <Form.Item name={[field.name, "method"]} label="加密方式">
                                          <Select options={ssMethods} />
                                        </Form.Item>
                                      </Col>
                                      <Col span={4}>
                                        <Form.Item name={[field.name, "password"]} label="密码">
                                          <Input />
                                        </Form.Item>
                                      </Col>
                                      <Col span={3}>
                                        <Form.Item name={[field.name, "uot"]} label="UoT" valuePropName="checked">
                                          <Switch size="small" />
                                        </Form.Item>
                                      </Col>
                                    </>
                                  ) : (
                                    <Col span={7}>
                                      <Form.Item name={[field.name, "password"]} label="密码">
                                        <Input
                                          addonAfter={
                                            <Button
                                              size="small"
                                              type="link"
                                              onClick={() =>
                                                form.setFieldValue(
                                                  ["servers", field.name, "password"],
                                                  randomAlnum(20),
                                                )
                                              }
                                            >
                                              生成
                                            </Button>
                                          }
                                        />
                                      </Form.Item>
                                    </Col>
                                  )}
                                  <Col span={2}>
                                    <Button
                                      danger
                                      type="text"
                                      icon={<DeleteOutlined />}
                                      style={{ marginTop: 30 }}
                                      onClick={() => removeServer(field.name)}
                                    />
                                  </Col>
                                </Row>
                              </Card>
                            ))}
                            <Button
                              type="dashed"
                              block
                              icon={<PlusOutlined />}
                              onClick={() =>
                                add({
                                  address: "",
                                  port:
                                    protocol === "shadowsocks"
                                      ? 8388
                                      : protocol === "trojan"
                                        ? 443
                                        : 1080,
                                  password: "",
                                  method: "2022-blake3-aes-128-gcm",
                                  uot: false,
                                })
                              }
                            >
                              添加服务器
                            </Button>
                          </div>
                        )}
                      </Form.List>
                    )}
                  </div>
                ),
              },
              {
                key: "stream",
                label: "传输与安全",
                forceRender: true,
                children: streamProtocols.has(protocol) ? (
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
                              <Input />
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
                        </Row>
                      </>
                    )}

                    {security === "reality" && (
                      <>
                        <Divider>REALITY</Divider>
                        <Row gutter={16}>
                          <Col span={8}>
                            <Form.Item name="tlsServerName" label="serverName / SNI">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="realityPublicKey" label="publicKey">
                              <Input />
                            </Form.Item>
                          </Col>
                          <Col span={8}>
                            <Form.Item name="realityShortId" label="shortId">
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
                  </div>
                ) : (
                  <Alert type="info" showIcon message="该协议不支持传输/安全设置。" />
                ),
              },
              {
                key: "advanced",
                label: "Mux / Sockopt",
                forceRender: true,
                children: (
                  <div>
                    {muxProtocols.has(protocol) && (
                      <>
                        <Divider titlePlacement="start">Mux</Divider>
                        <Row gutter={16}>
                          <Col span={6}>
                            <Form.Item name="muxEnabled" label="启用 Mux" valuePropName="checked">
                              <Switch />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="muxConcurrency" label="concurrency">
                              <InputNumber min={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="muxXudpConcurrency" label="xudpConcurrency">
                              <InputNumber min={0} style={{ width: "100%" }} />
                            </Form.Item>
                          </Col>
                          <Col span={6}>
                            <Form.Item name="muxXudpProxyUDP443" label="xudpProxyUDP443">
                              <Select
                                options={["reject", "allow", "skip"].map((value) => ({ value, label: value }))}
                              />
                            </Form.Item>
                          </Col>
                        </Row>
                      </>
                    )}
                    <Divider titlePlacement="start">Sockopt</Divider>
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
            ]}
          />
        </Form>
      </Modal>
    </div>
  );
}
