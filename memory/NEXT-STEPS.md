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
   期望：`cargo test` 122/122、clippy 零警告、Cargo 版本 `0.2.32`、
   `web/dist` 不旧于前端源码；工作区除本次未提交改动外干净。
3. 用户说“继续开发”但未指定任务时：先汇报核对结果，再从 `TODO.md` P1 建议，
   等用户选择。**每次代码修改都 bump 版本；不要自动打包、不要自动打 tag/Release。**

## 当前状态（2026-11-06）

- **当前源码：`0.2.32`，未打包、未发布**（用户未要求发布）。最近发布仍是
  GitHub `v0.2.31`（tag → `f3e0509`，§2.51、D056）：1 秒显示节拍。
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
- 上一发布：`v0.2.30`（采样与计费解耦，tag `c3bd5b5`，§2.50）、
  `v0.2.29`（3x-ui 模型重写取数）、`v0.2.28`（入站 WS 过期保护）。

## 下一步（按用户任务二选一）

### A. 用户要求“打包/发布”
1. 确认 `Cargo.toml` / `Cargo.lock` 版本一致（当前 `0.2.32`；新改动须先按
   规则 bump），同步 `memory/CURRENT-STATUS.md`、`memory/DEVELOPMENT-LOG.md`。
2. 本地自测：前端 `npm run build`、`cargo fmt --check`、`cargo test`、
   `cargo clippy`。
3. 更新 `.github/workflows/release.yml` 的 `body`：标题固定 `## 更新内容`
   （不带版本号），只列本次更新条目；不写安装/更新说明，不启用
   `generate_release_notes`（D046/D047）。本次条目应写明：实时速度改为
   原生绝对速率快照；只有打开面板时计算速度；流量统计始终运行。
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
