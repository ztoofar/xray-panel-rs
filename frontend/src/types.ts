export interface OnlineIp {
  ip: string;
  lastSeen: number;
}

export interface OnlineClient {
  email: string;
  ips: OnlineIp[];
}

export interface Inbound {
  id: number;
  userId: number;
  up: number;
  down: number;
  total: number;
  remark: string;
  enable: boolean;
  expiryTime: number;
  trafficReset: string;
  trafficResetDay: number;
  lastTrafficResetTime: number;
  listen: string;
  port: number;
  protocol: string;
  settings: Record<string, unknown>;
  streamSettings: Record<string, unknown>;
  sniffing: Record<string, unknown>;
  tag: string;
  createdAt: number;
  updatedAt: number;
  clientCount?: number;
  onlineCount?: number;
  onlineClients?: OnlineClient[];
}

export interface Client {
  id: number;
  inboundId: number;
  inboundIds: number[];
  email: string;
  enable: boolean;
  up: number;
  down: number;
  total: number;
  expiryTime: number;
  settings: Record<string, unknown>;
  comment: string;
  groupName: string;
  limitIp: number;
  limitHwid: number;
  resetDays: number;
  lastResetTime: number;
  createdAt: number;
  updatedAt: number;
}

export interface XrayStatus {
  running: boolean;
  pid?: number | null;
  startedAt?: number | null;
  lastExitAt?: number | null;
  lastExitCode?: number | null;
  lastError?: string | null;
  restarts: number;
  version?: string | null;
}

export interface SystemStats {
  cpuPercent: number;
  memTotal: number;
  memUsed: number;
  memPercent: number;
  load1: number;
  load5: number;
  load15: number;
  uptime: number;
  hostname: string;
}

export interface TrafficPoint {
  ts: number;
  up: number;
  down: number;
}

export interface DashboardData {
  xray: XrayStatus;
  system: SystemStats;
  inbounds: number;
  clients: number;
  enabledClients: number;
  onlineClients: number;
  up: number;
  down: number;
  history: TrafficPoint[];
  startedAt: number;
  version: string;
}

export interface SettingsData {
  panelName: string;
  publicAddress: string;
  trafficPollSecs: number;
  xrayExtra: Record<string, unknown>;
  panelListen: string;
}

export interface LinkItem {
  clientId: number;
  email: string;
  link?: string;
  error?: string;
}

export interface LinksResponse {
  address: string;
  items: LinkItem[];
}

export interface OutboundTraffic {
  tag: string;
  up: number;
  down: number;
  updatedAt: number;
}

export interface XrayTemplateResponse {
  template: Record<string, unknown>;
  traffic: OutboundTraffic[];
}
