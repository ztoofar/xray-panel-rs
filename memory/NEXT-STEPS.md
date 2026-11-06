# NEXT-STEPS

> 下一次会话从这里开始。写完本文件时假设读者没有本次会话的上下文。
> 最后更新：2026-11-06

## 第 0 步（每次会话必做，不可跳过）

1. 读 `CLAUDE.md` → `memory/CURRENT-STATUS.md` → 本文件。
2. **核对真实代码状态**：
   ```sh
   cd /d/ClaudeCode/Projects/xray-panel-rs
   git log --oneline -5
   git status --short
   cargo test 2>&1 | tail -5
   cargo clippy --all-targets -- -D warnings 2>&1 | tail -3
   grep '^version' Cargo.toml
   ls migrations
   ls -la web/dist/index.html
   find frontend-3xui/src -newer web/dist/index.html -type f | head
   ```
   期望：`cargo test` 132/132、clippy 零警告、Cargo 版本 `0.2.34`（已发布
   v0.2.34）、`web/dist` 不旧于前端源码；工作区除本次未提交改动外干净。
3. 用户说“继续开发”但未指定任务时：先汇报核对结果，再从 `TODO.md` P1 建议，
   等用户选择。**每次代码修改都 bump 版本；不要自动打包、不要自动打 tag/Release。**

## 当前状态（2026-11-06）

- **当前源码：`0.2.34`（已发布 v0.2.34）**（§2.54、D059）：不再注入
  `XRAY_BUF_SPLICE`，恢复 Xray 默认 splice 零拷贝；tunnel 入站实时速度改由
  `src/net_diag.rs` 读取 Linux `NETLINK_SOCK_DIAG` 内核 socket 计数（按监听
  端口归属，仅显示；计费仍用 Xray 计数）。验证：`cargo test` **132/132**、
  clippy 零警告、fmt、musl 交叉构建；前端无改动。**Linux 实际效果未验证**
  （Windows 无 sock_diag/无 splice），见 TODO T32。
- 0.2.33（已发布 v0.2.33，§2.53、D058）：曾默认注入
  `XRAY_BUF_SPLICE=disable` 关闭零拷贝换连续计数；0.2.34 已改为保留零拷贝 +
  内核计数显示。
- **当前发布：GitHub `v0.2.34`**（tag `5201899`，§2.54、D059）：
  main CI `42225524738`、release `42225960322` 成功；资产 5,533,163 字节，
  digest `sha256:7b50e743...ff60`；包内二进制 `8be945d3...6ec4`
  （13,015,312 字节，静态 ELF 已 strip）、`LUMEN_VERSION=0.2.34`，
  本机经代理下载校验通过。
- 上一发布：`v0.2.33`（tag `d6238b7`，§2.53、D058）：关闭 splice 换连续
  计数；资产 5,531,253 字节，digest `sha256:2439a1f7...306c`；包内二进制
  `ecaaec3f...8e49f`、`LUMEN_VERSION=0.2.33`，本机经代理下载校验通过。
- 更早：`v0.2.32`（tag `9e79339`，§2.52、D057）：原生绝对速率快照；
  资产 5,528,182 字节，digest `sha256:5414dff9...b224`；包内二进制
  `5971da08...eee0`、`LUMEN_VERSION=0.2.32`，本机下载校验通过。
- 0.2.32（§2.52、D057）：`/ws` 的 `traffic` 帧改为**原生绝对速率快照**
  `{inbounds:{tag:{up,down}}, clients:{email:{up,down}}, intervalMs, ttlMs}`
  （字节/秒），移除 3x-ui 的 `traffics`/`clientTraffics`/
  `clientTrafficIntervalMs`。服务端以 `ws_events.receiver_count() > 0` 判断
  是否有人看面板：无订阅者时不换算速率、不建帧、`traffic_speeds` 置空
  （REST `upSpeed/downSpeed` 为 0）；有订阅者且客户端 ≤2000 时仍 1 秒一帧。
  计费 `BillingSink` writer、在线/配额/重置始终按 `traffic_poll_secs` 运行，
  与是否有人查看无关。前端直接渲染速率并按 `ttlMs` 过期，不再做差分/除法。
- 0.2.32 验证：`cargo test` **122/122**、clippy 零警告、fmt、前端构建；
  `tmp/e2e-traffic-accuracy/e2e.py` **18/18**（含“关闭面板速度为 0、累计
  照常”与 WS 打开后 REST 速度非零）；`noclient.py` 4/4；无头 Chrome
  `tmp/e2e-traffic-ui/ui_run.py` **12/12**；`tmp/diag-traffic-ws/diag.py`
  稳态均值 +0.10%（真值 1.000/2.000 MB/s）。
- **未验证**：真实 Linux/musl 部署与 >2000 客户端回退；`tunnel_real.py`
  已改为原生协议但本次未重跑（需真实 Xray 环境）。
- 已知设计后果：若浏览器 WS 连不上、只能靠 REST 兜底，速度会一直显示 0
  （无人订阅即视为面板未被查看）。已写入 `docs/API.md` / `KNOWN-ISSUES.md`。
- 更早发布：`v0.2.30`（采样与计费解耦，tag `c3bd5b5`，§2.50）、
  `v0.2.29`（3x-ui 模型重写取数）、`v0.2.28`（入站 WS 过期保护）。

## 下一步（按用户任务二选一）

### A. 用户要求“打包/发布”
1. 确认 `Cargo.toml` / `Cargo.lock` 版本一致（当前 `0.2.34` 已发布；任何新
   改动先 bump 到 `0.2.35`），同步 `memory/CURRENT-STATUS.md`、
   `memory/DEVELOPMENT-LOG.md`。
2. 本地自测：前端 `npm run build`、`cargo fmt --check`、`cargo test`、
   `cargo clippy`。
3. 更新 `.github/workflows/release.yml` 的 `body`：标题固定 `## 更新内容`
   （不带版本号），只列本次更新条目；不写安装/更新说明，不启用
   `generate_release_notes`（D046/D047）。v0.2.33 的条目已随上一版发布，
   0.2.34 必须换成本次实际改动（保留 splice 零拷贝；tunnel 实时速度改用
   内核 socket 计数；http/mixed 的剩余限制）。
4. 提交并推送 `main`；等 CI 通过后推送 tag `v${Cargo.toml version}`，
   由 `.github/workflows/release.yml` 构建 tar.gz 并创建 Release。
5. 到 Releases 页面核对资产大小与 digest，更新文档。**本地不需要 musl 打包。**

### B. 用户要求继续开发
- 新功能请看 `docs/API.md`、`docs/ARCHITECTURE.md` 与 `TODO.md`。
- 出站检测：`src/outbound_check.rs`、`src/api/outbound_test.rs`、
  `frontend-3xui/src/pages/settings/AdvancedTab.tsx`。
- 日志：`src/panel_log.rs`、`src/api/compat.rs` 的 `server_logs`、
  `frontend-3xui/src/pages/index/LogModal.tsx`。
- 在线 IP/位置：`src/xray/stats.rs`、`src/api/compat.rs` 的
  `clients_online_ips`、`src/api/ip_location.rs`、`ClientInfoModal.tsx`。
- IP数据云设置：`pages/settings/AdvancedTabs.tsx`（高级页内标签）、
  `IpDataCloudTab.tsx`、`/settings#advanced`、`AllSetting.ipDataCloudKey`、
  `src/api/ip_location.rs` 的 `ip_data_cloud_lookup`。
- 流量/速度：`src/jobs.rs`（`watching`、1 秒显示 tick、`speed_sample`、
  `display_deltas`、`encode_traffic_frame`、`broadcast_traffic`）、
  `src/api/ws.rs`、`src/state.rs` 的 `TrafficSpeeds`、
  `src/db/client_traffic.rs`、`src/api/compat.rs` 的
  `inbounds_slim`/`client_record`、前端 `useWebSocket.ts` /
  `useInbounds.ts` / `useClients.ts`；回归脚本：
  `tmp/e2e-traffic-accuracy/e2e.py`、`noclient.py`、
  `tmp/e2e-traffic-ui/ui_run.py`、`tmp/diag-traffic-ws/diag.py`
  （需先 `cargo build` 与 `tmp/fake_stats` 构建）。
- 每次功能完成按 TODO 验收标准自测，追加 `DEVELOPMENT-LOG.md`；未验证项必须
  写“已实现，未验证”。

## 当前悬而未决的验证项

- **T32（最高优先）**：真实 Linux/musl 部署验证 0.2.34 内核 socket 计数
  驱动的 tunnel 下行速度（连续非零、无一次性爆发、网卡对照、sock_diag 权限
  回退、重启后基线重置、CPU/RSS），见 `TODO.md`。
- **T33（P2）**：评估 http/mixed CONNECT 是否也纳入内核计数（当前仍会一次性
  爆发），见 `TODO.md`。
- **T31**：真实 Linux/musl 部署验证 0.2.32 原生速度快照（1 秒刷新、关闭
  面板 REST 速度为 0 但计费继续、CPU/RSS）。
- **T26**：真实 Linux + systemd + musl 回归增强（在线公网 IP、日志目录
  写入/14 天轮转、出站检测临时 Xray 启动、设置保存 PID 不变、restartPanel
  由 systemd 拉起）。
- **T27**：在真实面板部署网络验证位置查询（zxinc 可达性/准确性/耗时，
  ipwho.is 兜底），对照 tool.lu 的纯真或淘宝库。
- **T28**：用有效 IP数据云 Key 验证成功响应、区县/街道精度、额度错误与
  客户端两行显示（当前只用假 Key 验证了 `key不正确`）。
- **T21**：真实 Xray 对 11 种原生入站的 `run -test`/实际连接。
- **T22**：hysteria2/wireguard 服务端订阅链接。
- **T23/T24**：真实 systemd 在线更新、安装幂等与代理提示。
- **KNOWN-ISSUES H4/H5**：英文界面后端错误仍中文；Xray OnlineMap 丢弃回环
  IP；位置服务依赖外网。
- 既有问题：`npm run typecheck`/`npm run test` 因 devDependencies 缺失仍有
  既有报错（非本次改动引入）。
