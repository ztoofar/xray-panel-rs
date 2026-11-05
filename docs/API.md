# API 设计（API）

> 路由唯一事实来源：`src/api/mod.rs`。本文按用途分组说明约定，不逐字段复制
> 每个处理器；改接口时同步更新本文件。最后核对：2026-11-01。

## 1. 基础约定

### 1.1 面板前缀

- 默认前缀 `/`；可在设置页设置 `webBasePath`（`/xxx/`）。
- 设置后所有路径（页面、静态资源、API）都要加前缀，例如
  `/secret/panel/api/clients/list`。
- 唯一例外：`/api/health` 始终保留在根路径。
- 前缀不匹配 → 直接 404；旧地址不会重定向。

### 1.2 认证

- 登录成功后下发 `xpanel_session` Cookie（HttpOnly、SameSite=Lax、Path=/）。
- 所有受保护路由由 `require_auth` 校验会话 Cookie：
  - Cookie 缺少/过期 → `401`。
  - 其中非 GET 请求还必须带 `X-CSRF-Token`，值等于当前会话的 CSRF token；
    不匹配 → `403`。
- CSRF token 来源：
  - 原生登录 `POST /api/login` 返回 JSON 中的 `csrf`；
  - 3x-ui 登录 `POST /login` 同时下发 `xpanel_csrf` Cookie；
  - `GET /csrf-token`、`GET /panel/csrf-token` 可在已有会话下获取。
- 登录限流：同一 IP 60 秒内 5 次失败后拒绝。
- 登录失败通知：面板设置 `loginNotifyWebhook` 非空时，密码错误会异步 POST 企业微信
  群机器人文本消息（尝试账户、真实 IP、时间、入口）；成功登录、空凭据和限流不发送。
  真实 IP 依次取 `X-Real-IP`、`CF-Connecting-IP`、`X-Forwarded-For` 首项，回退到 TCP 对端。

### 1.3 响应格式

**原生 API**（`/api/*`）：

- 成功：HTTP 2xx + 数据对象。
- 失败：非 2xx + `{"error": "..."}`。
- 分页等结构由各处理器定义。

**3x-ui 兼容 API**（`/panel/api/*`）：

```json
{ "success": true, "msg": "", "obj": { } }
{ "success": false, "msg": "原因", "obj": null }
```

多数业务失败仍返回 HTTP 200 + `success:false`（与 3x-ui 前端兼容）。

### 1.4 请求体与上传

- 兼容 API 同时接受 `application/json`、`application/x-www-form-urlencoded`、
  `multipart/form-data`。
- 路由组 Body 上限 **64 MiB**（用于 DB 恢复/大配置）。
- 数据库恢复上传字段名可为 `db`、`file`、`database`；必须是 SQLite 文件头。
- 时间：客户端 `expiryTime` 以毫秒写入；API 返回的客户端
  `lastOnline`/`createdAt`/`updatedAt` 为毫秒；入站 `expiryTime` 原样返回。

## 2. 公开端点（无需会话）

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/api/health` | `{ok,name,version}`；根路径始终可达 |
| POST | `/api/login` | JSON `{username,password}`；成功设置会话 Cookie 并返回 `{authenticated,username,csrf}` |
| GET | `/api/session` | 返回当前会话 `{authenticated,username,csrf}`；未登录时 `authenticated:false` |
| GET | `/csrf-token`、`/panel/csrf-token` | 返回 `{success,obj:"<token>"}` 并设置 CSRF Cookie |
| GET | `/login` | 登录页 HTML |
| POST | `/login` | 3x-ui 表单登录（字段 `username`、`password`） |
| GET | `/settings` | 独立设置页 HTML（账号/DB 备份恢复） |
| POST | `/logout` | 兼容登出 |
| POST | `/getTwoFactorEnable` | 固定返回 `false`（未实现 2FA） |
| GET | `/` | 登录页 HTML（无前缀时） |
| GET | `/<任意无点路径>` | SPA 入口（仅前缀 `/` 时；自定义前缀下须带前缀） |

## 3. 原生 API（受保护，`/api/*`）

### 会话与账户

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| POST | `/api/logout` | 删除当前会话 |
| POST | `/api/credentials` | 修改用户名/密码，旧会话失效 |

### 仪表盘

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/api/dashboard` | Xray 状态、系统指标、入站/客户端计数、在线数、总流量、历史、版本、启动时间 |

### 入站

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET/POST | `/api/inbounds` | 列表 / 新建 |
| GET/PUT/DELETE | `/api/inbounds/{id}` | 详情 / 更新 / 删除 |
| POST | `/api/inbounds/{id}/toggle` | 启停 |
| GET | `/api/inbounds/{id}/links` | 分享链接 |

### 客户端

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET/POST | `/api/clients` | 列表 / 新建 |
| GET | `/api/clients/online` | 在线客户端 |
| GET/PUT/DELETE | `/api/clients/{id}` | 详情 / 更新 / 删除 |
| POST | `/api/clients/{id}/toggle` | 启停 |
| POST | `/api/clients/{id}/reset` | 重置流量 |

### 设置与 Xray

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET/PUT | `/api/settings` | 外部地址等原生设置 |
| GET | `/api/xray/status` | supervisor 状态 |
| GET | `/api/xray/config` | 生成的运行配置 |
| POST | `/api/xray/test` | 用临时文件执行 `xray run -test` |
| POST | `/api/xray/restart` | 应用配置并重启 Xray |
| GET | `/api/xray/logs?limit=N` | 面板内存中的 Xray 日志（默认 200，最大 500 行） |
| POST | `/api/xray/new-x25519` | 生成密钥 |
| GET/PUT | `/api/xray/template` | 读取/保存 Xray 额外配置模板 |
| PUT | `/api/xray/outbounds` | 保存 outbounds |
| POST | `/api/xray/outbounds/reset` | 重置全部出站流量 |
| POST | `/api/xray/outbounds/{tag}/reset` | 重置单出站流量 |
| PUT | `/api/xray/routing` | 保存 routing |
| PUT | `/api/xray/dns` | 保存 dns |
| PUT | `/api/xray/log` | 保存 log 配置 |

### 备份

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/api/server/db` | 下载 `VACUUM INTO` 快照 |
| POST | `/api/server/db/restore` | 上传 .db 覆盖恢复（重启 Xray） |

## 4. 3x-ui 兼容 API（受保护，`/panel/api/*`）

> 这些路径由裁剪版 3x-ui 前端调用。完整清单以 `src/api/mod.rs` 为准；下面是
> 已实现的主要分组。

### 状态 / 设置 / 服务器

| 方法 | 路径 | 状态 |
| --- | --- | --- |
| GET | `/panel/api/status` | 已实现（版本、dbType） |
| POST | `/panel/api/setting/all` | 已实现（3x-ui 设置全集，含 `webBasePath`、`ipDataCloudKey`） |
| POST | `/panel/api/setting/update` | 已实现（校验并保存；`webBasePath` 立即生效；`ipDataCloudKey` 按字符串保存，去掉首尾空白并拒绝空白字符） |
| POST | `/panel/api/setting/defaultSettings` | 已实现（页面默认值） |
| POST | `/panel/api/setting/factoryDefaults` | 已实现（默认值） |
| GET/POST | `/panel/api/setting/getDefaultJsonConfig` | 已实现：返回默认可编辑 Xray 模板（含 policy 默认统计开关） |
| POST | `/panel/api/setting/updateUser` | 已实现（改管理员账号） |
| POST | `/panel/api/setting/restartPanel` | 已实现（进程退出，由 systemd 拉起）；设置保存本身不会重启 |
| GET | `/panel/api/server/panelLogFiles` | 已实现：列出面板历史日志文件 `{name,date,size}`（默认保留 14 天） |
| GET | `/panel/api/server/status` | 已实现（系统指标 + Xray 状态） |
| GET | `/panel/api/server/getConfigJson` | 已实现 |
| GET | `/panel/api/server/getXrayVersion` | 已实现：GitHub Release 列表（经更新代理，筛选 ≥26.6.27） |
| POST | `/panel/api/server/installXray/{version}` | 已实现：下载/校验/解压/替换 Xray 并重启 |
| POST | `/panel/api/server/updateGeofile`、`/{file}` | 已实现：按白名单下载 Geofiles，SHA-256 校验后原子替换并重启 |
| POST | `/panel/api/server/logs/{rows}` | 已实现：`source=panel` 返回面板 tracing 环形缓冲（最多 500 行）；`source=panel&file=<panel-YYYY-MM-DD.log>` 读历史文件；`source=xray&file=error\|access` 返回 `log.error`/`log.access` 文件末尾（error 未配置时回退到面板捕获的 Xray 进程输出）；不带参数保持旧行为（进程日志） |
| POST | `/panel/api/server/restartXrayService` | 已实现 |
| POST | `/panel/api/server/stopXrayService` | 已实现（只停 Xray 子进程） |
| GET | `/panel/api/server/panelUpdateCheck` | 已实现：查询 GitHub Releases 最新版本，返回 `current/latest/updateAvailable/releaseUrl` |
| POST | `/panel/api/server/panelUpdate` | 已实现：下载 Release 的 tar.gz，用 GitHub 资产 digest 校验并解出二进制，原子替换后退出（systemd `Restart=on-failure` 拉起）；非 systemd 返回 `restartRequired:true` |
| GET | `/panel/api/server/getNewUUID` | 已实现 |
| GET | `/panel/api/server/getNewX25519Cert` | 已实现 |
| GET | `/panel/api/server/getNewVlessEnc` | 已实现（调用 `xray vlessenc`） |
| GET | `/panel/api/server/getNewmldsa65`、`getNewmlkem768` | 已实现（调用 `xray mldsa65` / `mlkem768`） |
| POST | `/panel/api/server/getNewEchCert` | 已实现（`xray tls ech --serverName`） |
| POST | `/panel/api/server/getCertHash` | 已实现：证书文件（仅限入站/面板已引用的路径）或 PEM 内容的 SHA-256 DER 哈希 |

### 入站

| 方法 | 路径 | 状态 |
| --- | --- | --- |
| GET | `/panel/api/inbounds/list` | 已实现 |
| GET | `/panel/api/inbounds/list/slim` | 已实现（列表页轻量载荷） |
| GET | `/panel/api/inbounds/get/{id}` | 已实现 |
| GET | `/panel/api/inbounds/options` | 已实现（客户端表单入站选择器） |
| POST | `/panel/api/inbounds/add` | 已实现；接受的入站协议见下方「协议清单」 |
| POST | `/panel/api/inbounds/import` | 已实现：接受 3x-ui 导出的单个 inbound JSON（`data`），导入客户端、入站累计流量与 clientStats 流量；共享邮箱保留第一次导入的计数并强制启用 |
| POST | `/panel/api/inbounds/update/{id}` | 已实现：包含 shareAddrStrategy/shareAddr/subSortIndex/excludeFromSub/disableFlow |
| POST | `/panel/api/inbounds/del/{id}` | 已实现 |
| POST | `/panel/api/inbounds/setEnable/{id}` | 已实现 |
| GET | `/panel/api/inbounds/allLinks` | 已实现：按 shareAddrStrategy 选择地址，生成全部客户端分享链接 |
| GET/POST | `/panel/api/inbounds/{id}/fallbacks` | 已实现 |
| POST | `/panel/api/inbounds/{id}/resetTraffic` | 已实现 |
| POST | `/panel/api/inbounds/{id}/delAllClients` | 已实现 |
| POST | `/panel/api/inbounds/bulkDel` | 已实现 |
| POST | `/panel/api/inbounds/resetAllTraffics` | 已实现 |

#### 协议清单

`add`/`import`/`update` 接受以下 Xray 原生入站协议：
`vless`、`vmess`、`trojan`、`shadowsocks`、`http`、`mixed`、`wireguard`、
`hysteria`、`tunnel`、`tun`、`masque`。

`mtproto`、`tuic`、`amneziawg` 在 3x-ui 中由外部 sidecar（mtg-multi、
tuic-server、amneziawg-go）承载，本面板不内置这些运行时，因此明确返回
`success:false`，不写入数据库。出站侧同样只暴露 Xray 原生协议；`amneziawg`
出站不出现在下拉框中。

### 客户端

| 方法 | 路径 | 状态 |
| --- | --- | --- |
| GET | `/panel/api/clients/list` | 已实现 |
| GET | `/panel/api/clients/list/paged` | 已实现（分页/筛选/排序） |
| POST | `/panel/api/clients/add` | 已实现（兼容嵌套 `{client,inboundIds}`） |
| POST | `/panel/api/clients/bulkCreate` | 已实现 |
| GET | `/panel/api/clients/get/{email}` | 已实现 |
| POST | `/panel/api/clients/update/{email}` | 已实现 |
| POST/DELETE | `/panel/api/clients/del/{email}` | 已实现 |
| POST | `/panel/api/clients/resetTraffic/{email}`、`resetAllTraffics` | 已实现 |
| POST | `/panel/api/clients/bulkEnable`、`bulkDisable`、`bulkDel`、`bulkAdjust` | 已实现 |
| POST | `/panel/api/clients/bulkAttach`、`bulkDetach`、`/{email}/attach`、`/{email}/detach` | 已实现 |
| POST | `/panel/api/clients/groups/bulkAdd`、`bulkRemove` | 已实现（分组即 group_name 批量赋值） |
| GET | `/panel/api/clients/groups` | 已实现 |
| GET | `/panel/api/clients/subLinks/{sub_id}` | 已实现（二维码/分享链接） |
| POST | `/panel/api/clients/onlines` | 已实现 |
| POST | `/panel/api/clients/onlinesByGuid` | 已实现（空映射） |
| POST | `/panel/api/clients/onlineInbounds` | 已实现（按 attachment 归属） |
| POST | `/panel/api/clients/onlineIps` | 已实现：`email -> [ip...]`，来自 Xray `GetUsersStats` 连接级在线 IP（回环地址由 Xray 自身丢弃） |
| POST | `/panel/api/clients/ipLocation` | 已实现：`{ip}`，服务端查询 zxinc（纯真库；失败回退 ipwho.is），返回 `{ip,provider,country,region,city,isp,timezone,latitude,longitude}`；zxinc 不提供时区/经纬度（`timezone` 为空字符串，经纬度为 `null`）。配置 `ipDataCloudKey` 后并行查询 IP数据云：成功时附加 `ipDataCloud`（`country/province/city/district/street/isp/usageType/timezone/latitude/longitude`），失败时附加 `ipDataCloudError`；不影响主结果。前端默认不调用，仅在点击“位置”时请求 |
| POST | `/panel/api/clients/activeInbounds` | 已实现 |
| POST | `/panel/api/clients/lastOnline` | 已实现 |
| POST | `/panel/api/clients/delDepleted`、`delOrphans` | 已实现 |
| GET | `/panel/api/clients/export` | 已实现：返回 `{client, inboundIds, traffic}` 数组，可回灌 import |
| POST | `/panel/api/clients/import` | 已实现：接受 `data` 字符串数组，创建客户端、恢复流量；已存在邮箱跳过 |
| 其他 | `/clients/happLink`、`/clients/hwids`、`/clients/ips`、`/clients/clearIps`、`/{email}/externalLinks` | 入口与后端路由已移除；未列路径走 catch-all（不应再依赖） |

#### WebSocket `/ws`（0.2.25 起，3x-ui 兼容）

- 路径：`<basePath>ws`（默认 `/ws`），需要登录会话 Cookie。
- 服务端每个流量采集周期推送：
  `{type:"traffic", payload:{traffics, clientTraffics, onlineClients,
  onlineByGuid, activeInbounds, lastOnlineMap, clientTrafficIntervalMs},
  time}`。
  - `traffics`: `[{Tag, Up, Down, IsInbound, IsOutbound}]`，本轮入站增量；
  - `clientTraffics`: `[{email, up, down}]`，本轮客户端增量；
  - `clientTrafficIntervalMs`: 本轮增量**实际**跨越的采样间隔（毫秒，
    单调时钟测得）；通常约等于配置的轮询长度，tick 超时或漏采时更长。
    首次采样没有上一时间点，不携带增量（此时该字段回退为配置值）。
  - `lastOnlineMap`: `email -> 毫秒时间戳`；在线集合按本地上送。
- 前端用 `clientTrafficIntervalMs` 换算速度；帧中缺失的实体视为空闲并清空
  速度。统计接口失败时推送空增量以清空旧速度。
- 连接断开后浏览器自动重连；重连前/WS 不可用时，前端继续使用 REST 列表中的
  `upSpeed/downSpeed` 兜底。

#### 流量字段与重置语义（0.2.24 起）

- `/panel/api/inbounds/list/slim` 的每个入站行带 `upSpeed`/`downSpeed`
  （字节/秒）；`/panel/api/clients/list`、`/list/paged`、`/get/{email}` 的
  `traffic` 对象带 `upSpeed`/`downSpeed`。速率由流量 job 用本轮增量和真实
  采样间隔计算，空闲时为 0/缺省；前端直接展示，不再自行差分。
- `/panel/api/clients/resetTraffic/{email}`、`resetAllTraffics`、
  `/inbounds/{id}/resetTraffic`、`resetAllTraffics`、`/xray/resetOutboundsTraffic`
  只清数据库累计值，**不重置 Xray 计数器、不清基准**（3x-ui 模型），因此不会
  在下一轮把整段累计值重复计费。

### Xray

| 方法 | 路径 | 状态 |
| --- | --- | --- |
| GET/POST | `/panel/api/xray/` | 已实现：返回可编辑模板（生成配置去掉 inbounds/api/stats） |
| POST | `/panel/api/xray/update` | 已实现：整份保存可编辑模板（log/dns/outbounds/routing/policy/metrics/observatory/…），只忽略 inbounds/api/stats |
| POST | `/panel/api/xray/balancerStatus` | 已实现：返回 `{tag,running,override,selected}`，核心未运行/不支持时 `running:false` |
| POST | `/panel/api/xray/balancerOverride` | 已实现：`tag` + `target`（空 target 清除），经 RoutingService |
| POST | `/panel/api/xray/routeTest` | 入口已移除；路径走 catch-all（不再支持） |
| POST | `/panel/api/xray/resetOutboundsTraffic` | 已实现 |
| POST | `/panel/api/xray/testOutbounds` | 已实现（TCP / 临时 Xray SOCKS 隧道 HTTP 探测） |
| GET | `/panel/api/xray/outboundCheckStatus` | 已实现：返回最近一次周期检测 `{checkedAt, results:[{tag,success,delay,error}]}`（未检测时为空） |
| GET | `/panel/api/xray/getOutboundsTraffic` | 已实现 |
| GET/POST | `/panel/api/xray/getDefaultJsonConfig` | 已实现：返回默认模板 JSON 字符串 |
| GET | `/panel/api/xray/outbound-subs` | 已实现：列出出站订阅（不含原始 outbound 文本） |
| POST | `/panel/api/xray/outbound-subs` | 已实现：新建；返回含 `id`、自动 `tagPrefix`（`subN-`）的对象 |
| POST | `/panel/api/xray/outbound-subs/parse` | 已实现：只拉取+解析预览，不落库 |
| POST/DELETE | `/panel/api/xray/outbound-subs/{id}` | 已实现：更新（含启停） |
| POST | `/panel/api/xray/outbound-subs/{id}/refresh` | 已实现：立即拉取、保存并应用 |
| POST | `/panel/api/xray/outbound-subs/{id}/move` | 已实现：`dir=up|down` 调整顺序 |
| POST | `/panel/api/xray/outbound-subs/{id}/del` | 已实现：删除（DELETE 同义） |

出站订阅字段：`remark`、`url`、`tagPrefix`、`userAgent`、`proxyUrl`、
`updateInterval`（秒，最小 60）、`enabled`、`allowPrivate`、`allowInsecure`、
`prepend`。`proxyUrl` 支持 `http(s)://` 与 `socks4/5(h)://`；留空直连。
列表返回 `outboundCount`、`lastUpdated`、`lastError`，不返回
`lastFetchedOutbounds`/`linkIdentities`。`GET/POST /panel/api/xray/` 的载荷额外
包含 `subscriptionOutbounds`、`subscriptionOutboundTags` 与 `geodataSources`
（订阅出站只读运行时注入，不在可编辑模板中）。

### 节点 / 主机 / 兜底

| 方法 | 路径 | 状态 |
| --- | --- | --- |
| GET | `/panel/api/nodes/list` | 未实现（无界面入口；走 catch-all 空列表） |
| ANY | `/panel/api/nodes/{*rest}` | 未实现（无界面入口；走 catch-all） |
| GET | `/panel/api/hosts/list` | 固定空列表（无主机管理页；仅保留活动页面的 Host 标签读取） |
| ANY | `/panel/api/{*rest}`、`/panel/api` | catch-all：GET 返回空列表/`null`；写操作返回“不支持”错误 |

> catch-all 的 GET 行为是为了让裁剪版前端加载不报错，不代表功能已实现。
> 新增真实接口时应从 catch-all 提升为显式路由，并更新本表。

## 5. 错误与兼容性注意

- 后端面向用户的错误文案统一为中文；Xray 原生输出（如 `xray run -test` 的
  stdout/stderr）保留原文。前端语言切到英文时，后端错误仍显示中文
  （`docs/KNOWN-ISSUES.md` H4）。
- 设置保存（`/panel/api/setting/update`）只持久化，不重启面板或 Xray；
  `xrayTemplateConfig` 在该接口中被忽略，Xray 模板保存走
  `/panel/api/xray/update`。需要重启面板时显式调用
  `/panel/api/setting/restartPanel`。
- 高级设置键：`outboundCheckEnabled`（bool）、`outboundCheckTags`
  （逗号分隔 tag）、`outboundCheckIntervalMinutes`（1–1440）、
  `loginNotifyWebhook`（企业微信机器人，默认通知方式）。
- 未实现功能一律返回 `success:false`（或 4xx/5xx），**不要**依赖空数据判断成功。
- 兼容 API 字段命名/结构跟随 3x-ui 前端期望；修改时优先改后端适配而不是改前端
  表单（前端是裁剪副本）。
- `webBasePath` 保存在 `settings.x3ui_settings` 中，`setting/all` 返回规范化值；
  传入非法字符/`..` 会返回 `success:false`。
- `nodeAddress` 同样保存在 `x3ui_settings`：只接受主机名或 IP（拒绝 `://`、
  `/` 和空格），用于所有分享链接和二维码的主机部分。
  `setting/defaultSettings` 同时把它作为 `nodeAddress` 和 `webDomain` 返回。
  后端分享链接的主机优先级：`nodeAddress` → 旧 `public_address` 设置 →
  请求 Host 头；前端 `preferPublicHost` 在配置了 `nodeAddress` 时始终优先使用它。
- 面板路径修改后，当前会话 Cookie 仍有效（Path=/），但旧 URL 立即 404。
