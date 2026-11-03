# TODO

> 优先级：P0 阻塞/用户已要求 → P1 重要但不阻塞 → P2 改进 → P3 可选。
> 完成一项后：在 `DEVELOPMENT-LOG.md` 记录，并从本文件移除或标记完成。
> **不要因为“看起来应该做”就自动打包或扩大范围；先确认用户意图。**

## P0 — 用户明确要求时执行

_（暂无。T0 已于 2026-10-31 完成：0.2.6 已打包，见 `DEVELOPMENT-LOG.md`。）_

## P1 — 重要

### T23. 真实 Linux systemd 验证面板在线更新
- 现状：`panel_update` 只用单测验证版本比较；GitHub Actions 首次发布后，尚未在
  真实 Linux 上走完“检查更新 → 下载校验 → 替换 → systemd 重启 → 版本变化”。
- 内容：用旧版本安装包部署 0.2.15 → 登录设置页检查更新到 0.2.16 → 确认自动重启、
  DB/入站/设置保留、`/usr/local/bin/xray-panel-rs.bak` 存在。
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

### T1. 同步 README 与真实行为
- 现状：README 写“流量轮询默认 10 秒、可调 5–300 秒”，代码为默认 5 秒、
  `trafficPollSecs` 允许 2–300；README“明确不做”里写 IP 连接数限制，
  但 `jobs.rs` 已实现 `limitIp` 近似执行；`/settings` 说明也可能过期。
- 内容：以代码为准修订 README，补充面板路径（`webBasePath`）与定期内存维护说明。
- 验收：README 中每条行为都能在代码/测试中找到依据；不再有互相矛盾的描述。

### T2. 面板路径的 HTTP 层集成测试
- 现状：现有验证是 `base_path` 纯函数单测 + 无头 Chrome 手工端到端；
  没有覆盖“guard + 嵌套 Router + 静态资源重写”的自动化测试。
- 内容：用 axum 测试或临时监听端口，覆盖：
  `/` 在自定义前缀下 404、`/x/` 返回登录页、`/x/panel/...` 返回 SPA、
  `/x/assets/*` 可加载、`/x/api/health` 与根 `/api/health`、查询串保留。
- 验收：测试在未实现 guard 时会失败；`cargo test` 通过。

### T9. 在真实 Linux + 真实 Xray 上验证 v0.2.7 统计修复
- 现状：0.2.7 修复只在 Windows debug + in-memory SQLite 单测中验证；真实
  Xray 的 `QueryStats` 缺失行为、`reset_pattern` 部分失败、gRPC 超时均未注入。
- 内容（至少覆盖）：
  - Xray 正常运行，上传/下载各累计 1 GB；
  - 重启 Xray：新进程计数大于旧基准、小于旧基准两种情况；
  - 面板重启：累计流量不丢失、首轮不重复计；
  - 一客户端关联三个入站，分别产生流量；
  - 只重置一个 attachment；多个 attachment 重置时一个 RPC 失败；
  - `QueryStats` 临时缺一个方向；只有下载计数器回绕；
  - 旧核心升级到支持 `GetUsersStats` 的新核心；
  - gRPC 超时后恢复，统计继续正常。
- 验收：有可复现命令与日志证据；结果写入 `DEVELOPMENT-LOG.md` 与
  `KNOWN-ISSUES.md` G6。

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
