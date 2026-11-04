# TODO

> 优先级：P0 阻塞/用户已要求 → P1 重要但不阻塞 → P2 改进 → P3 可选。
> 完成一项后：在 `DEVELOPMENT-LOG.md` 记录，并从本文件移除或标记完成。
> **不要因为“看起来应该做”就自动打包或扩大范围；先确认用户意图。**

## P0 — 用户明确要求时执行

_（暂无。T0 已于 2026-10-31 完成：0.2.6 已打包，见 `DEVELOPMENT-LOG.md`。）_

## 已完成

### T25. 2026-11-04 七项增强 —— 已完成，本地验证，未打包
- 客户端在线 IP/按需位置查询、后端错误中文化、Xray 短版本号、安全警告按
  登录周期显示、高级页出站真实连接检测 + 微信通知、面板日志中国时间/详细/
  历史、设置保存不重启。
- 验证：`cargo test` 107/107、clippy、前端构建；`tmp/e2e-advanced/e2e.py`、
  `ui-run.py`、`online-e2e.py`。详见 `CURRENT-STATUS.md` §2.30。
- 未验证：真实 Linux/musl、真实公网客户端 IP、英文界面下的后端错误语言。

## P1 — 重要

### T26. 真实 Linux 回归 2026-11-04 增强
- 现状：在线 IP、出站检测、日志文件/保留、设置保存不重启、重启面板均只在
  Windows debug + 真实 Xray + 无头 Chrome 上验证。
- 内容：在真实 Linux + systemd + musl 包上确认：Xray OnlineMap 返回公网
  客户端 IP；出站检测临时 Xray 在 systemd 只读路径下可启动（`XRAY_LOCATION_ASSET`
  可写）；`panel-logs/` 目录可写且 14 天轮转；设置保存后台进程 PID 不变；
  `restartPanel` 由 systemd 拉起。
- 验收：日志/命令证据；更新 `KNOWN-ISSUES.md` H5。

### T27. 真实部署网络验证位置查询（zxinc）
- 现状：2026-11-04 将主源从 ipwho.is 改为 zxinc（纯真库），只在 Windows
  开发机联网实测；未在面板实际部署网络、真实 Linux/musl 上复测。
- 内容：在真实面板上对国内电信/联通/移动/境外 IP 各点一次“位置”，确认
  省/市/运营商与 tool.lu 的纯真或淘宝库一致，并记录响应耗时；确认 zxinc
  不可达时回退 ipwho.is 且错误提示可读。
- 验收：记录至少 4 个 IP 的结果与耗时；更新 `KNOWN-ISSUES.md` H5 的
  “未验证”边界。

### T28. 用有效 Key 验证 IP数据云成功响应
- 现状：2026-11-04 新增可选的 IP数据云高精查询（设置 → 高级 →
  IP数据云标签）。0.2.23 起不再使用独立的 `/settings#ipdata` 页面。
  仅用假 Key 验证了错误响应 `key不正确`；成功响应字段按官方文档解析，
  `data` 与 `data.location` 两种层级各有单测，但未用真实 Key 跑通。
- 内容：填入有效 Key，对国内电信/联通/移动/境外 IP 各查一次，确认
  `ipDataCloud` 的国家/省/市/区县/街道/运营商/场景字段与 tool.lu 的
  “高精归属地定位（IP数据云）”一致；确认客户端弹窗两行显示正常。
- 验收：记录至少 4 个 IP 的结果、响应耗时与剩余额度；若字段层级不同，
  修正 `parse_ipdatacloud` 并补测试；更新 `KNOWN-ISSUES.md` H5。

### T24. 真实 Linux 验证安装幂等与更新代理提示
- 现状：`install.sh` 的版本跳过/哈希跳过与 `network_hint` 只在 Windows 本地
  桩环境、fake xray 和真实本机连接拒绝上验证；未在真实 Linux + musl 二进制
  上执行。
- 内容：
  1. 安装 0.2.18 → 重跑脚本：确认走下载+哈希路径后跳过，不停止服务；
  2. 安装包含 marker 的新版本 → 重跑脚本：确认零下载跳过；
  3. `PANEL_FORCE=1`：确认重新执行 `deploy/install.sh`；
  4. 面板设置 `updateProxy=127.0.0.1:10813` → 检查更新：确认提示包含
     代理地址与 127.0.0.1 说明；
  5. 配置真实代理后：确认检查更新恢复正常。
- 验收：命令与日志证据写入 `DEVELOPMENT-LOG.md`；更新 `KNOWN-ISSUES.md`
  N5/N6 的验证状态。
- 备注：`LUMEN_VERSION` marker 只在下一个 Release 生效；0.2.18 及更早只能走
  哈希路径。

### T23. 真实 systemd 服务验证面板自动重启
- 现状：GitHub Actions `integration` 已在 Linux 验证 v0.2.17→v0.2.18 的下载、
  digest 校验、解压、替换和版本变化（非 systemd 路径，`restartRequired:true`）；
  一键安装也用 systemctl stub 验证通过。尚未在真实 systemd 服务下验证
  `PANEL_SYSTEMD=1`/`MainPID` 判定与 `Restart=on-failure` 自动拉起。
- 内容：用 0.2.17 安装包部署到真实 Linux systemd → 检查更新到 0.2.18 → 确认
  服务自动重启、DB/入站/设置保留、`.bak` 存在。
- 验收：有命令/日志证据；结果写入 `DEVELOPMENT-LOG.md` 与 `KNOWN-ISSUES.md` N2。

### T21. 真实 Xray 回归新增入站协议
- 现状：2026-11-03 的 11 种 Xray 原生入站协议只用 fake xray 验证了面板流程和
  生成结构；官方 Xray 的 `run -test`、启动、连接、流量统计均未执行。
- 内容（至少覆盖）：wireguard、hysteria、tunnel、tun、masque 在 Linux + 官方
  Xray 26.10.31 上逐个 `xray run -test`；hysteria/masque 用真实 TLS 证书；
  wireguard 客户端用生成的 keypair 实际握手；确认 Xray 启动后各入站端口正常。
- 验收：有每条命令与日志证据；结果写入 `DEVELOPMENT-LOG.md` 和
  `KNOWN-ISSUES.md` M1。下载/执行外部 Xray 二进制前需用户明确授权。

### T22. 补 hysteria2/wireguard 的服务端订阅链接
- 现状：`src/xray/link.rs` 只生成 vless/vmess/trojan/shadowsocks；
  `subLinks` 对新协议静默跳过（前端列表页链接不受影响）。
- 内容：按前端 `inbound-link.ts` 的 hysteria2 与 wireguard 格式在 Rust 侧实现，
  并补单测（auth/keypair/allowedIPs/MTU）。
- 验收：`subLinks` 对新协议返回非空链接；测试通过。

### T1. 同步 README 与真实行为 —— 已完成（2026-11-04）
- README 已重写并核对代码：轮询默认 5 秒、范围 2–300；`limitIp` 为近似执行；
  健康检查为 `/api/health`；补充 `webBasePath`、内存维护、安装幂等与
  `PANEL_FORCE`。见 `CURRENT-STATUS.md` §2.26。

### T2. 面板路径的 HTTP 层集成测试
- 现状：现有验证是 `base_path` 纯函数单测 + 无头 Chrome 手工端到端；
  没有覆盖“guard + 嵌套 Router + 静态资源重写”的自动化测试。
- 内容：用 axum 测试或临时监听端口，覆盖：
  `/` 在自定义前缀下 404、`/x/` 返回登录页、`/x/panel/...` 返回 SPA、
  `/x/assets/*` 可加载、`/x/api/health` 与根 `/api/health`、查询串保留。
- 验收：测试在未实现 guard 时会失败；`cargo test` 通过。

### T9. 在真实 Linux + 真实 Xray 上验证 0.2.24 统计/速度模型
- 现状：0.2.25 的 3x-ui 模型（重置只清 DB 累计值、保留基准；`/ws` 实时
  traffic 帧）已用 fake Xray（13/13）、无客户端入站（4/4）、真实 Xray
  26.10.31（15/15）和无头 Chrome UI+WS（10/10）验证；仍缺真实 Linux/musl、
  真实公网网络抖动、gRPC 故障注入、MASQUE 混合附着实测。
- 内容（至少覆盖）：
  - Xray 正常运行时上传/下载各 1 GB，核对入站与客户端累计（预期差值为协议开销）；
  - 客户端/入站重置后继续传输，确认只计新字节、不重复计旧累计；
  - 重启 Xray（新进程计数小于/大于旧基准）与面板重启，确认首轮不重复计；
  - 一客户端关联三个入站分别产生流量；只重置其中一个；
  - `QueryStats` 临时缺一个方向；只有下载计数器回绕；
  - 采样期间重启 Xray，确认该样本被丢弃而非错计；
  - 速度与 `iperf`/实际下载速率对比误差、空闲一个周期内归零；
  - 旧核心升级到支持 `GetUsersStats` 的新核心；gRPC 超时后恢复。
- 验收：有可复现命令与日志证据；结果写入 `DEVELOPMENT-LOG.md` 与
  `KNOWN-ISSUES.md` G6–G8。

### T3. 修复前端 devDependencies 缺失导致的 typecheck/test 失败
- 现状：`npm run typecheck` 有大量 vitest/storybook/@testing-library 模块缺失错误；
  这些是 node_modules 安装不完整造成的既有问题，不是业务代码错误。
- 内容：按 `package.json` 完整 `npm install`（或确认 lockfile 与镜像），
  再跑 `npm run typecheck` / `npm run test`。
- 验收：typecheck 只报告真实代码问题；测试可运行；在 `KNOWN-ISSUES.md` 更新状态。

### T14. 客户端导入/导出的浏览器与真实 Xray 验证
- 现状：后端接口已通过 Windows debug + fake xray 的端到端脚本验证；
  未在无头浏览器点击导出/导入弹窗，未在真实 Xray 上验证导入后 apply 与连接。
- 内容：无头 Chrome 打开客户端页执行导出（查看 JSON、下载）与粘贴导入；
  在真实 Linux + Xray 上导入后确认客户端可连接、流量计数从恢复值继续累计。
- 验收：有截图/日志证据；结果写入 `DEVELOPMENT-LOG.md` 与 `KNOWN-ISSUES.md`。
- 已知限制：`inboundIds` 为空的客户端无法导入（schema 外键），会明确跳过。

### T15. 登录失败通知的真实企业微信与 HTTPS 验证
- 现状：本地 HTTP mock 端到端通过；未测试真实企业微信群机器人 HTTPS。
- 内容：配置真实 webhook，触发两次失败登录（原生/兼容各一次），确认消息
  含账户与真实 IP；确认成功登录、空凭据、限流不发送；确认面板在无外网/证书
  错误时只记录 WARN，不影响登录响应。
- 验收：有真实消息截图或接口日志；结果写入 `DEVELOPMENT-LOG.md` 与
  `KNOWN-ISSUES.md` L1。

## P2 — 改进

### T4. 真实 Linux + 真实 Xray 回归
- 内容：在 Linux 上验证：自定义面板路径后的登录/资源/API、独立 `/settings`、
  15 分钟维护任务日志、真实 Xray 启停与流量轮询。
- 验收：有可复现的命令/日志证据；结果写入 `DEVELOPMENT-LOG.md`。

### T5. 设置页提示“路径立即生效”
- 现状：设置页顶部提示“所有更改需要保存并重启面板才能生效”，但 URI 路径保存后
  立即生效并自动跳转。
- 内容：在 URI 路径项增加说明，或调整顶部提示措辞。
- 验收：用户不会误以为路径需要重启；截图/无头 Chrome 验证。

### T6. 检查维护任务对 SQLite 两个连接的效果
- 现状：`PRAGMA shrink_memory` 通过池执行一次，可能只作用于其中一个连接；
  另一连接页缓存上限约 2 MiB。
- 内容：评估是否需要逐个连接执行；若收益可忽略，在 `DECISIONS.md` 记录结论。

### T11. 科幻主题 + 2026-11-01 修复的发布打包 —— 已完成（2026-11-01，0.2.9）
- 发布包 `dist/xray-panel-rs-linux-amd64-lumen.tar.gz` 与源码包
  `dist/xray-panel-rs-0.2.9-src.tar.gz` 已生成；`file`/`gzip -t`/`cmp`/
  `sha256sum -c` 通过，包内前端含 sci-fi 样式与本次修复。

### T10. 日志弹窗的浏览器端验证
- 现状：后端三种来源已用真实进程 + curl 验证，前端 `npm run build` 通过；
  尚未用无头浏览器打开弹窗。
- 内容：打开仪表盘“日志”，确认默认面板日志；切换 Xray error/access；
  验证未配置提示、文件路径提示、长行换行、自动刷新和下载文件名。
- 验收：无头 Chrome 断言 + 截图；控制台无报错。

### T12. RoutingService 诊断/实时操作 —— 已完成（2026-11-02）
- 已实现 balancerStatus / balancerOverride（`src/xray/router.rs`）；路由测试入口
  按用户要求移除，不再实现 TestRoute。override 不跨 Xray 重启持久化。
- 遗留验证：真实 Linux 上回归，见 T19。

### T13. 订阅输出使用 subSortIndex / excludeFromSub
- 现状：0008 已保存这两个字段，但订阅服务器未实现，服务端输出未使用。
- 内容：若将来实现订阅服务，按其排序/排除规则输出；当前保持只保存。
- 验收：订阅输出顺序与排除符合字段语义，并有测试。

### T19. 2026-11-02 功能整改的真实 Linux 回归
- 现状：负载均衡、密钥生成、证书哈希、Xray 安装、Geofiles、GEOdata 只在
  Windows + 真实 Xray 26.10.31 验证；Linux 路径/权限/`XRAY_LOCATION_ASSET`、
  GEOdata 自动下载、更新代理认证未验证。
- 验收：真实 Linux 上安装同一版本、更新一个 Geofile、运行 balancer 状态/
  override、生成 ECH/ML-DSA/ML-KEM；记录结果与失败恢复行为。

### T20. 清理活动页面中的 Hosts/Nodes 残留 plumbing
- 现状：Hosts/Nodes 管理页已删除，但 Inbounds/Clients 页面仍调用空列表接口
  用于 host 标签/节点过滤。无 UI 管理入口。
- 验收：删除或明确标注这些只读数据源；不留下“空但成功”的隐含接口。

### T16. 出站订阅真实 Linux + 真实 Xray 回归
- 现状：2026-11-01 的实现只在 Windows debug + fake xray + 本地 mock 订阅/
  代理服务器上验证；`xray run -test` 的真实拒绝/回滚未注入。
- 内容：在真实 Xray 上验证拉取（含 socks5 代理）、模板合并、重启生效、
  坏出站回滚、30 秒 job 到期刷新。
- 验收：记录真实核心版本与输出；失败时回滚且线上配置不丢。
- 相关：`docs/KNOWN-ISSUES.md` B7。

### T17. 扩展出站订阅格式与坏出站过滤
- 现状：只支持链接列表正文（base64/纯文本）；JSON/Clash 不解析。
  一条坏出站会让整批刷新回滚（不做逐条过滤）。
- 内容：按需支持 JSON 出站数组；在 apply 前尝试定位并剔除被核心拒绝的条目。
- 验收：混合好坏出站时保留可用条目并记录 `lastError`；有对应测试。

### T18. 出站订阅重定向的逐跳 SSRF 校验
- 现状：初始 URL 会做私网校验，ureq 跟随重定向时不再检查目标。
- 内容：为 HTTP 客户端增加逐跳校验或在重定向后复核。
- 验收：重定向到 127.0.0.1/私网地址被拒绝；测试覆盖。

## P3 — 可选/按需

### T7. 3x-ui 高级接口补全
- 仅当用户明确要求：2FA、邮件、订阅服务器、节点、HWID 日志等。
  当前统一返回明确错误，不伪造成功。
  （出站订阅、负载均衡状态/override、密钥/证书工具、版本/Geofiles 已实现。）
- 验收：每个接口有真实行为或明确文档化为不支持；不得返回假数据。

### T8. 清理非生效前端目录
- `frontend/`、`web/dist-custom-v2/` 为历史备查，当前不参与构建。
- 仅当用户确认不再需要时再删除；删除前记录于 `DECISIONS.md`。
