# 架构设计（ARCHITECTURE）

> 最后核对：2026-11-04，对照 `src/`、`migrations/`、`frontend-3xui/`、`deploy/`。
> 行为以代码为准；本文描述“为什么这样组织”。

## 1. 总体形态

单进程、单节点、单管理员的 Xray 控制面板：

```
浏览器
  │  HTTP（无内置 TLS）
  ▼
┌──────────────────────────── Rust 二进制（tokio N workers）────────────────────────────┐
│  外层 Router                                                                          │
│    └─ base_path guard：按配置剥掉面板前缀（/ 或 /xxx/）                                │
│  内层 Router                                                                          │
│    ├─ 静态资源 fallback（rust-embed 的 web/dist）                                      │
│    ├─ /api/*            原生 Rust API                                                 │
│    ├─ /panel/api/*      3x-ui 兼容 API                                                │
│    ├─ /login /settings  独立 HTML 页面                                                │
│    └─ require_auth + CSRF（受保护路由）                                                │
│                                                                                        │
│  AppState：SQLite 连接池、Config、XrayHandle、StatsClient、在线/日志/历史等内存状态     │
│  jobs：流量/在线轮询；maintenance：会话清理 + 内存释放                                 │
└───────────────────────────────────────┬───────────────────────────────────────────────┘
                                        │ spawn / kill / gRPC StatsService
                                        ▼
                              Xray-core 子进程（配置由面板生成）
```

- 面板负责 Xray 的**配置生成、校验、启动、监管、重启和流量统计**。
- 不依赖外部数据库/消息队列/Node 运行时；前端静态资源嵌入二进制。
- 目标产物：`x86_64-unknown-linux-musl` 静态 ELF。

## 2. 进程与运行时

- `src/main.rs`：解析环境变量 → 打开/迁移 SQLite → 应用数据库中保存的运行时设置
  → 创建 `AppState` → 启动 Xray supervisor、维护任务、流量 job → 监听 HTTP →
  优雅停机（SIGINT/SIGTERM）。
- Tokio 为多线程运行时，默认 **2 个 worker**（`PANEL_WORKERS`），低内存优先。
- 反压/超时：HTTP body 上限 64 MiB；Xray gRPC 连接超时 3s、RPC 超时 10s。
- HTTP 监听地址：环境变量 `PANEL_LISTEN` 是首次启动默认值，之后以数据库设置
  `x3ui_settings.webListen/webPort` 为准（重启生效）。
- 面板路径（`webBasePath`）是唯一**保存即生效**的网络设置。

## 3. 后端模块

| 模块 | 职责 |
| --- | --- |
| `src/config.rs` | 环境变量解析、Xray API URL/端口推导 |
| `src/state.rs` | `AppState`：DB、配置、Xray 句柄、统计客户端、内存状态、`base_path`、出站检测结果与唤醒信号 |
| `src/db/` | sqlx 轻量查询（无 ORM）；按表拆分模块 |
| `src/api/` | HTTP 处理器：原生 API、3x-ui 兼容 API、认证、备份、出站测试、IP 位置查询 |
| `src/panel_log.rs` | 面板日志：内存环形缓冲、UTC+8 时间、按日文件、历史文件列表 |
| `src/outbound_check.rs` | 高级设置中的出站真实连接周期检测与企业微信告警 |
| `src/auth.rs` | Argon2id、会话 Cookie、CSRF 校验、认证中间件 |
| `src/notify.rs` | 登录失败的企业微信 webhook 通知、真实 IP 头解析、文本消息清理 |
| `src/service.rs` | 由 DB 生成 Xray 配置、写盘、测试、应用/重启 |
| `src/jobs.rs` | 周期流量/在线采集、配额与到期执行、重置、历史记录 |
| `src/traffic.rs` | 纯计数器增量：缺失/负数保留基准、回绕按当前值计新增 |
| `src/db/client_traffic.rs` | per-attachment 基准的事务写入、全量/确认重置、reconcile 标记 |
| `src/xray/supervisor.rs` | Xray 子进程生命周期、日志读取、退避重启 |
| `src/xray/config.rs` | 生成最终 Xray 配置，强制 `stats/api/policy/routing` 约束 |
| `src/xray/identity.rs` | 统计身份 `<inbound_id>:<email>` 与流量 pattern |
| `src/xray/stats.rs` | gRPC StatsService 客户端、在线用户解码、超时与重连 |
| `src/xray/link.rs` | 分享链接生成 |
| `src/xray/subscription.rs` | 订阅正文解析、分享链接→出站、标签稳定分配 |
| `src/xray/router.rs` | RoutingService gRPC：balancer 实时状态与手动切换 |
| `src/updates.rs` | Xray Release 列表/安装、Geofiles 下载、更新代理、GEOdata 标准源 |
| `src/panel_update.rs` | 面板 GitHub Releases 检查、SHA-256 校验、二进制原子替换与 systemd 重启 |
| `src/outbound_subs.rs` | 出站订阅 CRUD/拉取/SSRF/代理、运行时合并、定时刷新 |
| `src/web/` | 静态资源服务、HTML 注入、面板路径中间件、独立设置页 |
| `src/sys.rs` | Linux `/proc` / `statvfs` 指标（CPU/内存/网络/磁盘/RSS/线程等） |
| `src/api/public_ip.rs` | 公网 IPv4/IPv6 查询，10 分钟缓存，后台刷新 |
| `proto/stats.proto` | Xray StatsService gRPC 定义 |

分享链接/二维码的主机来源：设置页 `nodeAddress`（存在 `settings.x3ui_settings`）→
后端 `public_address` 旧设置 → 请求 Host。前端 `preferPublicHost` 在 `nodeAddress`
非空时始终使用它（即使浏览器打开的是局域网地址）；每个入站自己的
`shareAddrStrategy`/`shareAddr` 仍然优先。

面板在线更新：`panel_update` 从 `PANEL_GITHUB_REPO`（CI 编译时注入为当前仓库，
本地默认 `ztoofar/xray-panel-rs`）的 latest release 下载
`xray-panel-rs-linux-amd64-lumen.tar.gz`，用 GitHub 为资产计算的 SHA-256 digest
校验，再用内置的最小 tar.gz 解析器取出 `xray-panel-rs`，写入同目录临时文件并
原子替换当前可执行文件；服务单元设置 `PANEL_SYSTEMD=1`（旧单元用
`INVOCATION_ID` + `systemctl show MainPID` 兼容判断），确认由 systemd 托管时
1.5s 后 `exit(1)`，由 `Restart=on-failure` 拉起新版本，否则返回
`restartRequired:true`。

## 4. HTTP 请求流程

1. **base_path guard**（仅当路径不是 `/`）：`/api/health` 放行；其他请求必须
   匹配配置前缀，剥掉前缀后重写 URI；不匹配直接 404。
2. **内层路由匹配**：静态 fallback / 原生 API / 兼容 API / 独立页面。
3. **受保护路由**：`require_auth` 校验 `xpanel_session` Cookie（数据库存 SHA256），
   过期返回 401；非 GET 请求校验 `X-CSRF-Token` 等于会话中的 CSRF。
4. 处理器读写 SQLite；涉及 Xray 的变更调用 `service::apply`。
5. 兼容 API 统一返回 `{success,msg,obj}`；原生 API 成功返回数据、失败返回
   `{"error": ...}` 和相应 HTTP 状态。

### 4.1 面板路径为什么用“外层 Router 包内层 Router”

axum 的 `Router::layer` 在**路由选择之后**才运行：如果 guard 和路由在同一层，
`/secret/panel/...` 会先因为没有匹配路由而落到 fallback，再被改写也来不及。
因此 `src/api/mod.rs` 用外层 Router 的 `fallback_service(内层 Router)`，guard 在
外层命中 fallback 后、内层重新路由前执行。详见 `DECISIONS.md` D009。

### 4.2 静态资源与 HTML 注入

- `web/dist/index.html`、`login.html` 由 Vite 生成，资源引用是根绝对路径
  `/assets/...`。服务 HTML 时按当前前缀改写为 `/<base>/assets/...`，并注入
  `window.X_UI_BASE_PATH` 与 `window.X_UI_CUR_VER`。
- 前端 `HttpUtil`、react-router basename、登录跳转都读取 `X_UI_BASE_PATH`。
- 独立 `/settings` 页是 Rust 内的静态 HTML 模板，用 `__BASE__` 占位符注入前缀。
- 资源请求经 guard 剥前缀后由 `rust-embed` 按路径返回；未知静态路径若最后一段
  不含 `.`，回退到 SPA `index.html`。

## 5. Xray 生命周期

1. 启动或配置变更时，`service::state_config(runtime=true)` 从数据库读取启用中的
   入站/客户端 + `xray_extra`，生成完整配置。`write_current`/`test_extra` 还会
   读取启用中的出站订阅，把其最近一次成功拉取的出站按 `prepend`/`priority`
   合并到 `outbounds` 两侧；该内容不写回 `xray_extra`（API 编辑模板也看不到）。
2. 面板强制覆盖：`inbounds`、`stats`、`api`、routing 中的 API 规则；`log` 保证有
   `loglevel`；`outbounds` 缺失时给 direct/blocked 默认值。`policy` 只强制每个
   level 的 `statsUserOnline`，并在缺失时补上 uplink/downlink 统计默认值；
   用户显式设置的 `connIdle`、`bufferSize`、`handshake` 和统计开关保持不变。
   Xray 页面读取的是生成后的“可编辑模板”（去掉 inbounds/api/stats），保存时
   整份写回 `xray_extra`（与 3x-ui 一样）；模板里的 `inbounds` 不参与运行时。
   入站按协议整形：vless/vmess/trojan/shadowsocks/hysteria/masque 从客户端表
   重建 `clients`，wireguard 重建 `peers`，masque 把 `password` 写成 `pass`
   且邮箱不能含 `:`；hysteria 强制 `version: 2`；tunnel/tun 原样透传。
   仅 Xray 原生协议进入配置；`src/xray/config.rs` 的 `XRAY_INBOUND_PROTOCOLS`
   之外的协议（3x-ui 的 sidecar 协议）会被跳过，避免 `xray run -test` 失败。
3. 写入临时文件（保留 `.json` 后缀）→ `xray run -test -c <temp>`；失败则删除临时
   文件并保留线上配置。
4. 成功后把旧配置复制为 `config.json.bak`，原子替换目标文件。
5. supervisor 通过 `xray run -c <config>` 启动子进程；stdout/stderr 同时写入
   内存环形缓冲（最多 500 行）和 `PANEL_XRAY_LOG` 文件。
6. 子进程退出：记录退出码，按 2s → 4s → … → 30s 退避重启；generation 计数递增，
   面板据此清空在线状态。
7. 手动停止只停 Xray 子进程，supervisor 和面板继续运行，可从仪表盘再启动。

> 不要把同一份 Xray 再用 systemd 启动，否则 API 端口 10085 或入站端口会冲突。

### 5.1 出站订阅注入

1. 后台 job 每 30 秒查询 `enabled=1` 且 `last_updated + update_interval <= now`
   的订阅（首次启动立即到期）。
2. 拉取前重新校验 URL：仅 http(s)，解析所有 A/AAAA 并拒绝私有/回环/链路本地
   地址（`allowPrivate` 可关闭检查）。配置了 `proxy_url` 时，本地解析失败会放行，
   由代理解析。
3. `ureq` 在阻塞线程中拉取（30s 超时、10 次重定向、8 MiB 上限），支持
   `http(s)`/`socks4/5(h)` 代理与 `allowInsecure`。正文按 base64 或纯文本拆行，
   逐条解析 vmess/vless/trojan/ss/hysteria2/wireguard 链接。
4. 标签稳定：identity（去掉 remark 的链接核心）映射优先，其次复用上一次同位置
   标签，最后用 `tagPrefix + slug(remark)` 分配；同批次冲突追加 `-N`。
5. 结果写入 `last_fetched_outbounds`/`link_identities` 后，如内容变化则调用
   `service::apply`。完整运行时配置若未通过 `xray run -test`，回滚到上一次
   成功列表并记录 `last_error`。手动刷新与后台任务走同一路径。
6. `GET /panel/api/xray/` 把启用订阅的出站和 tag 作为 `subscriptionOutbounds`/
   `subscriptionOutboundTags` 返回；前端只读展示，并允许在路由/balancer 中选择。

### 5.2 Xray 版本 / Geofiles / GEOdata

1. 设置页「运行环境 → 更新代理」的 `updateProxy`（http(s)/socks）用于
   GitHub Release 查询、Xray 二进制下载和 Geofiles 下载；留空直连。
2. 版本列表来自 GitHub Releases（≥26.6.27）；安装时停止 Xray，下载对应平台
   zip，校验同一 Release 的 `.dgst` SHA2-256，解压 `xray`（Windows 为
   `xray.exe`），在目标目录内原子替换 `XRAY_BIN`，再重启 Xray。失败时保留旧
   二进制并尝试恢复子进程。
3. Geofiles 只接受内置白名单，按上游 `.sha256sum` 校验；写入
   `XRAY_LOCATION_ASSET`（存在时）或 Xray 二进制所在目录，成功后重启 Xray。
4. GEOdata 使用 Xray 原生 `geodata` 顶层配置块；`/panel/api/xray/` 返回
   `geodataSources` 标准源，下载出站由该块的 `outbound` 选择（可选代理出站）。
5. 兼容 API `api.services` 现在包含 `StatsService`、`HandlerService`、
   `RoutingService`；负载均衡状态/切换通过 `src/xray/router.rs` 调用。

## 6. 流量、在线与配额

- 轮询周期：`traffic_poll_secs`（默认 5 秒，校验范围 2–300）；前端入站列表工具栏
  可改，保存后通过 `Notify` 立即唤醒 job。
- 每 tick：
  1. 读取 clients / client_inbounds / inbounds，构建 attachment 身份映射。
  2. `GetUsersStats`（若核心支持）拿连接级在线用户与 IP；未知统计身份直接丢弃。
   在线 IP 存在 `AppState.online_clients`，由 `POST
   /panel/api/clients/onlineIps` 提供给客户端详情弹窗。Xray 的 OnlineMap
   会丢弃回环地址，因此本机/回环来源的客户端不会显示 IP。
   位置查询是独立的按需接口 `POST /panel/api/clients/ipLocation`，不在轮询
   中调用；服务端固定访问 ipwho.is，失败回退 ipapi.co。
  3. `QueryStats("", false)` 拿累计计数，按统计身份拆成
     `identity -> (uplink, downlink)`；缺失的方向保留为 `None`，不当成 0。
  4. 每个 `(client, inbound)` attachment 单独计算增量并与自己的基准比较；
     计数器回绕时把当前值视为新增量。缺失或负数样本保留原基准。
  5. 在**一个 SQLite 事务**内写回客户端累计值、per-attachment 基准和
     `last_online`；inbounds/outbound_traffics 同样按方向独立判断。
  6. 执行配额、到期、`limitIp` 超限禁用；按 `reset_days`/入站定时策略重置。
     只有全部 Xray 计数器都确认重置成功才清数据库；部分失败时只标记
     `needs_reconcile`，下一轮取到样本后“只更新基准、不记增量”。
  7. 更新在线集合（连接信号 ∪ 本轮流量信号），窗口 = `2×poll` 并限制在 20–60 秒。
  8. 记录全局流量历史（最多 240 点）。
- Xray 子进程**新启动**时（supervisor 的 `start_epoch` 增加，首次启动也算），
  先在一个事务里把 clients/inbounds/outbound_traffics/client_inbound_traffic
  的 `last_*` 基准清零（不动累计 `up/down`），再开始采样。generation 变化
  （启动或停止）则清空在线集合并让在线 API 探测缓存失效。这样新进程的计数器
  不会被拿去减旧基准；若面板启动时没有子进程（`start_epoch == 0`）则保留基准，
  兼容仍存活的残留进程。
- 统计身份是 `<inbound_id>:<email>`：同一客户端关联多个入站时，Xray 的每个
  attachment 有独立计数器；面板逐 attachment 计算增量后汇总回客户端，并用身份
  反查入站 tag。配置写盘时 routing 的 `user` 规则会从面板邮箱展开为这些身份；
  API 返回仍用邮箱。外部配置直接使用裸邮箱时保留旧聚合路径（clients.last_*）。
- 配额/到期禁用后是否重启 Xray 由 `restartXrayOnClientDisable` 控制（默认 true）。

## 7. 认证与安全

- Argon2id，参数 `m=8192 KiB, t=2, p=1`（为 512 MiB VPS 控制登录成本）。
- 会话：随机 token 存 Cookie（HttpOnly、SameSite=Lax、Path=/），数据库只存
  SHA256(token)；默认 TTL 7 天，可由 `sessionMaxAge` 设置（分钟）。
- CSRF：每个会话一个随机 token，登录时同时下发非 HttpOnly 的 `xpanel_csrf`
  Cookie；非 GET 请求要求 `X-CSRF-Token` 头匹配。
- 登录限流：每 IP 60 秒窗口内最多 5 次失败，内存 map 上限约 1024 个 IP。
- 登录失败通知：两个登录入口在校验失败后异步 POST 企业微信群机器人文本消息，
  包含尝试账户、真实 IP、时间和入口；配置项 `loginNotifyWebhook` 为空则关闭。
  真实 IP 信任 `X-Real-IP` / `CF-Connecting-IP` / `X-Forwarded-For`，最后回退
  TCP 对端；直接暴露面板时这些请求头可伪造，只用于通知展示。
- 面板不内置 TLS；公网部署需 HTTPS 反代 + `PANEL_SECURE_COOKIE=true`。
- 面板路径是**混淆措施，不是安全边界**；`/api/health` 始终在根路径可达。

## 8. 前端

- 生效目录：`frontend-3xui/`（3x-ui 前端的裁剪副本），构建到 `web/dist/`，
  Rust 用 `rust-embed` 嵌入。
- 路由仅 7 个：`/`（系统状态）、`/inbounds`、`/clients`、`/outbound`、`/routing`、
  `/xray`、`/settings`；登录页与独立设置页是单独的 HTML 入口。
- 数据层：TanStack Query + `HttpUtil`；WebSocket 已禁用，靠轮询/操作后失效缓存。
- `frontend/` 与 `web/dist-custom-v2/` 是早期自研前端，保留但**不参与构建**。
- 前端 CLAUDE.md（`frontend-3xui/CLAUDE.md`）来自上游 3x-ui，其中
  `internal/web/dist` 等描述不适用于本项目；构建输出以根 `CLAUDE.md` 为准。

## 9. 后台任务

| 任务 | 周期 | 作用 |
| --- | --- | --- |
| 流量/在线 job | `traffic_poll_secs`（2–300s，默认 5s） | 统计、在线、配额/到期/重置、历史 |
| 出站订阅 job | 30 秒检查一次 | 到期的启用订阅拉取/解析/保存；内容变化时重启 Xray |
| 出站连接检测 job | 10 秒轮询配置；按设置的 1–1440 分钟频率执行 | 用临时 Xray 实例对选中的出站做 SOCKS + HTTP 204 真实探测；不可用/恢复时经企业微信通知 |
| maintenance | 15 分钟 | 清理过期会话；`PRAGMA shrink_memory`；收缩内存容器；glibc `malloc_trim` |
| 公网 IP 刷新 | 缓存 10 分钟，按需后台刷新 | 仪表盘 IPv4/IPv6 |

### 日志来源（日志弹窗）

- 面板日志：`tracing` 同时输出到 stdout、内存环形缓冲和按日文件
  （`src/panel_log.rs`）。内存缓冲最多 500 行；文件默认在 DB 同目录
  `panel-logs/panel-YYYY-MM-DD.log`，可用 `PANEL_LOG_DIR` 覆盖，保留 14 天。
  所有层固定按 UTC+8 渲染时间（与主机时区无关），缓冲/文件层记录 target、
  文件与行号。接口 `POST /panel/api/server/logs/{rows}` + `source=panel`
  读实时缓冲；再加 `file=<name>` 读历史文件。`GET
  /panel/api/server/panelLogFiles` 列出可用历史文件。内容包括启动/运行告警、
  登录审计（成功、失败、限流、登出、改密）、设置保存、入站/客户端增删改、
  Xray 重启/停止、面板重启和出站检测结果。
- Xray 核心日志：读取当前配置 `log.error` / `log.access` 指向的文件末尾
  （相对路径按面板 cwd 解析，Xray 子进程继承同一 cwd）；tail 最多约 1 MiB。
  `source=xray&file=error|access`。error 未配置时回退到 supervisor 捕获的
  stdout/stderr 环形缓冲；access 未配置时返回空并提示。
- 前端 `LogModal` 是一个窗口，顶部下拉在“面板日志 / Xray 错误日志 /
  Xray 访问日志”之间切换，默认显示面板日志；面板日志还可选择“实时日志”
  或某一天的历史文件，历史模式不参与自动刷新。

## 10. 部署

- `deploy/install.sh`：安装二进制到 `/usr/local/bin/xray-panel-rs`，数据目录默认
  `/etc/xray-panel`，环境文件 `/etc/xray-panel.env`（0600），安装 systemd 服务。
- systemd：`Restart=on-failure`、`NoNewPrivileges`、`PrivateTmp`、
  `ProtectSystem=full`、`UMask=0077`。
- 备份：下载 SQLite `.db`；恢复会覆盖当前数据库并重启 Xray，要求 schema 一致。
