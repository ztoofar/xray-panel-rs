# xray-panel-rs — 交付说明（2026-10-31）

状态：**可运行 MVP 已完成并验证**，位于 `D:\ClaudeCode\Projects\xray-panel-rs`。

## 交付范围

保留：
- 单机面板、管理员登录（Argon2id；默认 admin/admin，可在面板内修改）、会话 Cookie、CSRF、登录限流
- 入站 CRUD：VMess / VLESS / Trojan / Shadowsocks / HTTP / Mixed
- 客户端 CRUD、凭据自动生成、流量/到期限制、启用/停用、流量重置
- Xray 配置生成、`xray run -test` 校验、子进程监管与自动重启
- Xray gRPC StatsService 流量统计、周期重置（时/天/周/月）
- VLESS / VMess / Trojan / Shadowsocks 分享链接
- 面板设置、Xray 额外配置（log/dns/outbounds/routing）编辑
- 前端嵌入二进制；监听地址可手动设置（`PANEL_LISTEN`）

明确去掉：
- 多节点、Bot/通知、订阅服务器
- MTProto、AmneziaWG、TUIC、MASQUE、WARP/Nord/PIA
- 广告/赞助、API 文档
- SSL 证书/强制 HTTPS（面板只跑 HTTP）
- IP 限制/在线 IP 统计

## 架构

- 后端：Rust 2021、Axum 0.8、Tokio、SQLx（SQLite）、tonic + prost
- 前端：React 19、Vite 8、TypeScript、Ant Design 6
- 关键文件：
  - `src/main.rs`：启动、种子管理员、优雅退出、维护任务
  - `src/db/`：全新 schema（`migrations/0001_init.sql`）与读写层
  - `src/xray/config.rs`：配置生成（API inbound、stats/policy/routing 合并）
  - `src/xray/supervisor.rs`：Xray 子进程、配置测试/原子写入、日志环形缓冲
  - `src/xray/stats.rs`：常驻 gRPC 统计客户端
  - `src/jobs.rs`：流量增量落库、配额/到期停用、周期重置
  - `src/api/`：登录、仪表盘、入站、客户端、设置、Xray 管理
  - `frontend/src/pages/`：登录、仪表盘、入站、客户端、Xray、设置

## 验证结果

- `cargo test`：8 个单测全部通过（配置生成、分享链接、流量增量/解析）
- `cargo clippy --all-targets -- -D warnings`：零警告
- `npm run build`：前端构建成功（约 398 KB gzip）
- `cargo build --release`：成功，Windows 二进制约 4.7 MB
- Linux amd64 交叉编译：`cargo zigbuild --release --target x86_64-unknown-linux-musl`
  成功，产物为静态 ELF（无动态依赖），约 5.2 MB，前端已嵌入
- 端到端（fake xray 验证配置测试/进程启动）：
  - 登录 → 会话 → CSRF 校验
  - 创建 VLESS 入站 → 生成 `config.json` → 启动子进程
  - 创建客户端（自动生成 UUID）→ 配置注入 → 分享链接正确
  - 静态资源、SPA 回退、API 404 JSON 正常
- 内存实测（Windows，release，空闲 + 30 次请求）：
  - 面板约 **9.3 MB 工作集 / 2.5 MB 私有内存**
  - 真实 Xray 进程内存另计

## 已生成的 Linux 交付包

- `dist/xray-panel-rs-linux-amd64.tar.gz`（约 2.3 MB）
- 解压后目录：`dist/xray-panel-rs-linux-amd64/`
  - `xray-panel-rs`：ELF 64-bit x86-64，静态链接（musl），已 strip
  - `xray-panel.service`、`xray-panel.env.example`、`README.md`、`SHA256SUMS`
- SHA256：`1520f5b0457f37cacfad88b54f1ec3b1733295a2644babf4596dcab384bc2849`
- 默认：监听 `0.0.0.0:2053`，首次账号 `admin/admin`（登录后立即修改）

## 构建与运行

```sh
cd frontend && npm ci && npm run build && cd ..
cargo build --release
PANEL_DB=/etc/xray-panel/panel.db \
XRAY_BIN=/usr/local/bin/xray \
XRAY_CONFIG=/usr/local/etc/xray/config.json \
PANEL_LISTEN=0.0.0.0:2053 \
./target/release/xray-panel-rs
```

完整安装说明见 `README.md`（无安装脚本，手动安装）。

## 已知限制

- 尚未实现 WireGuard / Hysteria 入站（核心 6 种协议可用）
- 客户端/入站变更通过重建配置并重启 Xray 生效，会短暂中断连接
- 传统 Shadowsocks 加密仅支持单客户端（Xray 限制）
- 单管理员账号
- 不做 SSL 证书、IP 限制
- 面板异常崩溃后 Xray 子进程可能残留（正常停止/systemd 停止会一并结束）

## 清理清单（用户手动执行）

开发工具（本项目为编译验证而安装）：
- `C:\Users\29789\.rustup`
- `C:\Users\29789\.cargo`（含 crates 缓存）
- `D:\ClaudeCode\Tools\mingw64`
- `D:\ClaudeCode\Tools\node-v22.23.4-win-x64`
- `D:\ClaudeCode\Tools\winlibs.zip`
- `D:\ClaudeCode\Tools\rustup-init.exe`
- `D:\ClaudeCode\Tools\node-v22.23.4-win-x64.zip`
- `D:\ClaudeCode\Tools\zig-npm`（Zig 0.17 的 npm 二进制与标准库，交叉编译用）
- `D:\ClaudeCode\Tools\zig.zip`（未完成的官方下载，可删）
- `D:\ClaudeCode\Tools\zig-index.json`、`cz.json`
- `C:\Users\29789\.cargo\bin\cargo-zigbuild.exe`
- npm 缓存：`C:\Users\29789\AppData\Local\npm-cache`
- 若 rustup 改过用户 PATH，移除 `%USERPROFILE%\.cargo\bin`
- winget 未成功安装任何包

项目构建产物（可删，不影响源码）：
- `D:\ClaudeCode\Projects\xray-panel-rs\target`
- `D:\ClaudeCode\Projects\xray-panel-rs\frontend\node_modules`

## 2026-10-31 第二阶段暂停点（3x-ui 前端复用）

已完成：
- 前端已替换为 3x-ui 的 React 源码：`frontend/`；旧的 Rust 自研前端保留在 `frontend-custom/`
- 已复制 14 个语言包到 `internal/web/translation/`
- Vite 输出改为 `web/dist`；用 Node 26 构建成功（`index.html` + `login.html` + assets）
- 裁剪了 `panelRoutes` 中的节点 / API 文档 / 赞助商路由；删除订阅页构建入口
- Rust 新增 `src/api/compat.rs`（3x-ui 响应包 `{success,msg,obj}`）：
  - `/csrf-token`、`/panel/csrf-token`、`/login`、`/logout`、`/getTwoFactorEnable`
  - `setting/all|update|defaultSettings|factoryDefaults|getDefaultJsonConfig`
  - `server/status|getConfigJson|getXrayVersion|getPanelUpdateInfo|xrayMetricsState`
  - `inbounds/list|list/slim|get|options|add|update|del|setEnable`
  - `clients/list|list/paged|onlines|onlinesByGuid|activeInbounds|lastOnline|groups`
  - `hosts/list`、`xray/getOutboundsTraffic|getXrayResult|getDefaultJsonConfig`
  - `/panel/api/{*rest}` 空实现兜底
- `src/web/mod.rs`：`/login` → `login.html`，`/panel/*` → `index.html`，注入版本；
  `/` 按登录状态跳转；`/assets/*` 提供嵌入资源
- `cargo check` 通过（仅 1 个 dead_code 警告）

下次继续：
1. 启动面板实测 3x-ui 登录 → `/panel` → 仪表盘/入站/客户端，按 Zod 报错修响应形状
2. 客户端 CRUD、批量操作、分组、IP/HWID、外部链接等接口
3. 入站全协议表单（WireGuard/Hysteria/MTProto/AmneziaWG/TUIC/MASQUE 等）与统计
4. 设置页全量（安全/2FA/API Token/邮件/证书）、Xray 页全部 schema（basics/outbounds/routing/balancers/dns/advanced）
5. 裁剪 SettingsPage 的 Telegram/Discord/订阅 Tab、LoginPage 的 SponsorSlot
6. 删除 `frontend-custom/`，重新交叉编译 Linux 二进制并打包
7. 验证 `setting/update` 表单解析与 `xrayTemplateConfig` 保存/apply

环境：Node 26 位于 `D:\ClaudeCode\Tools\node-v26.13.0-win-x64`；Zig 交叉编译环境可用。
状态：**已暂停，未关机**。

### 热修复：config.json.tmp 扩展名
- 原因：Xray 靠扩展名判断配置格式，`.tmp` 导致 `Failed to get format`
- 修复：`xray/supervisor.rs::temp_config_path()` 生成 `config.json.tmp.json`；
  `service.rs` 测试文件为 `config.json.test.json`；新增单元测试，`cargo test` 9/9 通过
- 已打包旧自研界面的热修复二进制：
  `dist/xray-panel-rs-linux-amd64-hotfix.tar.gz`
  SHA256 `fcac78f88bb6838ce86c932aa946ba649d1e241820204cdfe4bb1841b688cead`
- 3x-ui 前端工作区已恢复（`web/mod.rs` 与 `web/dist` 回到 3x-ui 版本）

## v1 最终交付（2026-10-31，自研界面）

当前生效前端是 `frontend/`（自研 React）；3x-ui 前端复用方案暂停，源码在
`frontend-3xui/`、构建产物在 `web/dist-3xui/`，后续可继续。

已完成并验证：
- 入站/出站/客户端全部表单化：协议、传输、TLS/REALITY、嗅探、多服务器、多用户，
  可动态增删，不需要手写 JSON
- 客户端 UUID/密码/账号一键生成；一个客户端可关联多个入站（多对多
  `client_inbounds` 表）
- 在线客户端：`statsUserOnline` + `GetUsersStats`，每入站显示在线数量、悬浮详情；
  客户端列表在线标绿并置顶
- 出站流量统计；DNS / 路由 / 日志结构化配置
- SQLite `.db` 备份/恢复（`VACUUM INTO` + 事务恢复）
- `config.json.tmp` 扩展名导致 Xray 格式识别失败的问题已修复
- 测试：`cargo test` 9/9，`cargo clippy -D warnings` 零警告；前端构建通过
- 端到端：内嵌客户端自动生成、多入站关联/解除、出站/DNS/路由、备份恢复

发布包：
- `dist/xray-panel-rs-linux-amd64-v1.tar.gz`（约 2.5 MB）
- 静态 ELF x86-64（musl），SHA256
  `c7b11efaa4568cd82c70d35a7804014e6cac35cda1c9865e346106f686c6e1e4`
- 默认 `0.0.0.0:2053`，首次 `admin/admin`

注意：
- 在线状态需要 Xray 26+ 的 `statsUserOnline` / `GetUsersStats`；旧核心下为空
- `.db` 恢复要求备份 schema 与当前版本一致
- 尚未实现：SSL 证书、IP 连接数限制、WireGuard/Hysteria 等额外协议、
  3x-ui 全量设置页（2FA/邮件/API Token/证书/升级）等
- 机器未关机（如需要请明确告知）

## 3x-ui 前端已启用（2026-10-31 晚）

- 当前生效前端：`frontend/`（裁剪后的 3x-ui React 源码，已移除节点/API 文档/
  赞助商路由与订阅页入口）；自研前端保留在 `frontend-custom/`、`web/dist-custom/`
- 入站/出站页面直接使用 3x-ui 原表单，协议子项、传输/安全、fallback、嗅探完整
- Rust 兼容层已实现：`/login`、`/logout`、`/csrf-token`、登录页 GET、`setting/all|update`、
  `server/status`、`inbounds` 列表/新增/更新/删除/启停、`clients` 列表/分页/新增/更新/
  删除、多入站关联、在线状态、`.db` 备份恢复、`getNewUUID`/`getNewX25519Cert`
- 未知 Xray 原生协议（hysteria/wireguard 等）采用配置透传，settings 原样写入
- 新增原始配置编辑器：侧边栏“原始配置”或 `/raw-config`；保存前 JSON 校验、
  `xray run -test`、备份 `.bak`、重启 Xray；再次保存面板项会覆盖手动配置
- 未实现的写接口统一返回“not supported”，不再假成功
- 已验证：3x-ui 表单登录、GET `/login`、面板 HTML、VLESS/Hysteria 入站新增、
  客户端增删改（响应含 inboundIds）、原始配置 GET/PUT；`cargo test` 9/9、Clippy 零警告

测试包：
- `dist/xray-panel-rs-linux-amd64-3xui.tar.gz`（4.2 MB）
- SHA256 `73208593c4fe3460cc5dc5ae288b66e7073149f95f298eb55d0ce8334f3e50b6`
- 默认 `0.0.0.0:2053`、`admin/admin`

仍待用户实测/补齐：
- 真实浏览器过一遍 3x-ui 入站/出站/客户端页面，按 Zod/Network 报错继续补接口
- 未实现的后端功能：sidecar 协议（MTProto/AmneziaWG/TUIC/MASQUE）、WARP/Nord/PIA、
  2FA/邮件/API Token/证书/面板升级、客户端批量/分组/HWID/IP 限制
- 3x-ui 某些页面请求的接口仍走空实现；写操作已改为明确报错

## 备注

- 构建完成后曾安排自动关机，用户随后要求取消，已成功取消（`shutdown /a`）。
- 源码、前端构建产物、release 二进制均已落盘。

## v2 自研前端字段补齐（2026-10-31 晚）

用户明确：**自研面板的入站/出站/客户端/路由/Xray 配置要和 x-ui 一样，不是做兼容层**。
当前生效前端为 `frontend/`（自研 React + AntD），`frontend-3xui/`、`web/dist-3xui/`
仅保留参考，不参与构建。

本次补齐：
- 入站表单：
  - 传输：tcp/ws/grpc/httpupgrade/xhttp/kcp/quic/http；tcp header、ws heartbeat、
    grpc multiMode、xhttp mode、kcp 全套、quic、http/2
  - 安全：none/TLS/REALITY；TLS 证书文件/内容、SNI、ALPN、fingerprint、
    allowInsecure；REALITY target/serverNames/shortIds/privateKey/publicKey/
    spiderX，并新增 `/api/xray/new-x25519` 一键生成密钥对
  - sockopt：tcpFastOpen/tcpNoDelay/tcpKeepAlive/keepAliveIdle/Interval/
    tcpCongestion/tcpMptcp/acceptProxyProtocol/domainStrategy/dialerProxy/
    mark/interface/tproxy
  - 协议：VLESS/VMess/Trojan/Shadowsocks/HTTP/Mixed/WireGuard；VLESS/Trojan
    fallbacks；VMess alterId/security；SS network/ivCheck；WireGuard
    secretKey/peers/MTU/DNS/subnet/workers/noKernelTun
  - 嗅探：enabled/destOverride/domainsExcluded/metadataOnly/routeOnly
  - 入站内客户端：UUID/密码/账号、flow、alterId/security、限额、到期、
    IP/HWID、按天重置、备注、启用
- 出站：新增 wireguard、dns；freedom/blackhole/socks/http/shadowsocks/
  vmess/vless/trojan/wireguard/dns；多服务器/多用户；传输/安全/
  REALITY；mux；sockopt
- 客户端：备注、分组、IP 限制、HWID 字段、按天重置、批量启用/停用/重置/删除；
  多入站、在线标绿置顶保持
- 路由：sourcePort/localPort/sourceIP/localIP/user/vlessRoute/process/
  localOS/attrs(JSON 对象)/domainMatcher/ruleTag/balancerTag；balancer
  管理（策略按 Xray 的 `{type}` 对象写入）
- Xray：DNS/log/outbounds/routing 结构化，原始配置编辑器保留并增加覆盖提示
- 数据库迁移 `0004_client_extras.sql`：comment/group_name/limit_ip/
  limit_hwid/reset_days/last_reset_time
- 后端：VMess alterId/security 注入；客户端独立按天重置；在线 IP 超限停用；
  WireGuard 协议透传
- 修复 `src/web/mod.rs` 重复 `let path`

验证：
- `cargo test` 9/9；`cargo clippy --all-targets -- -D warnings` 零警告
- 前端 `tsc --noEmit && vite build` 通过
- fake Xray 端到端：VLESS+ws+tls+fallback+sockopt 生成正确；客户端扩展字段
  写入/更新不丢失；路由 balancer 写入；WireGuard/DNS 出站写入；
  `/api/xray/new-x25519` 返回密钥
- 新的 Linux 静态包：`dist/xray-panel-rs-linux-amd64-v2.tar.gz`

仍与 3x-ui 有差距（未声称 100% 相同）：
- 3x-ui sidecar/集成协议（Hysteria/MTProto/AmneziaWG/TUIC/MASQUE、WARP/Nord/PIA）
  按早期范围未做
- TLS ECH/证书管理、finalmask、xhttp 完整 xmux/extra、Xray observatory 等
  极端高级字段尚未逐项结构化；可用 `/raw-config` 兜底
- `limitHwid` 仅保存/展示，未强制

## 最终版：3x-ui 五页面已复制并接入（2026-10-31 深夜）

用户要求“入站/出站/客户端/路由/Xray 配置抄过来”，已执行：

- 生效前端改为 `frontend-3xui/` 构建产物 `web/dist/`；只保留六个路由：
  `/inbounds`、`/clients`、`/outbound`、`/routing`、`/xray`、`/raw-config`
- 根路径和 `/login` 提供 3x-ui 登录页；登录后进入 `/panel/inbounds`
- Rust 静态服务：`/panel/*` → SPA，`/raw-config` → 原始配置编辑器，
  `/settings` → 面板设置页（改管理员账号、`.db` 备份/恢复、原始配置入口）
- 3x-ui 页面需要的兼容 API 已补齐核心部分：
  - `POST /panel/api/xray/` 返回完整配置 payload；`POST /xray/update` 保存
    log/dns/outbounds/routing/observatory 等并重启
  - `/xray/resetOutboundsTraffic`、`/xray/testOutbounds`（测试暂返回失败）、
    `/nodes/list` 空列表
  - 入站 fallbacks GET/POST、allLinks、resetTraffic、delAllClients、bulkDel、
    resetAllTraffics
  - 客户端 get/resetTraffic/resetAllTraffics/bulkEnable/bulkDisable/bulkDel/
    bulkAdjust/bulkAttach/bulkDetach、attach/detach、groups bulkAdd/bulkRemove、
    HWID/IP/subLinks 空实现
  - `client_record` 现在返回真实的 limitIp/limitHwid/comment/group/reset
- 前端禁用 WebSocket（Rust 后端未实现实时推送），避免反复重连报错
- 移除登录页赞助位、侧边栏捐赠/文档链接和 GitHub 版本徽章
- 无头 Chrome 验证：
  - 登录 admin/admin 成功，跳转 `/panel/inbounds`
  - 入站/客户端/出站/路由/Xray 页面均渲染；入站表单弹窗可打开并成功创建入站
  - 原始配置 iframe 正常加载并显示 config.json
  - `/settings` 正常显示
- `cargo test` 9/9、`cargo clippy --all-targets -- -D warnings` 零警告

最终包：
- `dist/xray-panel-rs-linux-amd64-3xui-final.tar.gz`（4.1 MB）
- SHA256 `0e115dccae2893113dd7b115b8f1053a8e069f0e6b81a519bc0a879e3afb0bb1`
- 二进制 SHA256 `6331c502d3f0ed7c1b10f49b91e68f719379333009914b06e687b9e784581910`

仍缺：3x-ui 的节点/订阅/2FA/邮件/HWID 日志/出站订阅/部分测试扫描接口；
这些在本次范围外，相关按钮会报错。自研前端源码保留在 `frontend/`，
构建产物保留在 `web/dist-custom-v2/`。

## 修复：客户端添加入口与 VLESS 密钥生成（2026-10-31）

1. 单个添加客户端：`POST /panel/api/clients/add` 之前按平铺字段解析，而
   3x-ui 前端发的是 `{"client": {...}, "inboundIds": [...]}`，导致
   `email is required`。现先展开 `client` 对象再复用同一套创建逻辑。
2. 批量添加客户端：`/panel/api/clients/bulkCreate` 未注册，落到兜底
   stub，返回 `this feature is not supported by the Rust panel yet`。
   现实现该接口（JSON 数组，逐条创建，重复邮箱进 `skipped`，返回
   `{created, skipped}`）。
3. VLESS 生成密钥：`/panel/api/server/getNewVlessEnc` 原为 unsupported。
   现执行 `xray vlessenc`，解析 `Authentication:` 块，返回 x25519 /
   mlkem768 及 `_xorpub` / `_random` 共 6 个 auth（wire mode 由
   `.native.` 替换派生），字段为 `id/label/decryption/encryption`。

验证：
- 新增测试 `vless_enc_parses_auth_blocks_and_derives_wire_modes`
- `cargo test` 11/11；clippy 零警告
- fake xray 端到端：`/panel/api/clients/add` 创建成功；`bulkCreate`
  返回 `created:1` 且重复邮箱被跳过；`getNewVlessEnc` 返回 6 个 auth

新包：
- `dist/xray-panel-rs-linux-amd64-3xui-final3.tar.gz`
- SHA256 `c4203884e9c7f035526280db01e984eef17bd5048fa38e10eb2b8fcfcfd3b0a9`
- 二进制 SHA256 `a666d9eea52dd1a658c7079e94087aebce2cdf5ee8371b968021603e6ec8a3a7`

## Lumen 品牌、系统状态、面板设置与出站测试（2026-10-31）

品牌与主题：
- 面板更名为 **Lumen / 流明**；侧栏、登录页、命令面板、页面标题、PWA
  manifest、i18n 文案统一替换，保留 `xray-panel-rs` 作为程序/包名
- 主题色改为青蓝 `#0EA5E9`（hover `#38BDF8`、active `#0284C7`），
  全局圆角 10/14px，默认浅色，保留深色/极暗主题

新增页面：
- 系统状态 `/panel`：CPU/内存/交换/磁盘/网络/连接数/运行时间/Xray 状态，
  操作栏支持重启/停止 Xray、日志、配置、备份恢复、系统历史、Xray 指标
- 面板设置 `/panel/settings`：仅保留常规与安全两类；安全页保留管理员
  账号/密码修改（`/panel/api/setting/updateUser`，成功后清会话并登出）；
  去掉 2FA、API Token、Telegram/邮件/Discord/订阅设置
- 客户端表单精简为六项：邮箱、关联入站、流量上限、UUID、过期时间、启用；
  编辑时未触碰的隐藏字段保持原值（过期时间为负的延迟启动也不会被清掉）

出站测试：
- `/panel/api/xray/testOutbounds` 不再是失败桩
- `tcp` 模式并行直连出站 endpoint（vnext/servers/peers），返回每个地址的延迟
- `http` / `real` 模式启动临时 Xray（本地 SOCKS 入站 + 路由到被测出站），
  通过 SOCKS5 实际建立到测试目标的连接并计时；临时配置 0600、结束即删
- 修复 `stopXrayService` 之后 supervisor 退出、无法再重启的问题

验证：
- `cargo test` 15/15；clippy 零警告
- 无头 Chrome 实测：系统状态、面板设置、客户端弹窗均正常渲染（浅色）
- API 实测：stop → restart 成功；updateUser 校验旧密码；批量/单条创建客户端正常

新包：
- `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`
- SHA256 `63843431a5a33ed590307e4272283ef37e4aa4af6f4b5968c6801f2e3f62714d`
- 二进制 SHA256 `45e94e0894102d12702fbdb02bb7e83c9f7c3b53330493db0440fabe9567cd77`

## 收尾调整：原始配置/日志/公网 IP/指标入口（2026-10-31）

- 移除原始配置：删除前端路由与侧栏项、后端 `/raw-config` 页面与
  `/api/xray/raw-config` 接口、自定义设置页中的入口
- 系统状态：重启/停止 Xray 现在有成功/失败 toast（14 个语言新增
  `pages.index.stopXraySuccess` / `restartXraySuccess`）
- 日志：实现 `/panel/api/server/logs/{rows}`，返回 Xray 子进程日志缓冲的
  最后 N 行，并在 supervisor 启动时写入 `[panel] xray started`；
  日志导出文件名改为 `lumen.log`，行前缀改为 `Lumen:`
- 公网 IP：`/panel/api/server/status` 新增 `publicIP`，后台每 10 分钟通过
  ipv4/ipv6.icanhazip.com 获取并缓存，状态轮询不会被阻塞
- 移除 Xray 指标按钮与弹窗（`XrayMetricsModal` 不再挂载）
- 验证：`cargo test` 15/15，clippy 零警告；无头 Chrome 实测重启 toast、
  日志弹窗、IP 显示、侧栏无「原始配置」、无「Xray 指标」
- 重新打包 `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`
- SHA256 `8338dd5b9c882cd17ce7a7afe77420909ff1ff23b6ac0f5a6ba9b1d70016b91e`
- 二进制 SHA256 `360bd653ccf32bfbe0ab25b961baa27c9c8e70a3b658d1f6c2e1af58e7770320`

## 客户端流量/日期限制核查与修复（2026-10-31）

核查结论：
- 流量限制已实现：`jobs` 每 10 秒（默认，可配 5–300 秒）读取 Xray Stats，
  累计 up/down；`total > 0 && used >= total`、客户端的 `limitIp` 超限、
  或入站级流量/到期，都会把对应对象 `enable=false`，随后重新生成配置并
  重启 Xray，使其立即断流
- 客户端按天重置（reset_days）也会在后台执行

修复：
- 日期限制此前不生效：前端和兼容 API 写入的 `expiryTime` 是**毫秒**，
  而 `jobs` 拿它和**秒**级 now 比较，永远不会过期。现统一按毫秒比较，
  并兼容旧数据里的秒值（`expired_at`）
- 批量调整客户端的“加天数”把秒加进了毫秒字段，已改为 `addDays * 86_400_000`
- 新增测试 `expiry_handles_milliseconds_and_legacy_seconds`

仍存在的行为（非 bug）：因到期/超量被自动禁用的客户端，重置流量后不会
自动重新启用，需要在客户端页手动打开开关；负过期时间（延迟启动）不参与
到期判定；`limitHwid` 仍只保存不强制。

验证：`cargo test` 16/16、clippy 零警告。
新包：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`
- SHA256 `df6f2b097542952efaf111fec2e284d524527c3dfb5eaa88474dfc246c5f2024`
- 二进制 SHA256 `d58e547f83a6cd05f2564d385329dce017c0c12cab003db6943d5b22556813fc`

## 系统状态指标修复（2026-10-31）

此前 swap/disk/appStats.mem/netIO/tcpCount/udpCount 都是硬编码 0。现改为
Linux `/proc` 实时采集：
- 交换空间：`/proc/meminfo` 的 SwapTotal/SwapFree
- 存储：`statvfs("/")` 的已用/总量（libc）
- 面板内存/线程：`/proc/self/status` 的 VmRSS、Threads
- 连接数：`/proc/net/tcp`、`tcp6`、`udp`、`udp6` 行数
- 整体速度：`/proc/net/dev` 各网卡（排除 lo）rx/tx 字节差 / 时间差，
  得到 bytes/s；间隔 <0.2s 时沿用上次速率
- 总发送/接收：系统网卡累计 rx/tx（不再错误地使用入站流量之和）
- 另补 cpuSpeedMhz / logicalPro

验证：
- 新增 5 个解析单测（meminfo/net_dev/status/socket 表/速率回绕）
- `cargo test` 21/21；clippy 零警告；Linux musl 交叉编译通过
- Windows 开发机无法运行 Linux `/proc` 代码，仅验证了 JSON 结构与解析逻辑；
  真实数值需在 Linux 上确认

新包：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`
- SHA256 `f4d70292616891247fda3a6637cb4bc0b8e2085ee93bf15dc4c4eeeffc01f4a9`
- 二进制 SHA256 `84706e7c42cc0af1319711179b0047a8fc6567ad0d5f08cb404e1ffe946f160a`

## 客户端二维码分享 / 信息精简 / 时间修复（2026-10-31）

- 二维码分享失败：`/panel/api/clients/subLinks/{subId}` 原为空桩。现按
  subId 找到客户端及其关联入站，用 `xray::link::client_link` 返回直连分享
  链接（vless/vmess/trojan/ss 等），二维码弹窗按链接逐个渲染
- subId：新建客户端若未提供会自动生成 16 位；迁移 `0005_client_sub_id.sql`
  为历史空 subId 客户端回填随机值，QR 弹窗不再提示 noSubId
- `GET /panel/api/clients/get/{email}` 改为 3x-ui 约定结构
  `{client, inboundIds, externalLinks, tunnelAllowedIPs}`，消除 Zod 校验警告
- 客户端信息弹窗精简为：在线/启用、邮箱、关联入站、流量/剩余、到期时间、
  UUID、创建时间、更新时间；其余订阅、IP/HWID、备注/分组、隧道配置等移除
- 创建/更新时间显示错误：数据库存秒，接口按毫秒返回（`* 1000`），与前端
  日期格式化一致

验证：
- `cargo test` 22/22；clippy 零警告；Linux musl 交叉编译通过
- 无头 Chrome：二维码弹窗渲染出 VLESS/REALITY 链接与二维码；信息弹窗
  显示正确创建/更新时间
- API 实测：迁移回填 subId、新建客户端自动生成 subId、subLinks 返回链接、
  client_get 结构符合 schema

新包：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`
- SHA256 `5b5906806f569c88185376b488eadbe9fc8d4a4da4582019b8fa6bc85dcdff80`
- 二进制 SHA256 `f74d7ce184cb3691d1c68be8f1dca328d0114fdb4a382c8e8933bfba3ef30455`

## 客户端列表分页修复（2026-10-31）

根因：客户端页从 `/panel/api/setting/defaultSettings` 读取 `pageSize`，
而该接口只返回 accessLogEnable/devChannelEnable。前端拿不到 pageSize 时
回退到 200（"一页显示"模式），看起来就像没有分页。

修复：
- `defaultSettings` 现在返回 `pageSize`（默认 25）、`expireDiff`、
  `trafficDiff`、`tgBotEnable`、`ipLimitEnable`、订阅相关字段等，优先取
  面板已保存设置
- `/clients/list/paged` 本身已支持 page/pageSize/search，无需改动

验证：
- `cargo test` 22/22；clippy 零警告
- API：33 个客户端，pageSize=25 时第 1 页 25 条、第 2 页 8 条
- 无头 Chrome：表格实际渲染 25 行，分页器显示 `33 / 1 / 2 / 25 条/页`

新包：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`
- SHA256 `142924f3c686e9bff6ab57b859c283a9ff94bd6852373ca4dcadce8b70d4fc0b`
- 二进制 SHA256 `bacccb5096f125092ab2d4cddb869eab5343f3d48684b3589e4d333d795d95bb`

## 入站计数/客户端筛选/速度/最后在线/路由重复（2026-10-31，未打包）

- 入站列表默认不显示客户端数：`/inbounds/list/slim` 之前只返回 5 个字段。
  现补充 settings（含 clients）、clientStats、enable/up/down/expiryTime、
  originNodeGuid 等，客户端按 client_inbounds 关联归属；在线映射改为
  `{"": [...]}`，`activeInbounds` 由 jobs 每轮记录有流量的入站 tag
- 客户端筛选失效：`/clients/list/paged` 现实现 bucket（active/deactive/
  depleted/expiring）、protocol、inbound、group、到期/用量区间、
  hasComment、hasTgId、autoRenew 过滤，以及 createdAt/updatedAt/email/
  traffic/remaining/expiryTime 排序；summary 计数与邮箱列表同步修正
- 速度不显示：无 WebSocket，改为前端对 5 秒轮询的累计流量做差值求速率
  （客户端列表 5s、入站 slim 改为 5s 轮询），15 秒保留窗口
- 客户端信息增加“上次在线”：jobs 记录在线邮箱的最后出现时间，
  `/clients/lastOnline` 返回毫秒映射，信息弹窗展示
- 路由规则每次保存多一条默认规则：`ensure_api_rule` 之前无条件 insert，
  现在先删除所有 api 入站→api 出站规则再插入一条；连续保存 3 次验证仍为 1 条

验证：`cargo test` 23/23、clippy 零警告；API 与无头 Chrome 实测筛选、
入站客户端数、客户端/入站速度、最后在线行、路由去重均正常。

打包（2026-10-31，包含以上全部修复）：
- `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4.2 MB）
- SHA256 `75e39213d1260a9ea4c6998573ca8b72cabcb469f5e457cd750632e0ae090f13`
- 二进制 SHA256 `6a8b34ae53033c92eea1f33d654ffef0280f41b438562f0c5160e5d26e52d73b`

## Xray 日志落盘（2026-10-31，未打包）

- 新增 `PANEL_XRAY_LOG`，默认 `PANEL_DB` 同目录的 `xray.log`
  （systemd 示例配置下即 `/etc/xray-panel/xray.log`，目录已在
  ReadWritePaths 中）
- supervisor 把 Xray 的 stdout/stderr（带 `[stdout]`/`[stderr]` 前缀）以及
  `[panel] xray started` 事件追加写入该文件；仍保留内存缓冲
- `/panel/api/server/logs/{rows}` 改为优先从日志文件尾部读取最后 N 行，
  文件不存在/为空时回退内存缓冲（面板“日志”弹窗无需改动）
- `deploy/xray-panel.env.example` 与 README 环境变量表已补充
- 日志文件跨面板重启持续追加；暂未做轮转/大小上限

验证：`cargo test` 24/24、clippy 零警告；实测默认路径生成文件、接口读取、
重启后继续追加且只返回最后 N 行。

## 客户端列表查询全量修复与自测（2026-10-31，未打包）

- 在线筛选：后端 `filter=online` 之前没有匹配逻辑（bucket 只认
  active/deactive/depleted/expiring），点击“在线”卡片必然为空；现按
  `online_clients` 邮箱集合匹配
- 在线刷新：前端 onlines / onlinesByGuid / activeInbounds / lastOnline
  查询原先 `staleTime: Infinity` 且只取一次，无 WebSocket 时永远不更新；
  客户端页与入站页均改为 5 秒轮询
- 老版 Xray 无 `GetUsersStats` 时，后台改为按本轮流量增量推断在线
  （并记录 lastOnline），新版 Xray 仍使用真实在线 IP 数据
- 搜索：按 UI 提示扩展为邮箱、备注、分组、subId、UUID、password、auth
- 排序：新增 lastOnline；其余 createdAt/updatedAt/email/traffic/
  remaining/expiryTime 已有
- 过滤完整覆盖：bucket（含 online）、protocol、inbound、group、
  expiryFrom/To、usageFrom/To、hasComment、hasTgId、autoRenew，
  与排序、分页、summary 计数/邮箱列表联动
- 自测：新建 7 个客户端（跨 vless/trojan、多入站、启用/禁用、耗尽/
  即将耗尽/过期、不同分组/备注/用量）+ 自动化脚本
  `tmp/test_filters.py`，40+ 断言全部通过；`cargo test` 24/24、clippy 零警告

## 出站 HTTP / 真实延迟测试修复（2026-10-31，未打包）

根因：之前只做到 SOCKS5 CONNECT 成功就计时，而 Xray 可能在隧道尚未真正
打通时就返回 SOCKS 成功，所以 HTTP/真实延迟都显示 1ms 左右。

修复：
- 探测目标改为 `www.gstatic.com:80/generate_204`（纯 HTTP，无需 TLS 客户端）
- SOCKS 建连后实际发送 HTTP GET，读到状态行才算成功；结果新增
  `httpStatus`、`connectMs`、`ttfbMs`
- HTTP 模式 delay = 请求 TTFB；真实延迟模式 delay = 含隧道建立的总耗时
- 整体探测超时 10s；失败时返回具体原因（无响应/状态行错误等）
- fake xray 测试桩增加 250ms 延迟响应，用于验证计时确实来自请求往返

验证：`cargo test` 25/25、clippy 零警告；fake 端到端返回
`httpStatus=204, delay=250, connectMs=0, ttfbMs=250`（修复前恒为 0-1ms）。

## VLESS 分享链接补全 encryption（2026-10-31，未打包）

根因：`xray::link::client_link` 的 vless 分支把 `encryption` 写死为 `none`，
即使入站配置了 VLESS Encryption，生成的 vless:// 链接也不带客户端加密参数，
扫码导入后无法连接。

修复：
- vless 链接改用入站 `settings.encryption`（vlessenc 生成的客户端值），
  为空时回退 `none`；服务端 `decryption` 不写入链接
- 前端二维码不再按“是否后量子”隐藏，改为按 QR 容量判断（errorLevel L /
  version 40 = 2953 UTF-8 bytes）；短 vlessenc 链接现在会正常出二维码，
  超长链接仍只提供复制
- 新增两个单测：加密链接带 CLIENTKEY 且不带 SERVERKEY；普通链接保持 none

验证：`cargo test` 27/27、clippy 零警告；API 返回
`vless://...?encryption=mlkem768x25519plus.native.0rtt.CLIENTKEY&...`；
无头 Chrome 实测加密节点二维码正常渲染。

## 入站启用/禁用修复（2026-10-31，未打包）

根因有两个：
1. 前端开关用 `FormData`（multipart/form-data）提交，而兼容层 `read_params`
   只解析 JSON / urlencoded，读不到 `enable`，后端始终按 false 处理；
2. 开关直接改行对象的 `enable` 字段，但没有触发 React 渲染，看起来“点了没反应”。

修复：
- `read_params` 增加 multipart/form-data 解析（axum::extract::Multipart），
  所有 FormData 表单（入站开关、备份上传等）受益；
- 入站列表开关改为乐观更新 + `useReducer` 强制重渲染，失败时回滚；
- 禁用后 `service::apply` 重新生成配置，`config_value` 只读取
  `list_enabled`，因此禁用入站不会出现在 Xray 配置中，连接立即断开；
  启用后重新加入配置并重启 Xray。

验证：
- 单测 `reads_multipart_form_fields`；`cargo test` 28/28、clippy 零警告；
- API：multipart `enable=false/true` 均生效；禁用后 Xray 配置 inbounds 仅
  `["api"]`，启用后为 `["tog-4433","api"]`；
- 无头 Chrome：点击开关立即 `true→false`、再点 `false→true`，后端状态同步。

打包（2026-10-31，包含日志落盘、在线/筛选、出站延迟、vless encryption、
入站启停等全部最新修复）：
- `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4.3 MB）
- SHA256 `adcc503372043e73eefd32f5dccad6cb6cea61a87f76b74973140f4c792da28f`
- 二进制 SHA256 `0fffe3f21c0e9790e3ec4c95776314bc1df1371fc2753e8729babf635fe65535`

## 面板设置生效与精简（2026-10-31，未打包）

- 移除“通知”和 LDAP 两个子页签；GeneralTab 现在保留：常规、证书、外部流量、日期和时间
- 保存后设置不再被运行值覆盖：`setting/all` 之前会用当前监听地址覆盖已保存的
  webListen/webPort，导致刷新后又变回去；现已改为已保存值优先
- 监听 IP/端口：面板启动时读取已保存的 `webListen`/`webPort`，
  优先于 `PANEL_LISTEN` 环境变量；点击“重启面板”后即按新地址监听
- 会话时长：登录/会话创建时读取 `sessionMaxAge`（分钟），新会话生效
- 默认设置接口补齐 `sessionMaxAge`、`datepicker`、`timeLocation`、
  `restartXrayOnClientDisable`，日期选择器与页面大小保存后立即生效
- `restartXrayOnClientDisable=false` 时，客户端因超量/到期被禁用不再重启 Xray
- 保存时校验 webPort(1-65535) 与 webListen(IP 或空)
- 仍不支持（保存但不生效）：webBasePath（面板固定根路径）、面板 TLS
  证书/私钥、可信代理 CIDR、IP 白名单、面板出站、外部流量 webhook、时区/LDAP

验证：
- 设置 webPort=2067 后重启：环境仍是 2066，面板实际监听 2067；
- sessionMaxAge=2 后新登录会话 TTL=120s（旧会话保持原 7 天）；
- setting/all 与 defaultSettings 返回保存值；
- 无头 Chrome：设置页签仅剩常规/证书/外部流量/日期和时间，界面显示保存的 IP/端口；
- `cargo test` 28/28、clippy 零警告

## 设置页精简 + 运行环境可视化 + 100 客户端稳定性自测（2026-10-31）

设置页：
- 只保留已实现项；移除通知、LDAP、证书、外部流量、时区、webDomain、
  webBasePath、可信代理 CIDR、IP 白名单、面板出站等未实现字段
- 页签变为：常规、运行环境、日期和时间
- 新增“运行环境”页签：Xray 二进制、配置文件、API 地址、日志文件、
  安全 Cookie；这些值保存在数据库，启动时覆盖环境变量（已保存优先），
  修改后点“重启面板”生效
- 常规页只留监听 IP/端口、会话时长、页面大小、客户端禁用后重启 Xray、
  Reality 扫描候选、语言；日期页保留日历选择
- `PANEL_DB`、`PANEL_LOG`、`PANEL_WORKERS` 仍需环境变量（启动前需要）

验证：
- 用错误环境变量启动（XRAY_BIN=/nonexistent、PANEL_LISTEN=2099），
  面板读取保存值后监听 2071、使用保存的 fake xray 路径、配置与日志写到
  保存路径，Xray 状态 running
- 无头 Chrome：设置页签为「常规/运行环境/日期和时间」

稳定性自测（120 客户端、2 个入站）：
- 24 并发 × 25s：26,864 请求，0 失败，p50 19.6ms / p95 47.3ms /
  max 193.6ms，约 1075 req/s，RSS 21.3MB → 22.9MB
- 100 并发 × 20s：15,475 请求，0 失败，p50 145ms / p95 186ms /
  max 326.6ms，约 774 req/s，RSS 23.9MB；结束后面板仍正常，客户端 120 个
- 测试使用 fake Xray，无法验证真实 100 条 VPN 隧道的吞吐/断线，只覆盖
  面板 API、配置生成、并发与内存稳定性

生产包（2026-10-31，含设置页精简/运行环境/稳定性修复）：
- `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4.3 MB）
- SHA256 `3511acbb055d40285406dcaeb068c3c651a91d14dae96962436e15a7bb6d88b9`
- 二进制 SHA256 `e56cec3c01772c991affef2648a00948564b7f1c8dc6ed7b013af5cd8038fc6c`

生产包补充：加入 `deploy/install.sh`（一键安装 systemd 服务、创建数据目录、
备份现有 Xray 配置）。归档重新生成：
- SHA256 `22d31f170ad02ef05d6de98e2dae812c98bb198bbdba9bcf0a3636bcf09bb765`
- 二进制 SHA256 不变 `e56cec3c01772c991affef2648a00948564b7f1c8dc6ed7b013af5cd8038fc6c`

## 在线客户端按“入站连接”归属（2026-10-31，未打包）

问题：客户端同时关联多个入站时，只要在线就会在每个入站都显示在线；
用户级 stats 只有 email，没有入站信息。

实现：
- 生成 Xray 配置时强制启用 access log（`log.access` 指向
  `PANEL_XRAY_ACCESS_LOG`，默认与 xray.log 同目录的 access.log）
- jobs 每 10s 增量读取 access log，解析
  `[inboundTag >> outboundTag] email: xxx`，维护
  email -> inboundTag -> 最近出现时间（10 分钟窗口，文件超 64MB 清空）
- 新增 `POST /panel/api/clients/onlineInbounds` 返回在线客户端的
  email -> [inboundTag]
- 前端入站列表优先使用该精确映射判断“该入站上的在线客户端”；
  没有日志记录时回退到原来的入站活跃度判断
- stats API 不可用时，也改用 access log 作为在线兜底

验证：
- 单元测试 `parses_access_log_email_and_inbound`、配置中 `log.access` 断言
- E2E：alice 同时关联 vless:4433 与 trojan:4434，仅写入 4433 的访问日志，
  接口返回 `{"alice":["in-4433-tcp"]}`，在线列表仅 alice；
  入站列表 4433 显示在线 1，4434 显示在线 0（两者客户端数均为 2）
- `cargo test` 29/29、clippy 零警告

生产包（2026-10-31，含按入站归属在线、访问日志、设置页运行环境等）：
- `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4.3 MB，含 install.sh）
- 归档 SHA256 `622e1ebd583d84494ae7cb39074507f7d16be20c41b97c7efe2bb6355ae912e5`
- 二进制 SHA256 `1e52a2d7a9a1359399c189710962436e33a531c222e6708db2569ebbd010d287`
- `cargo test` 29/29、clippy 零警告

## 在线归属回归修复 + 版本号 0.2.0（2026-10-31，未打包）

- 访问日志箭头格式修复：Xray 实际输出为
  `[inboundTag -> outboundTag]`，此前解析器只按 `>>` 拆分，导致
  “入站 tag”解析成整段字符串、永远匹配不到，入站列表在线数变成 0。
  现同时支持 `->` 与旧格式 `>>`，单测覆盖两种格式
- 版本号升到 **0.2.0**（Cargo.toml），随 `X_UI_CUR_VER` 注入前端：
  系统状态页显示 v0.2.0，侧栏展开时品牌旁也显示版本号（抽屉同样）
- 验证：`cargo test` 29/29、clippy 零警告；E2E 写入
  `[in-4433-tcp -> direct] email: alice` 后接口返回
  `{"alice":["in-4433-tcp"]}`；无头 Chrome 确认页面显示 v0.2.0
- 当前 dist 内的生产包是 0.1.0 且不含箭头修复，需要重新打包才能部署

## 0.2.0 生产包（2026-10-31）

- `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4.3 MB，含 install.sh）
- 归档 SHA256 `9df7fe458314ca6e325b2892d522a9e11c1ecdd605218c78f74f96d60d18bfd5`
- 二进制 SHA256 `9a583a167d7445fafe0bdf9b0575b7f41e95989c3c89d76ac74c6a4f52190423`
- 内容：访问日志 `->` 解析修复、入站在线精确归属、版本号 0.2.0 展示

## 在线状态收尾：Xray 重启清空 + lastOnline 兜底 + 轮询节拍（2026-10-31）

完成上一轮未收尾的在线状态优化：

- `XrayHandle::generation()`：supervisor 在 Xray 子进程启动、停止、崩溃退出时
  递增共享 `Arc<AtomicU64>`；jobs 每轮比对，变化即清空 `online_clients` /
  `online_last_seen` / `online_inbounds` / `active_inbounds`，并把 access log
  读取位置重置到 EOF，避免重启后旧连接继续显示在线
- 修正宽限单测：`online_grace_secs(300)` 现在断言 60 秒（2×轮询，上限 60），
  旧断言 600 秒与 `MAX_ONLINE_GRACE_SECS` 矛盾
- stats API 不可达时的两处改进：
  - `StatsServiceClient::connect` 增加 3 秒连接超时（RPC 仍为 10 秒）
  - tick 改为固定节拍：sleep 扣除本轮耗时，10 秒轮询不会再被两次失败的
    connect（各约 2 秒）拉长到约 14 秒
  - stats 失败分支也会调用 `persist_online_heartbeat`，此时由 access log
    判定在线的客户端同样会写入 `clients.last_online`
- 验证（fake xray，stats API 不可达，access log 注入 alice）：
  - 上线后 `/panel/api/clients/lastOnline` 返回 alice（毫秒）且数据库
    `last_online` 已更新
  - 重启 Xray 后 0.5 秒（下一次 tick）在线列表清空，未等 20 秒宽限自然过期
  - debug 日志 tick 起点间隔稳定为 10.0 秒（37.17 → 47.18 → 57.18 → 07.21）
  - `cargo test` 34/34、`cargo clippy --all-targets -- -D warnings` 零警告
- 生产包 **0.2.1**（2026-10-31，含以上全部改动，musl 静态 ELF，已 strip）：
  - `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4.3 MB，含 install.sh）
  - 归档 SHA256 `57389d09a99a772d0c13c9af5802c530594c5bd57d11d8aa8d590bbaa4e11d83`
  - 二进制 SHA256 `cdfc5871bf103a0512a8b5f7b98e6596d8f8f604538eaee9719496f4006cdb3d`
  - 版本号从 0.2.0 升到 0.2.1，用于区分 12:30 的旧 0.2.0 包

## 入站在线归属修复：长连接标签不再随宽限过期（2026-10-31，未打包）

问题：入站页对“实际在线但空闲”的客户端显示不稳定；多个入站在跑流量时，
客户端还可能被显示到并非其连接的入站上。

根因：入站归属来自 access log 的 `accepted` 行，而一条长连接只在建立时写
一次。`online_inbounds` 之前和在线宽限共用 20–60 秒 TTL，过期后前端回退到
`activeInbounds`（最近有流量的入站）：
- 空闲但一直连着的客户端在入站页上消失；
- 有多个活跃入站时，客户端会被标记到所有活跃入站上。

修复：新增 `retain_online_inbounds`（`src/jobs.rs`）。只要面板当前仍认为该
邮箱在线（`online_clients` 快照，来源为 GetUsersStats / 流量 / access log），
就保留它的入站标签；客户端下线后仍按原宽限过期。新增单测覆盖三种状态：
在线且标签很旧（保留）、离线且标签过期（删除）、离线但刚出现过（保留）。

验证：
- `cargo test` 35/35、`cargo clippy --all-targets -- -D warnings` 零警告
- fake xray 回归：注入 access log 后 `/clients/onlineInbounds` 返回
  `{"alice":["e2e-in"]}`；重启 Xray 后在线表与入站归属一起清空
- 已知限制：同一段连续在线期间先后连接过多个入站时，旧入站标签会保留到
  该客户端下线（Xray 与 access log 都不提供连接关闭事件）

当前未打包，`dist/` 内仍是 0.2.1 包（不含本修复）。

## 改用 3x-ui 稳定版入站在线语义：移除 access.log 依赖（2026-10-31，未打包）

> 该近似方案无法区分同入站下的具体客户端，已由下一节的 per-attachment 方案取代。

用户选择“照搬 3x-ui 稳定版”后执行：

- 全局在线仍来自 Xray `GetUsersStats`（email + 在线 IP），不读 access.log
- 按入站在线改为只看 `activeInbounds`：每轮从
  `inbound>>>tag>>>traffic` 增量记录最近有流量的入站 tag，前端只把全局
  在线的客户端显示在活跃入站上（与 3x-ui 相同）
- 删除整条 access log 链路：
  - `online_inbounds`、`access_log_offset` 状态与
    `update_online_inbounds` / `parse_access_line` / `retain_online_inbounds`
  - `/panel/api/clients/onlineInbounds` 现在固定返回 `{}`，前端回退到
    `activeInbounds`
  - 自定义 `/api/inbounds` 列表也只用 active tags
  - 生成配置不再强制写 `log.access`，保留用户在 Xray 页配置的 log 块
  - 移除 `PANEL_XRAY_ACCESS_LOG` 环境变量、`Config.xray_access_log` 字段，
    README 与 env 示例同步删除
- 行为取舍（与 3x-ui 一致）：空闲但保持连接的客户端在宽限窗口（20–60s）
  后会从入站页的在线列表消失，但仍在全局在线列表；入站只有在最近有流量时
  才会显示在线客户端

验证：
- `cargo test` 32/32（删除了 3 个 access-log 相关单测）、clippy 零警告
- fake xray 端到端：生成配置 `log` 仅 `{"loglevel":"warning"}`（无 access）；
  `/clients/onlineInbounds` 返回 `{}`，`/clients/activeInbounds` 返回
  `{"":[]}`；入站 slim 与客户端关联正常；面板与 Xray 正常启动
- 生产包 **0.2.2**（2026-10-31，musl 静态 ELF，已 strip）：
  - `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4.3 MB，含 install.sh）
  - 归档 SHA256 `308bb185278cafeed430697e8b39a1317b901e3454774a001cc5d3664568ef8c`
  - 二进制 SHA256 `aea10419bcf99a7e3fd81357e941f75bc303de82dcc131fb1d8f7d766d0e2ff0`
  - 版本号 0.2.2；README/env 示例已移除
    `PANEL_XRAY_ACCESS_LOG`，生成配置不再强制 access log

## 入站在线精确归属：per-attachment 统计身份（2026-10-31）

用户场景：A、B、C 都关联入站1 和入站2；A/B 连接入站1、C 连接入站2。
`activeInbounds` 只知道入站有没有流量，会把关联到活跃入站的**所有在线客户端**
都显示上去，无法做到“入站1 只显示 A/B、入站2 只显示 C”。

实现方式（对齐 3x-ui per-attachment 方向）：

- 新增 `src/xray/identity.rs`：Xray 统计身份写成 `<inbound_id>:<email>`，
  `traffic_pattern()` 生成对应重置 pattern
- 配置生成：VLESS/VMess/Trojan/Shadowsocks-2022 的 client `email` 改为
  attachment 身份；数据库和 API 仍保存/展示面板邮箱
- 路由 `user` 规则：仅在写入磁盘的**运行配置**里展开为各 attachment 身份；
  API/界面返回的配置仍显示面板邮箱，避免编辑器出现 `12:alice`
- 流量统计：`jobs` 把同一客户端的各个 attachment 累计值求和后再算增量；
  兼容配置里残留的纯 email key
- 在线归属：`GetUsersStats` 返回的 attachment 身份解码回
  (面板邮箱, 入站 tag)，存入 `OnlineUser.inbounds`；
  `/clients/onlineInbounds` 返回精确映射，自定义 `/api/inbounds` 也优先用它
- 重置：客户端重置/按天重置/入站重置/全部重置都改为按
  `user>>><inbound_id>:<email>>>traffic>>>` 逐 attachment 执行
  - 关键修正：任何会清零客户端数据库累计值的操作，必须同时重置该客户端的
    **全部** attachment，再清数据库；只重置部分 attachment 会让累计值下降，
    下一轮被 `delta()` 当成计数器回绕而重复计流量
  - 入站定时重置和兼容层手动入站重置现在都会重置成员客户端的全部
    attachment；删除不再使用的 `reset_traffic_by_inbound`
- `limitIp`：同一客户端各 attachment 的在线 IP 去重后计数

验证：
- `cargo test` 36/36、clippy 零警告
- fake Xray + fake StatsService（`tmp/fake_stats`，测试专用）端到端：
  - 初始在线 `1:A`、`1:B`：入站1 显示 A/B，入站2 显示 0——即使两个入站
    最近都有流量
  - 加入 `2:C` 后：入站1 仍只有 A/B，入站2 只有 C；
    `/clients/onlineInbounds` = `{A:[in1], B:[in1], C:[in2]}`
  - A 的流量按 attachment 汇总：100 + 50 = 150；`lastOnline` 三人均正常落库
  - 运行配置的 routing `user` 为 `["1:A","2:A"]`，API 展示仍为 `["A"]`
- 已知影响：
  - 直接读 Xray stats 的外部工具会看到 `id:email` 身份（面板内部使用）
  - 备份/恢复不受影响，数据库仍存面板邮箱
- 生产包 **0.2.3**（2026-10-31，musl 静态 ELF，已 strip）：
  - `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4.3 MB，含 install.sh）
  - 归档 SHA256 `336d92c8f13b52742c92132f4b80c01298d39001506fb0969b3b198e569f89ad`
  - 二进制 SHA256 `86996dc61b83465818cc6b17cb46d5450303d7f4b93afd6cd7df480520ca92a1`
  - 版本号 0.2.3，包含 per-attachment 精确入站在线归属

## 可配置刷新间隔（2026-10-31）

入站列表工具栏新增 **刷新间隔**（设置页不再显示该控件）：

- 选项 2 / 5 / 10 / 15 / 30 / 60 秒；新装默认 **5 秒**
- 已有安装保留数据库里的原值（此前默认 10 秒），可在入站列表直接改
- 选择后乐观更新 + 立即保存；后端 `Notify` 唤醒轮询任务
- 该值同时控制：
  - 后端 Xray stats / 在线状态 / 入站活跃度采集（`traffic_poll_secs`，2–300）
  - 前端系统状态、入站、客户端等页面的轮询间隔
  - 速度列的计算窗口与后端采集对齐，减少之前的倍率偏差
- 后端保存后通过 `Notify` 立即唤醒轮询任务，不必等完当前 sleep
- `setting/all`、`setting/defaultSettings` 返回 `trafficPollSecs`；
  `setting/update` 校验 2–300 后写入 `settings.traffic_poll_secs`
- 14 个语言包新增 `pages.settings.refreshInterval` /
  `refreshIntervalDesc`（非中英文语言暂用英文文案）

验证：
- 前端 `npm run build` 成功，改动文件 typecheck 无报错
- `cargo test` 36/36、clippy 零警告
- fake Xray + fake StatsService 端到端：
  - 新库默认 5 秒：dashboard 历史时间戳间隔 5,5,5
  - 设置保存为 2 秒后：3（过渡）,2,2,2；`setting/all`、
    `defaultSettings` 与数据库均返回 2
- 生产包 **0.2.5**（2026-10-31，musl 静态 ELF，已 strip）：
  - `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4.3 MB，含 install.sh）
  - 归档 SHA256 `f3c10e2e3a610c540a35002e179a221a849f4e7b933d2a0aa77c588efcb696f2`
  - 二进制 SHA256 `0b19c0ae00b05f46973f275a75a3e77af926f79bd9a8593d6fceb2d3e5a11c49`
  - 版本号 0.2.5：刷新间隔选择器位于入站列表工具栏，设置页不再显示；
    同时包含 per-attachment 精确入站在线

## 面板路径 / 版本号位置 / 定期内存释放（2026-10-31）

### 面板路径可自定义

- 设置页「常规 → URI 路径」新增 `webBasePath`（原 3x-ui 字段，翻译键
  `pages.settings.panelUrlPath`），必须为 `/xxx/`；保存后**立即生效**，
  无需重启面板
- 后端新增 `src/web/base_path.rs`：校验/规范化路径，并在路由匹配前剥掉
  前缀。实现上用外层 Router 把内层 Router 当 fallback service，保证中间件
  运行在路径匹配之前（同层 `.layer` 会在路由选择后才执行，只能命中 404）
- 静态资源：Vite 产物里的 `/assets/...` 在注入 HTML 时改写为
  `<前缀>/assets/...`；`X_UI_BASE_PATH` 注入实际前缀，前端路由、HttpUtil、
  登录跳转直接复用
- 独立 `/settings` 页面同步注入 `B` 前缀，账号/备份/退出请求都走新路径
- `/api/health` 始终保留在根路径，方便探活；其余非前缀请求一律 404
- 启动时从 `x3ui_settings.webBasePath` 读取；设置保存后更新内存中的前缀，
  旧地址立即失效

### 版本号只在系统状态

- 侧边栏品牌区（桌面/抽屉）不再显示面板版本号，删除 `VersionBadge`
  与相关 CSS
- 版本号仅保留在「系统状态」页（概览操作栏版本按钮 / 更新弹窗）
- 侧边栏「面板设置」图标链接改为带 `X_UI_BASE_PATH` 前缀

### 定期释放内存

- 维护任务由每小时会话清理改为**每 15 分钟**：清理过期会话 →
  `PRAGMA shrink_memory` → 收缩在线/活跃/日志/流量历史/登录限流等容器
  → glibc 下调用 `malloc_trim(0)`
- musl 的 mallocng 在 free 时自行 madvise 归还空组，无需（也没有）
  `malloc_trim`；生产 musl 静态包同样受益于容器收缩与 SQLite 释放

### 验证

- `cargo test` 40/40、`cargo clippy --all-targets -D warnings` 零警告
- `cargo zigbuild --release --target x86_64-unknown-linux-musl` 通过：
  静态 ELF、已 strip（本次未打包，等用户要求再出 tar.gz）
- 前端 `npm run build` 成功；改动文件 typecheck/oxlint 无新增错误
- fake Xray + 无头 Chrome 端到端：
  - 默认 `/`：系统状态正常，`.brand-version`/`.sider-version` 均不存在，
    版本按钮 `v0.2.5` 仅在概览区
  - 设置页把 URI 路径改为 `/secretui/` 并保存：浏览器自动跳到
    `/secretui/panel/settings`；旧 `/`、`/panel` 返回 404；
    `/secretui/assets/*` 正常加载；带前缀 API 登录态正常
  - `/secretui/settings` 独立设置页可加载并带正确前缀；改回 `/` 后恢复
  - 维护任务临时改为 5 秒观察：debug 日志连续输出
    `maintenance: released idle panel memory`，无 SQLite/panic 报错

## 长期记忆系统启用（2026-10-31）

从本日起采用 `CLAUDE.md` + `memory/` + `docs/` 的长期记忆结构：

- `CLAUDE.md`：会话启动流程、不可违反规则、核心约束、构建命令、索引
- `memory/CURRENT-STATUS.md`：当前真实状态与验证边界
- `memory/DEVELOPMENT-LOG.md`：追加式开发记录（时间/改动/文件/测试）
- `memory/TODO.md`：待办与验收标准
- `memory/NEXT-STEPS.md`：下一次会话执行清单
- `docs/ARCHITECTURE.md` / `DATABASE.md` / `API.md` / `DECISIONS.md` /
  `KNOWN-ISSUES.md`：架构、数据、接口、决策（追加式）、已知问题

本文件（PROGRESS.md）继续保留为历史长日志，不再作为会话主入口。

## 生产包 0.2.6（2026-10-31）

包含本轮全部未打包改动：

- 面板路径可自定义（设置页「常规 → URI 路径」，保存立即生效、自动跳转；
  旧地址 404；静态资源/登录/API/独立 `/settings` 均支持前缀）
- 版本号只显示在系统状态页（侧边栏版本号已移除）
- 每 15 分钟内存维护（会话清理、SQLite `shrink_memory`、容器收缩、
  glibc `malloc_trim`）
- 上一版 0.2.5 发布包保留为
  `dist/xray-panel-rs-linux-amd64-lumen-0.2.5.tar.gz`

产物：

- `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4.4 MB，含 `install.sh`、
  service/env 示例、README、SHA256SUMS）
- 二进制：静态 musl ELF，已 strip，11,591,760 字节
- 归档 SHA256 `52dfe0d993f01f4f525f363e1892eae45ac03c0899982cd853ec32b675f584be`
- 二进制 SHA256 `a67fbf5479e6dc1ee5bb97faccca2b3ceeabe1632e808ab92cc2d40d226c9425`

验证：

- `cargo test` 40/40、`cargo clippy --all-targets -D warnings` 零警告
- 前端 `npm run build` 成功
- `cargo zigbuild --release --target x86_64-unknown-linux-musl` 成功
- 包内二进制与交叉编译产物 `cmp` 一致；`file` 确认静态 ELF、已 strip；
  `gzip -t` 通过；二进制内含版本串 `0.2.6`
- 本次未在真实 Linux + 真实 Xray 上运行该二进制（Windows + fake Xray
  端到端验证见上一节）

### 0.2.6 源码包

- `dist/xray-panel-rs-0.2.6-src.tar.gz`（4,531,619 字节，根目录
  `xray-panel-rs-0.2.6-src/`）
- SHA256 `021ce74d8f98ddc3ec99138dc8eb1eddcad52cf0a78e74b83d096b801217a3b1`
- 包含 Rust/前端/迁移/部署/文档/记忆文件，以及已构建的 `web/dist/`
  （Rust 编译时嵌入）；不含 `target`、`tmp`、`node_modules`、发布包
- 验证：解压后关键目录与工作区 `diff` 一致；`cargo metadata --offline` 成功

## 0.2.7 发布（2026-10-31）

状态：**已打包**；未在真实 Linux + 真实 Xray 上运行。

包含：

- 流量统计可靠性修复：per-attachment 基准、缺失统计保留基准、重启基准重置、
  定时重置仅确认成功后清零、在线 API 重探与未知身份过滤（详见
  `memory/CURRENT-STATUS.md` 2.4、`docs/DECISIONS.md` D019）。
- 日志查看器：单窗口切换面板 / Xray 错误 / Xray 访问日志，默认面板日志；
  登录成功/失败/限流/登出/改密审计；兼容登录入口纳入限流（详见 2.5、KNOWN-ISSUES H/I）。
- 版本号 0.2.7。

发布包：

- `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4,586,290 字节）
  SHA256 `2b2ed83872d49710533cff475196c262a90deb2324e8e5488f160f242cada3e2`
- 包内二进制 `xray-panel-rs`：11,652,592 字节，静态 musl ELF，已 strip
  SHA256 `19463641bb8a7503c63a58007f39745f78f32ef2c0a20c0be055390185c08088`
- 源码包 `dist/xray-panel-rs-0.2.7-src.tar.gz`（4,553,753 字节，根目录
  `xray-panel-rs-0.2.7-src/`）
  SHA256 `0da48c6d3ccbf5bdf4881c9574e9d9b9edd8a540bd6a36c04313f9c25928b64d`

验证：

- `cargo test` 56/56、`cargo clippy --all-targets -- -D warnings` 零警告
- 前端 `npm run build` 成功；`cargo zigbuild --release --target
  x86_64-unknown-linux-musl` 成功
- 包内二进制与交叉编译产物 `cmp` 一致；`file` 确认静态 ELF、已 strip；
  `gzip -t` 通过；二进制内含版本串 `0.2.7`、登录审计串和嵌入前端 `Panel Logs`
- 本次未在真实 Linux + 真实 Xray 上运行该二进制（真实进程冒烟测试只在
  Windows debug 构建上进行）

## 0.2.8 发布（2026-10-31）

状态：**已打包**；未在真实 Linux + 真实 Xray 上运行。

相对 0.2.7：

- 日志弹窗改为**最新日志显示在上方**（下载仍按时间正序）。
- 包含 0.2.7 的全部内容：流量统计可靠性修复、日志查看器、登录审计与兼容
  登录限流。0.2.7 归档为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.7.tar.gz`。

发布包：

- `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4,586,919 字节）
  SHA256 `4403dd162f390ecd033e806fbba986736196a0f2ffd9299a3db876bbf278f83b`
- 包内二进制 `xray-panel-rs`：11,652,576 字节，静态 musl ELF，已 strip
  SHA256 `e8b61c65c49af7bb5163a0aed73e07138f57bce1ffff16514e0906acea0ebba7`
- 源码包 `dist/xray-panel-rs-0.2.8-src.tar.gz`（4,555,692 字节，根目录
  `xray-panel-rs-0.2.8-src/`）
  SHA256 `f9fb86756e5ac328b0a1decdc7502674a696988cd24cc920aa49d8f1b0e9a134`

验证：

- `cargo test` 56/56、`cargo clippy --all-targets -- -D warnings` 零警告
- 前端 `npm run build` 成功；musl 交叉编译成功
- 包内二进制与交叉编译产物 `cmp` 一致；`file` 静态 ELF、已 strip；
  `gzip -t` 通过；二进制内含 `0.2.8` 与嵌入前端 `Panel Logs`
- 未在真实 Linux + 真实 Xray 上运行

## 0.2.9 发布（2026-11-01）

状态：**已打包**；未在真实 Linux + 真实 Xray 上运行。

相对 0.2.8：

- Xray 设置整份保存：`policy`（连接限制、统计开关）和 `metrics` 不再被静默丢弃；
  `ensure_policy` 只强制 `statsUserOnline`，显式开关保留；重置默认配置生效。
- 高级模板只暴露可编辑块（不包含面板托管的 inbounds/api/stats）。
- 入站：实现导入与批量分享链接；shareAddrStrategy/shareAddr/subSortIndex/
  excludeFromSub/disableFlow 落库（迁移 0008）；`disableFlow` 生效；多入站关联
  客户端的列表/详情修复。
- 备份恢复按共有列复制，旧 schema 备份（0007 及更早）仍可恢复。
- 包含 0.2.8 之后一直未打包的科幻主题（默认深空蓝黑 + 霓虹青，浅色冰蓝变体）。
- 0.2.8 归档为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.8.tar.gz`。

发布包：

- `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4,597,175 字节）
  SHA256 `a80c70c83ac02e9c2f005dd8fce4cccae057e8f0796ed6106476d1d868cd1caa`
- 包内二进制 `xray-panel-rs`：11,682,224 字节，静态 musl ELF，已 strip
  SHA256 `850be8d3faf0174cff21869b493b9aa73cf1930c9229686ba515c8a0cd171993`
- 源码包 `dist/xray-panel-rs-0.2.9-src.tar.gz`（4,573,246 字节，根目录
  `xray-panel-rs-0.2.9-src/`）
  SHA256 `b16461bff832d41131482392034c641109c5d502192c6484cd0bd8b7c8ce234e`

验证：

- `cargo test` 61/61、`cargo clippy --all-targets -- -D warnings` 零警告
- 前端 `npm run build` 成功；musl 交叉编译成功
- 包内二进制与交叉编译产物 `cmp` 一致；`file` 静态 ELF、已 strip；
  `gzip -t` 与包内 `sha256sum -c` 通过；二进制内含 `0.2.9`、
  `0008_inbound_meta`/`sub_sort_index`、`Panel Logs`、`connectionLimits`
- 未在真实 Linux + 真实 Xray 上运行；policy/metrics 的 `run -test` 未用真实核心验证

## 2026-11-01 11:58 — 入口精简 + 客户端导入/导出修复（未打包）

- 前端移除：入站菜单「删除所有客户端」「将客户端添加到分组」；客户端页
  「批量添加客户端」。删除 `AddClientsToGroupModal`、`ClientBulkAddModal`
  及后者测试，清理 14 个 locale 的对应文案 key。后端兼容接口保留。
- 后端新增：`GET /panel/api/clients/export` 返回 `[{client, inboundIds, traffic}]`；
  `POST /panel/api/clients/import` 创建/关联客户端、恢复凭据与流量、重复邮箱跳过；
  `db::clients::set_traffic` 恢复计数。
- 验证：`cargo test` 61/61、clippy 零警告、前端构建成功；
  `tmp/e2e-clients-io/e2e.py` 端到端全部通过（导出字段、重复导入、删除、
  错误路径、导入 created=2、二次导出逐字段一致）。
- 未打包、未 bump 版本；未在真实 Xray 上验证导入后运行行为。

## 2026-11-01 12:03 — 打包 0.2.10（入口精简 + 导入导出修复 + 移除菜单栏搜索）

- 版本：0.2.9 → 0.2.10。
- 新内容：入站/客户端三个入口移除；客户端导出/导入真实实现；侧边栏
  Command Palette / Ctrl+K 全局搜索移除（页面内筛选搜索保留）。
- 构建：`cargo test` 61/61、clippy 零警告、前端构建成功、musl release 成功。
- 归档：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4,589,521 字节，
  SHA256 `543dec00...7d268`）；二进制 11,635,888 字节，
  SHA256 `d5036a05...03fb62`；0.2.9 归档保留。
- 未在真实 Linux + 真实 Xray 上运行。

## 2026-11-01 12:17 — 打包 0.2.11（登录失败微信通知）

- 新增登录失败企业微信群机器人通知：真实 IP、尝试账户、时间、入口；成功登录不发送；
  设置项 `loginNotifyWebhook`，设置页可配置。
- 依赖 `ureq` + rustls；`cargo test` 63/63、clippy 零警告、前端构建成功、musl 构建成功。
- 归档 5,217,711 字节 SHA256 `3bae78cd...e0e755`；二进制 12,782,992 字节
  SHA256 `618caef9...8b655`；0.2.10 归档保留。
- 未在真实企业微信和真实 Linux 上验证。

## 2026-11-01 23:03 — 出站订阅（未打包）

- 新增 `outbound_subscriptions`（迁移 0009）与 `/panel/api/xray/outbound-subs*`
  兼容接口：列表/新建/更新/删除/排序/立即刷新/预览。
- `src/xray/subscription.rs` 解析 base64/纯文本订阅里的 vmess/vless/trojan/ss/
  hysteria2/wireguard 链接；identity 稳定标签分配。
- 订阅出站只在运行时配置中合并（prepend/priority），不写入可编辑模板；
  每订阅可选 HTTP/SOCKS 代理拉取；30 秒 job 到期刷新，内容变化才重启 Xray；
  apply 失败回滚到上一次成功列表。
- 出站页“更多”只保留导入/导出；移除 WARP/NordVPN/PIA 菜单与 XrayPage 弹窗挂载。
- 验证：`cargo test` 76/76、clippy 零警告、前端构建成功、本地 E2E
  （mock 订阅/HTTP/SOCKS5 代理 + fake xray）与无头 Chrome 菜单/代理输入框检查通过。
- 未在真实 Linux + 真实 Xray 上验证；未打包、未 bump 版本。

## 2026-11-01 23:12 — 打包 0.2.12（出站订阅）

- 版本 0.2.12；内容：出站订阅（含每订阅 HTTP/SOCKS 代理）、出站页“更多”仅导入/导出。
- `cargo test` 76/76、clippy 零警告、前端构建成功、musl release 成功。
- 归档 `dist/xray-panel-rs-linux-amd64-lumen.tar.gz` 5,365,187 字节
  SHA256 `3eddd23b...cff949`；二进制 13,120,464 字节 SHA256 `8fc5c32f...e06e78`；
  源码包 4,570,602 字节 SHA256 `b734da61...2332`；0.2.11 归档保留。
- 未在真实 Linux + 真实 Xray 上运行。

## 2026-11-02 00:35 — 功能整改（未打包）

- 移除：路由测试、REALITY 扫描、系统历史、面板自更新、客户端 IP 日志、
  Happ/订阅链接 UI、批量调整 flow/limitHwid/adTag、不支持入站协议与 WireGuard；
  清理未挂载页面/测试与空/假成功路由。
- 实现：负载均衡状态/override（RoutingService）、ECH/ML-DSA-65/ML-KEM 生成、
  证书 SHA-256、Xray 版本安装、Geofiles 更新、GEOdata 标准源与更新代理。
- 验证：`cargo test` 83/83、clippy 零警告、前端构建成功；真实 Xray 26.10.31
  Windows E2E（密钥、证书哈希、balancer、版本安装、geoip.dat 更新）全部通过；
  `geodata` 配置 `xray run -test` 通过；订阅/更新/UI 无头检查通过。
- 未验证真实 Linux/GEOdata 自动下载/代理认证；未打包、未 bump 版本。

## 2026-11-02 00:42 — 打包 0.2.13（功能整改）

- 版本 0.2.13；内容：功能整改（移除路由测试/目标扫描/系统历史/面板更新/
  客户端 IP 日志/Happ 与订阅 UI/不支持协议；负载均衡状态/override、ECH/
  ML-DSA-65/ML-KEM、证书哈希、Xray 版本/Geofiles/GEOdata/更新代理）。
- `cargo test` 83/83、clippy 零警告、fmt 通过、前端构建成功、musl release 成功。
- 归档 `dist/xray-panel-rs-linux-amd64-lumen.tar.gz` 5,354,421 字节
  SHA256 `11ab05a6...a377ac`；二进制 12,623,520 字节 SHA256 `d407c249...db9c6`；
  源码包 `dist/xray-panel-rs-0.2.13-src.tar.gz` 4,183,211 字节
  SHA256 `086ff06b...addd9`；0.2.12 归档保留。
- 未在真实 Linux + 真实 Xray 上运行该二进制。

## 2026-11-02 — 修复 Xray 安装时下载代理失效（未打包）

- 现象：更新代理使用面板托管 Xray 的本地入站时，安装 Xray 在下载 zip 阶段
  报 `Connection refused (os error 111)`。
- 根因：`install_xray` 先停止 Xray 再下载，代理入站随进程消失。
- 修复：下载/校验/解压到临时文件在 Xray 运行时完成，替换前才停止；下载
  失败不再中断 Xray。
- `cargo fmt`/clippy 通过，`cargo test` 83/83；未打包、未真机验证。

## 2026-11-02 — 修复 Geofiles 写入只读文件系统（未打包）

- 现象：Geofiles 更新写入 `/usr/local/bin` 时 `Read-only file system`。
- 原因：systemd `ProtectSystem=full` 且 `ReadWritePaths` 缺少 XRAY_BIN 目录。
- 修复：部署单元加入 `/usr/local/bin`；README/known issues 同步。
- 未打包；已部署实例需手动修改单元或设置 `XRAY_LOCATION_ASSET`。

## 2026-11-02 01:11 — 打包 0.2.14（Xray 安装与 systemd 写入修复）

- 版本 0.2.14；内容：0.2.13 全部功能 + Xray 安装先下载再停止 +
  systemd `ReadWritePaths` 加入 `/usr/local/bin`。
- `cargo test` 83/83、clippy 零警告、fmt 通过、musl release 成功。
- 归档 `dist/xray-panel-rs-linux-amd64-lumen.tar.gz` 5,354,579 字节
  SHA256 `3efcf828...06068`；二进制 12,623,456 字节 SHA256 `1ab4559f...34bd2`；
  源码包 `dist/xray-panel-rs-0.2.14-src.tar.gz` 4,185,587 字节
  SHA256 `00e37130...f2624`；0.2.13 归档保留。
- 未在真实 Linux + 真实 systemd 环境运行。

## 2026-11-03 — 协议补全 + 节点地址设置，打包 0.2.15

- 入站补全 11 种 Xray 原生协议（vless/vmess/trojan/shadowsocks/http/mixed/
  wireguard/hysteria/tunnel/tun/masque），出站 13 种；MTProto/TUIC/AmneziaWG
  依赖外部 sidecar，按用户确认不列入，后端明确拒绝。
- 后端按协议重建 clients/peers、生成 WireGuard X25519 密钥与 allowedIPs、
  Hysteria auth、MASQUE pass；修复 Shadowsocks 2022 无客户端丢 password、
  导入客户端缺 subId。
- 面板设置新增「节点地址」；分享链接/二维码优先使用配置的域名/IP，不再使用
  浏览器局域网地址；后端 subLinks/allLinks 同样优先。
- 验证：cargo test 89/89、clippy 零警告、前端构建成功；fake xray API 端到端
  （协议 11/11、节点地址 17/17）+ 无头 Chrome 全部通过。
- 打包：版本 0.2.15，musl release 成功；归档
  `dist/xray-panel-rs-linux-amd64-lumen.tar.gz` 5,398,823 字节 SHA256
  `40b4b245...fb13a`；二进制 12,691,200 字节 SHA256 `105a28a9...b06f2`；
  源码包 `dist/xray-panel-rs-0.2.15-src.tar.gz` 4,207,649 字节 SHA256
  `d6e7c21e...5cac3`；0.2.14 归档保留。
- 未验证：真实 Linux + 官方 Xray 的 `run -test`/连接（T21）；真实客户端用
  新链接连接。

## 2026-11-03 — Release 资产精简为 tar.gz，在线更新改用资产 digest

- 用户要求不再单独发布二进制和 SHA256SUMS；Release 只保留
  `xray-panel-rs-linux-amd64-lumen.tar.gz`。
- 面板在线更新改为下载 tar.gz，使用 GitHub 资产 `digest` 校验，并用内置最小
  tar.gz 解析器解出 `xray-panel-rs`；新增 `flate2` 直接依赖与 3 个解析测试。
- 版本 0.2.17；`cargo test` 93/93、clippy 零警告、musl release 成功。
- v0.2.16 更新器仍查找原始二进制资产，需用一键安装脚本升级一次后才能使用
  0.2.17+ 的在线更新（见 KNOWN-ISSUES N3）。

## 2026-11-03 — 0.2.17：Release 只保留 tar.gz

- `v0.2.17` 发布成功；唯一资产 tar.gz 5,422,123 字节，GitHub asset digest
  `sha256:23a4885d...18a9`。
- 在线更新改为下载 tar.gz + digest 校验 + 内置 tar 解析提取二进制。
- 0.2.16 用户需用一键安装脚本升级一次，之后在线更新恢复（KNOWN-ISSUES N3）。

## 2026-11-04 — 0.2.18：安装/更新代理、systemd 判定与 Linux 集成测试

- 一键脚本支持安装/更新两种模式与 PANEL_PROXY；面板在线更新复用 updateProxy
  并在错误中带出代理地址。
- 服务单元注入 PANEL_SYSTEMD=1，旧单元用 MainPID 兼容；修复 CI 中误判 systemd
  导致进程退出。
- 新增 integration workflow：Linux 上真实跑通 v0.2.17→v0.2.18 在线更新和
  v0.2.18 一键安装；坏代理报错验证通过。
- 发布 v0.2.18，唯一资产 tar.gz 5,425,000 字节，digest `sha256:ba59a2a2...8496`。

## 2026-11-04 18:37 — 七项增强（未打包）

- 客户端详情在线 IP + 按需第三方位置查询（`onlineIps` / `ipLocation`）。
- 后端面向用户错误统一中文；Xray 原生输出保留英文。
- Xray 版本号解析为 `26.10.31` 短格式。
- 设置页安全警告关闭后持久化，重新登录后恢复。
- 高级设置页：出站真实连接检测（单/多选、1–1440 分钟）、微信 Webhook
  不可用通知、最近检测结果；`outbound_check` 后台 job。
- 面板日志固定 UTC+8，按日文件 `panel-logs/` 保留 14 天，记录 target/file/line
  与更多审计事件；日志弹窗支持实时/历史切换。
- 设置保存不再触发 Xray/面板重启；重启面板按钮始终可用并显式执行。
- 验证：cargo test 107/107、clippy、fmt、前端 build；三个 e2e 脚本全通过。
- 未打包、未 bump 版本（0.2.20）。

## 2026-11-04 19:10 — 0.2.21 发布

- 版本 0.2.20 → 0.2.21；提交 `85f85da`，tag `v0.2.21`。
- GitHub Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.21>
  唯一资产 tar.gz 5,465,980 字节，asset digest
  `sha256:143e27bfdc28fa745f433c54337c52c17daa81709d05d2296236639bf47f101a`。
- 本地校验：tar SHA-256 一致、包内 `sha256sum -c` 通过、静态 ELF 已 strip、
  二进制 SHA256 `997fb41a...dcdb`、含 `LUMEN_VERSION=0.2.21`。
- 内容：0.2.20 + 2026-11-04 七项增强（CURRENT-STATUS §2.30）。
- 未验证：真实 Linux/musl 运行。

## 2026-11-04 23:15 — 0.2.22 发布

- 版本 0.2.21 → 0.2.22；提交 `99612d0`，tag `v0.2.22`。
- GitHub Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.22>
  唯一资产 tar.gz 5,473,730 字节，asset digest
  `sha256:c3a60555bd500a17ea52a53fe39ac56436ffe529c404a081e09e2cb1044c44e9`。
- 内容：位置查询主源改为纯真（zxinc）、可选 IP数据云高精查询与独立设置页。
- Release 页面只显示“0.2.22 更新内容”；安装/代理/在线更新说明不再写入 Release
  （DECISIONS D046）。
- 未验证：有效 IP数据云 Key 成功响应、真实 Linux/musl、真实公网客户端。

## 2026-11-04 23:14 — Release 标题调整

- v0.2.22 Release 的更新内容标题改为 `## 更新内容`，不再带版本号前缀。
- 发布 workflow 与 notes 存档同步；临时更新 workflow 已删除。
