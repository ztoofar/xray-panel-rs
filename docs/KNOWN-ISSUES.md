# 已知问题与限制（KNOWN-ISSUES）

> 状态标记：
> - **确认**：已在当前代码中确认（读代码或测试/运行验证）。
> - **历史**：来自早期验证记录，本次会话未重新验证。
> - **未验证**：只有记录/推测，尚未在真实环境确认。
>
> 修复后不要删除条目，改为“已修复（日期/提交说明）”，保留历史。

## A. 功能范围限制（设计上不做）

### A1 3x-ui 高级接口未实现 —— 确认
2FA、邮件、订阅服务器、节点管理、HWID 日志没有后端实现。写操作返回明确错误；部分只读接口返回空列表/`null` 以便页面渲染。
涉及：`src/api/compat.rs` 的 `unsupported` / `stub` / `nodes_stub`。
影响：裁剪版 3x-ui 页面上相关按钮可能报错或显示空数据。
（出站订阅已于 2026-11-01 实现，见 B7；订阅服务器仍是另一项未实现功能。）

### A2 无 WebSocket —— 确认
前端 WebSocket 已禁用，所有数据靠轮询/操作后重新拉取。数据最多滞后一个
轮询周期（默认 5 秒，配置范围 2–300 秒）。

### A3 单管理员、无 2FA / API Token —— 确认
只有 `users` 表中的单管理员；`api_tokens` 表是 0001 遗留，无对应功能。
`/getTwoFactorEnable` 固定返回 `false`。

### A4 面板不提供 TLS/证书管理 —— 确认
只监听 HTTP。公网使用必须自建 HTTPS 反向代理，并设置
`PANEL_SECURE_COOKIE=true`；默认账号 `admin/admin` 必须立即修改。

### A5 面板自更新未实现 —— 确认
`/panel/api/server/getPanelUpdateInfo` 固定返回
`currentVersion == latestVersion`、`updateAvailable:false`；
`PanelUpdateModal` 因此不会真正升级。升级需替换二进制并重启服务。

### A6 结构化 Xray access log 接口未实现 —— 确认
`/panel/api/server/xraylogs/{rows}` 固定返回空数组；仪表盘的“日志”按钮读取的是
面板捕获的 Xray stdout/stderr，而不是结构化访问日志。

## B. Xray 集成限制

### B1 在线列表依赖新核心 —— 确认（代码）/ 历史（真实核心）
在线状态依赖 Xray 的 `GetUsersStats` 与 `statsUserOnline`（Xray 26+）。
旧核心下在线列表为空；代码会记住 `Unimplemented`，Xray 重启（generation 变化）
时立即重新探测，否则每 10 分钟重试。

### B2 变更会重启 Xray —— 确认
客户端/入站/路由/出站等变更通过重新生成配置并重启子进程生效，会短暂中断
现有连接。`restartXrayOnClientDisable` 只控制“因禁用客户端触发的重启”。

### B3 面板崩溃可能遗留 Xray —— 历史
面板异常崩溃（非正常停止）时，已启动的 Xray 子进程可能残留。使用 systemd 时
默认 KillMode 会随服务停止子进程；手动运行时需自行清理残留 Xray。

### B4 Xray 配置由面板托管 —— 确认
`inbounds`、`stats`、`api`、routing 的 API 规则由面板强制覆盖；`policy` 只强制
每个 level 的 `statsUserOnline` 并补缺失的统计默认值，连接限制和显式统计开关
会保留。直接编辑生成的 `XRAY_CONFIG` 会在下次应用时被覆盖；应通过 Xray 页面
修改 `xray_extra`（log/dns/outbounds/routing/policy/metrics 等）。模板里的
`inbounds` 不参与运行时，已从高级编辑器隐藏。

### B5 WireGuard 入站 —— 已移除
2026-11-02 起入站表单与兼容 API 不再接受 WireGuard（及 MTProto/AmneziaWG/
TUIC/TUN）。旧数据库中残留的此类入站不会被面板生成或管理。

### B6 Shadowsocks / HTTP 协议限制 —— 历史
- Shadowsocks 传统加密方式只支持单客户端（Xray 限制），推荐 2022-blake3。
- 同一客户端关联多个 Shadowsocks 入站时，应使用相同加密方式和密码。
- HTTP/Mixed 协议没有标准分享链接。

### B7 出站订阅限制 —— 确认（代码 + 本地 E2E）
- 只解析链接列表订阅（base64 或纯文本换行的 vmess/vless/trojan/ss/hysteria2/
  wireguard 链接）；JSON/Clash 格式的出站订阅文档不解析，该行会被忽略。
- 创建/刷新时会先解析域名并拒绝私有/回环/链路本地地址（`allowPrivate` 可关闭）。
  重定向由 ureq 跟随（最多 10 次），没有逐跳重新做私网校验；订阅 URL 由管理员
  配置，属于受信输入，但不要把面板暴露给不可信管理员。
- 配置代理时若面板本地无法解析目标域名，会跳过本地解析、把解析交给代理
  （socks5h/HTTP 代理解析），这是为了支持“面板网络不通、代理可通”的场景。
- 新出站通过完整运行时配置的 `xray run -test` 校验：失败时回滚到上一次成功
  列表并记录 `lastError`，不会覆盖线上配置。3x-ui 的“逐条丢弃坏出站”未实现。
- 标签稳定策略含“同位置旧标签”回退：订阅内容整体换成另一台服务器时，位置相同
  的新出站可能沿用旧标签。
- hysteria2 出站使用当前 finalmask 形状；旧版 Xray 可能拒绝，届时刷新会回滚。

### B8 版本 / Geofiles / GEOdata 更新限制 —— 部分验证
- Windows + 真实 Xray 26.10.31 已验证：Release 列表、安装（`.dgst` 校验、
  zip 解压、替换、重启）、单个 Geofiles 下载（`.sha256sum` 校验、重启）、
  `geodata` 配置块通过 `xray run -test`。
- 未在真实 Linux 上验证：路径权限、systemd 重启方式。
- Geofiles 写入 `XRAY_LOCATION_ASSET`（若已设置且目录存在），否则写入
  `XRAY_BIN` 所在目录。使用 `ProtectSystem=full` 的 systemd 单元时，必须把
  `XRAY_BIN` 目录（默认 `/usr/local/bin`）加入 `ReadWritePaths`，否则 Geofiles
  更新和 Xray 二进制替换都会报 `Read-only file system`。0.2.13 的单元遗漏了
  该路径；源码中的 `deploy/xray-panel.service` 已修复。容器部署同样需要可写挂载。
- GEOdata 的自动下载由 Xray 自身执行，走配置块中的 `outbound`；面板不会代为
  下载 geodata 资产。更新代理只覆盖 GitHub/Geofiles。
- 更新代理地址（可能含凭据）以明文存储在 `x3ui_settings`，仅管理员可读。

## C. 配额与在线执行

### C1 `limitHwid` 只保存不强制 —— 确认
数据库/API/界面保存 `limitHwid`，但没有 HWID 上报通道，未做强制禁用。

### C2 `limitIp` 为近似执行 —— 确认
通过 Xray 在线 IP 数判断，超限会禁用客户端（并可能重启 Xray）。不是严格的
连接级防火墙限制。

### C3 在线宽限窗口最长 60 秒 —— 确认
在线窗口 = `2×poll`，限制在 20–60 秒。客户端断开后最多再显示 60 秒。

### C4 客户端时间单位混用 —— 确认（有兜底）
UI 写入的 `expiryTime` 是毫秒，历史数据可能是秒；代码用阈值
`< 100000000000` 视为秒来兼容。新增代码不要假设单一单位。

## D. 面板路径（`webBasePath`）相关

### D1 路径是混淆，不是安全边界 —— 确认
`/api/health` 始终在根路径可达；端口扫描仍能发现面板。真正安全依赖：
强密码、HTTPS 反代、防火墙、限制来源。

### D2 旧 URL 立即 404 —— 确认
保存新路径后旧地址不再重定向；书签/打开中的页面会失效。前端在设置保存成功
后会跳转到新地址；API 调用方需自行更新。

### D3 静态资源改写依赖 Vite 的 `/assets/` 约定 —— 确认（实现）/ 风险
`serve_entry` 只把 HTML 中 `="/assets/`、`='/assets/` 改写为带前缀路径。
如果未来改变 Vite 的 `base` 或资源引用方式，需要同步修改此处和
`docs/ARCHITECTURE.md`。

### D4 路径自动化测试只覆盖纯函数 —— 确认（测试缺口）
`web::base_path` 有 normalize/strip/rewrite 单元测试；guard + 嵌套 Router +
静态资源重写的完整 HTTP 层自动化测试尚未编写。当前端到端证据是 2026-10-31
的无头 Chrome 手工验证（见 `DEVELOPMENT-LOG.md`）。待办：`TODO.md` T2。

## E. 构建与验证环境

### E1 前端 devDependencies 不完整 —— 确认
当前 `node_modules` 缺少 vitest/storybook/@testing-library 等，导致
`npm run typecheck` 报大量“Cannot find module”和 story/test 类型错误。
这些不是业务代码错误；判断改动以“改动文件无新增错误”为准。
待办：`TODO.md` T3。

### E2 本次未在真实 Linux + 真实 Xray 回归 —— 未验证
2026-10-31 的三项改动只在 Windows debug + fake Xray + 无头 Chrome 上验证，
并做了 musl 交叉编译；没有在真实 Linux 发行版 + 真实 Xray 核心上跑完整流程。
待办：`TODO.md` T4。

### E3 README 与实际行为不一致 —— 确认（文档问题）
至少包括：
- README 写“流量轮询默认 10 秒、可调 5–300 秒”，代码默认 5 秒、范围 2–300；
- README 的“明确不做”提到 IP 连接数限制，但 `jobs.rs` 已实现 `limitIp` 近似执行；
- README 对 `/settings` 与面板路径的描述未包含最新行为。
以代码和 `docs/` 为准；待办：`TODO.md` T1。

## F. 数据与运维

### F1 DB 恢复要求 schema 一致 —— 历史
恢复只做表复制，不自动迁移旧 schema；不兼容备份会失败并回滚。

### F2 备份包含 `api_tokens` 表 —— 确认
虽然当前无 API token 功能，备份/恢复表清单仍包含它，保持与历史 schema 兼容。

### F3 SQLite 页缓存收缩只执行一次 —— 确认（影响有限）
维护任务通过连接池执行一次 `PRAGMA shrink_memory`，不一定覆盖两个池连接；
每连接页缓存上限约 2 MiB。评估项见 `TODO.md` T6。

### F4 内存维护周期固定 15 分钟 —— 确认
没有配置项；如需调整只能改 `src/main.rs` 的 `MAINTENANCE_INTERVAL`。
glibc 下调用 `malloc_trim(0)`；musl 的 mallocng 自行归还空闲页，没有该函数。

## G. 流量统计可靠性（0.2.7 修复）

### G1 已修复：Xray 重启后基准未重置 —— 已修复（2026-10-31）
`jobs.rs` 现在在新 Xray 子进程启动时（`XrayHandle::start_epoch` 增加，停止不
增加），先清零 clients/inbounds/outbound_traffics/client_inbound_traffic 的
`last_*` 基准（累计 `up/down` 不动），因此新进程计数器不会再减旧基准。
generation 变化只负责清在线集合与在线 API 探测缓存。验证：单元测试
`reset_all_baselines_keeps_accumulated_totals`；真机重启场景未验证（见 G6）。

### G2 已修复：定时重置失败仍清零数据库 —— 已修复（2026-10-31，0.2.24 起由 G7 取代）
客户端/入站的定时重置现在要求全部 Xray 计数器确认成功才清零数据库；任一失败
只把该客户端全部 attachment 标记 `needs_reconcile` 并记录本次尝试，不清累计
流量。验证：`partial_reset_is_reconciled_without_billing_the_gap`、
`verified_reset_clears_totals_and_baselines`。0.2.24 起不再重置 Xray 计数器，
这两个测试与 reconcile 机制一起删除，模型见 G7。

### G3 已修复：缺失统计项被当作 0 —— 已修复（2026-10-31）
`QueryStats` 缺失的方向现在是 `None`，保留原基准；负数样本同样被忽略。
验证：`missing_direction_preserves_its_baseline`、`traffic::tests`。

### G4 已修复：多入站共用聚合基准 —— 已修复（2026-10-31）
新增 `client_inbound_traffic`，按 attachment 独立计算增量后汇总给客户端。
被否的“只修补 if 条件”方案见 `DECISIONS.md` D019。
验证：`accumulates_each_attachment_independently`。

### G5 已修复：下载计数器单独重置、未知在线用户、在线 API 重探 —— 已修复（2026-10-31）
- 客户端/入站/出入站都同时比较上/下行基准。
- 在线 API 返回的未知身份直接丢弃（裸邮箱仍接受存在的客户端）。
- generation 变化时失效在线 API 探测缓存并清在线状态。
验证：`unknown_online_identities_are_ignored`、`user_stats_keep_each_attachment_separate`。
在线 API 探测失效无独立单元测试（依赖 Xray 状态），逻辑为直接写原子变量。

### G6 尚未在真实 Xray 上做故障注入 —— 未验证
以上结论来自源码审查与 SQLite 单元测试；尚未在真实 Debian + Xray 上验证：
真实重启后计数、真实 `GetUsersStats` 缺失行为、gRPC 超时恢复。建议按
`TODO.md` T9 的清单执行。`reset_pattern` 已于 0.2.24 从代码中移除。

### G7 0.2.24：重置改为 3x-ui 模型，速率改由后端计算 —— 假/真 Xray 均已验证
- 重置不再调用 `QueryStats(reset=true)`，只清数据库 `up/down`，保留
  `last_*` 基准；原先“重置失败后下一轮重复计整段累计值”的路径消失，
  `needs_reconcile` 不再写入。
- 速率在每个成功采样周期内由后端计算（本轮增量 ÷ 单调时钟真实间隔），
  通过入站 slim 的 `upSpeed/downSpeed` 和客户端 `traffic.upSpeed/downSpeed`
  下发；空闲实体无条目，前端不再差分快照，也不会永久停留在旧速度。
- 验证：
  - `tmp/e2e-traffic-accuracy/e2e.py`（假 Xray + 假 StatsService）13 项全通过；
  - `tmp/e2e-traffic-real/e2e.py`（真实 Xray 26.10.31，Windows；真实
    SOCKS→VLESS→本地 HTTP，限速 ~2 MB/s）15 项全通过：
    8 MiB 下载后客户端 `down=8,388,725`（响应体 + 117 字节 HTTP 头），
    入站 `down=8,388,608`（响应体）；两者相差 117 字节（0.0014%）。
    观测速度 2,158,371 B/s，与限速一致；空闲 3 秒后速度归零；
    重置后只再下载 2 MiB，客户端只新增 `2,097,269`，未重计旧累计。
  - `cargo test` 113/113、clippy 零警告。
- 仍未验证：真实 Linux/musl、真实网络抖动、MASQUE 混合附着的真实计数、
  gRPC 故障注入（见 G6）。
- 上游行为记录：Xray 26 的 freedom 对来自 VLESS/VMess/Trojan/Hysteria/
  WireGuard/Shadowsocks 入站的**私有/回环目标默认黑洞 30–90 秒**
  （SSRF 防护，`defaultBlockPrivateRule`）。需要访问内网时，必须给 direct
  outbound 配 `settings.finalRules: [{"action":"allow","ip":["geoip:private"]}]`。
  3x-ui 使用同一个 Xray 核心，行为相同，不是面板统计问题。

### G9 0.2.25：恢复 3x-ui 实时 WS 推送，无客户端入站速度已验证
- `/ws` 每轮推送 `traffic` 帧（入站/客户端增量、在线集合、最后在线、采集
  间隔）；前端按真实间隔换算，缺失实体清空速度，统计失败时推空增量。
- 无关联客户端的入站速度只依赖 `inbound>>>tag>>>traffic` 增量；fake 与
  无头 Chrome 均验证：Solo（无客户端）行显示速度并在空闲后清除。
- 真实 Xray、fake Xray 与 UI+WS 回归均通过（详见 `CURRENT-STATUS.md`
  §2.40）。仍未验证：真实 Linux/musl、公网网络抖动、gRPC 故障注入。

### G10 0.2.26：WS 帧内存优化 —— 已实现，规模验证有限
- 无订阅者时不构建/序列化流量帧；增量改为直接序列化借用结构体；广播缓冲
  8 → 2 帧；WS 增加 30s Ping、90s 空闲断开、10s 发送超时。
- Windows debug + fake stats 的 3000 客户端场景已验证内存下降（详见
  `CURRENT-STATUS.md` §2.42）；帧字段与 0.2.25 一致。
- **未验证**：真实 Linux/musl 长时运行 RSS、超大客户端规模下的真实内存曲线。
  收益随活跃客户端数近似线性，小规模面板可能看不出明显差别。

### G8 重置边界仍有一个采集周期的残差 —— 与 3x-ui 相同
手动重置与采集并发时，重置前刚采样到的增量可能落在重置之后写入，最多
一个 `traffic_poll_secs` 的流量会留在重置后的累计值里。3x-ui 的
`submitTrafficWrite` 也有同样的边界，属于“重置瞬间”口径而非重复计费。

## H. 日志查看器

### H1 面板日志只在内存中 —— 确认
`src/panel_log.rs` 的环形缓冲最多 500 行，只包含当前面板进程的 `tracing`
输出（按 `PANEL_LOG`/`RUST_LOG` 过滤）；面板重启后清空。不读取 journald，
因此与 `journalctl -u xray-panel-rs` 的内容不完全相同。时间戳为 UTC。

### H2 Xray access/error 日志需要先在 Xray 页面配置 —— 确认
日志弹窗的“Xray 错误/访问日志”读取 Xray 配置里的 `log.error` / `log.access`
文件。默认配置没有设置路径：错误日志回退到面板捕获的 Xray 进程输出，访问日志
显示为空并提示。相对路径（如 `./access.log`）按面板进程 cwd 解析；systemd
单元设置 `WorkingDirectory=/etc/xray-panel`，Xray 子进程继承同一目录。
`none` 视为未配置。tail 只读取文件末尾约 1 MiB。

### H3 日志弹窗未做浏览器端到端验证 —— 未验证
2026-10-31 的后端接口已用真实进程 + curl 登录验证（面板/error/access 三种来源），
前端 `npm run build` 通过；尚未用无头浏览器实际打开弹窗检查下拉切换和长行换行。

## I. 认证与登录日志（2026-10-31）

### I1 兼容登录入口原先没有速率限制 —— 已修复
主 UI 使用 `/login`（`compat::login`），此前只有 `/api/login` 走每 IP 限流。
现在两个入口共用同一限流器（60 秒 5 次失败），未知用户名也执行等时 Argon2
校验，避免通过响应时间枚举用户名。登录成功/失败/限流/登出/改密都会写入
面板日志（内存缓冲 + journald）。验证：连续 5 次错误后第 6 次返回
“too many login attempts”。

## J. Xray 设置 / 入站操作核对（2026-11-01）

### J1 已修复：连接限制、统计开关、metrics 不生效 —— 已修复（2026-11-01）
`/panel/api/xray/update` 之前只白名单保存部分块，`policy`/`metrics` 被静默丢弃；
`ensure_policy` 强制覆盖统计开关；默认配置接口方法不匹配且返回当前配置。
现在整份保存可编辑模板，只忽略 `inbounds`/`api`/`stats`；`policy` 只强制
`statsUserOnline`，显式连接限制与统计开关保留；重置默认返回真正的默认模板。
验证见 `memory/CURRENT-STATUS.md` 2.7（API + 无头浏览器）。

### J2 负载均衡实时操作 —— 已实现（2026-11-02）
`balancerStatus` / `balancerOverride` 经 RoutingService gRPC 实现，真实 Xray
已验证；override 不跨 Xray 重启持久化。路由测试入口按用户要求移除，
`routeTest` 不再提供。跟踪：`TODO.md` T12（已完成）。

### J3 已修复：入站导入/批量链接/元数据丢失 —— 已修复（2026-11-01）
`inbounds/import` 未注册、`allLinks` 固定空数组、shareAddr/subSortIndex/
excludeFromSub/disableFlow 不落库。现在已实现并验证；`disableFlow` 同时影响
运行时客户端和分享链接；共享客户端导入保留第一次导入的流量计数。
`allLinks` 覆盖 vless/vmess/trojan/shadowsocks；WireGuard/AmneziaWG 入站已移除。
订阅服务器仍未实现，`subSortIndex`/`excludeFromSub` 目前只保存不参与服务端输出。
2026-11-03 起入站恢复 WireGuard/Hysteria/TUN/Tunnel/MASQUE（见 M1）；后端
`allLinks`/`subLinks` 的分享链接生成仍只覆盖 vless/vmess/trojan/shadowsocks，
新协议由前端列表页生成链接，服务端订阅链接为空。

### J4 模板内 inbounds 不生效 —— 确认（设计）
数据库入站是唯一来源，`build_config` 会移除模板 `inbounds`；高级编辑器不再
展示该视图。需要 Xray 原生额外入站时必须改后端设计。

## K. 客户端导入/导出（2026-11-01）

### K1 无入站关联的客户端不支持导入 —— 设计限制
- 现状：导出会把 `inboundIds` 为空的客户端列出，但 `clients.inbound_id` 非空且
  有外键，导入时该项会以 `at least one inbound is required` 跳过。
- 影响：无法从 3x-ui 迁移“孤立客户端”；不会产生假数据或部分写入。
- 后续：若需要支持，需要迁移 clients 表允许无主记录并调整列表/删除/流量语义。

### K2 导入/导出未在真实 Xray 上验证 —— 未验证
- 现状：后端逻辑已在 Windows debug + fake xray + 临时 DB 中做完整往返验证
  （`tmp/e2e-clients-io/e2e.py`），但未在真实 Linux + 真实 Xray 上确认
  导入后的配置生成、连接与流量继续累计。
- 影响：真实环境行为未证明；见 `TODO.md` T14。

## L. 登录失败通知（2026-11-01）

### L1 未在真实企业微信接口上验证 —— 未验证
- 现状：本地 HTTP mock 验证了请求体、账户、真实 IP、成功登录不发送等行为；
  未用真实企业微信群机器人 URL 验证 HTTPS、证书链和接口返回。
- 影响：如企业微信侧要求特定格式或证书，可能只留下面板 WARN 日志而不出现通知；
  不影响登录本身（异步发送）。
- 后续：见 `TODO.md` T15。

### L2 真实 IP 依赖反向代理请求头 —— 设计限制
- 面板优先信任 `X-Real-IP`、`CF-Connecting-IP`、`X-Forwarded-For` 首项；
  直接暴露面板时请求方可伪造这些头，伪造值只会出现在通知中，不参与限流或授权。

## M. 入站/出站协议补全（2026-11-03）

### M1 真实 Xray 配置校验未执行 —— 未验证
- 现状：后端用 fake xray（`run -test` 恒成功）完成 11 种入站协议的添加、
  客户端建号和配置生成端到端；无头 Chrome 验证了下拉、表单渲染和 9 种无证书
  协议的真实前端保存。配置字段按 Xray-core main 源码（wireguard、hysteria、
  masque、tun、dokodemo/tunnel、shadowsocks）逐项核对。
- 未验证：官方 Xray 26.10.31 的 `xray run -test` 与真实连接/流量。原因是本机
  没有 Xray 二进制，下载并执行外部二进制需要用户显式授权。
- 影响：配置结构错误或 Xray 版本差异只能在真实环境暴露；hysteria/masque 还
  需要有效的 TLS 证书文件（missing file 会让 `-test` 失败）。
- 后续：在装有官方 Xray 的 Linux/Windows 环境逐个协议执行 `-test` 和连通测试。

### M2 sidecar 协议不提供 —— 设计限制
- `mtproto`、`tuic`、`amneziawg` 入站与 `amneziawg` 出站依赖 3x-ui 随包的
  mtg-multi / tuic-server / amneziawg-go，本面板不内置，故不下拉、后端明确
  返回 `unsupported protocol`。
- 影响：从 3x-ui 迁移这些入站会被拒绝；其余 11 种原生协议可正常迁移。

### M3 新协议的服务端订阅链接为空 —— 已知
- `src/xray/link.rs` 仅生成 vless/vmess/trojan/shadowsocks 链接；
  `subLinks` 对 wireguard/hysteria/masque 静默跳过。列表页/二维码的前端
  链接生成不受影响。后续可按前端 `inbound-link.ts` 的格式补 hysteria2 与
  wireguard 链接。

## N. GitHub 发布与面板在线更新（2026-11-03）

### N1 在线更新依赖 GitHub 可达与 systemd —— 设计限制
- 面板通过 GitHub API/Release 下载新二进制；无法访问 GitHub 时需在设置页配置
  `updateProxy`（HTTP/SOCKS5），或离线替换二进制。
- 自动重启依赖 systemd `Restart=on-failure`（安装脚本/示例单元已配置）。非
  systemd 启动时更新接口返回 `restartRequired:true`，需要手动重启。
- 仅支持 Linux；Windows/macOS 返回明确错误。

### N2 真实 systemd 自动重启仍未验证 —— 部分验证
- 已验证：GitHub Actions `integration` 在 Linux 上跑通
  v0.2.17 → v0.2.18 的检查更新、tar.gz 下载、资产 digest 校验、解压替换，
  重启后版本变为 0.2.18；坏代理返回带代理地址的错误；一键安装脚本在 Linux
  用 systemctl stub 安装 0.2.18 成功。
- 未验证：真实 systemd 服务下 `PANEL_SYSTEMD=1`/`MainPID` 判定和
  `Restart=on-failure` 自动拉起。影响：若单元环境或权限不同，更新可能需要
  手动执行 `systemctl restart xray-panel`。

### N3 v0.2.16 → v0.2.17 需要一次手动升级 —— 兼容性限制
- v0.2.16 在线更新查找 Release 中的原始 `xray-panel-rs` 资产；v0.2.17 起 Release
  只保留 tar.gz，因此 0.2.16 面板无法自动更新到 0.2.17。
- 解决：在 0.2.16 机器上执行一次一键安装脚本（会保留 DB/配置），或手动替换
  二进制；升级到 0.2.17 后在线更新恢复正常。

### N4 一键脚本代理与面板设置相互独立 —— 设计限制
- `install.sh` 的 `PANEL_PROXY` 只用于脚本下载，不会写入面板的 `updateProxy`；
  安装完成后需要在「设置 → 更新代理」再配置一次，面板在线更新才会走代理。
- 代理不可达时脚本和面板都会返回包含代理地址的错误；两处错误文案/退出行为
  已由 `integration.yml` 覆盖。

### N5 安装脚本的“已最新”跳过依赖版本声明或哈希 —— 兼容性限制
- 0.2.18 及更早的二进制没有 `LUMEN_VERSION` marker，无法直接读取版本；脚本会
  下载 Release tar.gz，并按包内二进制 SHA-256 与已安装文件比对后跳过。该路径
  仍需下载约 5.4 MB，但不会停止服务或覆盖二进制。
- 本地自行编译并替换的二进制与 Release 内容不同，会进入正常安装路径。
- 跳过路径不会重写 systemd 单元；需要更新单元或修复安装时使用 `PANEL_FORCE=1`。
- 未在真实 Linux 上验证（本地使用桩环境）。

### N6 代理可达但目标不可达时的提示可能归因于代理 —— 未验证
- `network_hint` 在配置了更新代理时，把传输层失败归因为代理不可达；若代理
  本身正常而目标 DNS/连接失败，实际原因可能不同。错误链仍保留原始 URL 与
  系统错误，便于进一步排查。

### N7 非网络类错误文案 —— 2026-11-04 已基本中文化
- 0.2.20 起，代理/传输失败只返回中文提示，原生 `io`/`ureq` 错误仅写入日志。
- 2026-11-04 源码改动起，`fail()`/`AppError`/`bail!`/`context` 中本面板产生的
  面向用户文案统一为中文（含 GitHub 返回非 200、Release 缺资产、JSON 解析、
  证书/文件错误）。
- 保留英文的只有 Xray 原生 stdout/stderr、系统/第三方错误原文（位于链尾或
  仅写日志），这是有意为之。英文界面下后端错误仍显示中文（H4）。

### H4 后端错误统一中文，英文界面下不切换语言 —— 设计限制
- 2026-11-04 起后端面向用户的错误文案统一为中文；前端切换英文后，登录/设置/
  入站/客户端等后端错误仍显示中文。
- 原因：避免为 14 个 locale 维护后端错误资源包；当前用户以中文运维为主。
- 影响：纯英文用户会看到中英混排；Xray 原生输出仍可能是英文。

### H5 在线 IP 与位置查询的限制 —— 未验证/第三方依赖
- Xray 的 OnlineMap 会丢弃回环地址（`127.0.0.1`、`::1`），本机或同主机回环
  转发来的客户端即使在线也不会显示 IP（真实公网客户端不受影响）。
- 在线 IP 依赖 Xray 26+ 的 `GetUsersStats`；旧核心或未启用
  `statsUserOnline` 时 `onlineIps` 返回空对象，客户端弹窗只显示在线状态。
- 位置查询由面板服务器优先访问 zxinc（纯真库），失败回退 ipwho.is；面板
  无外网时按钮返回中文错误。默认不查询，只有点击“位置”才产生外部请求。
- zxinc 响应通常需要 1–5 秒，比 ipwho.is 慢；其数据不含时区/经纬度，
  返回的 `timezone` 为空字符串、`latitude`/`longitude` 为 `null`。zxinc
  不可用时的 ipwho.is 回退对国内 IP 的省/市可能不准（例如杭州 IP 被定位到
  南京）。
- `ipLocation` 只接受 IP 字面量，不接受域名；不查询也不会缓存结果。
- IP数据云为可选功能，需要自己的 `ipDataCloudKey`（官网注册，有免费额度）。
  未配置时只查纯真；配置后并行查询，失败只在第二行显示错误，不影响纯真结果。
  Key 以明文保存在 `settings.x3ui_settings` 并由 `setting/all` 返回给单个管理员，
  不额外加密。
- IP数据云的响应解析按官方文档实现（兼容 `data` 与 `data.location` 两种层级），
  已用假 Key 验证真实接口的错误返回（`key不正确`）；**尚未用有效 Key 验证
  成功响应**，街道/区县精度和免费额度以账号套餐为准。
- 未在真实 Linux/musl 与真实公网客户端上验证（Windows + 局域网源 IP 已验证）。
