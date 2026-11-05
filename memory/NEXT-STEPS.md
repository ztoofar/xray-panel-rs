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
   期望：`cargo test` 114/114、clippy 零警告、Cargo 版本 `0.2.28`、
   `web/dist` 不旧于前端源码；工作区除本次未提交改动外干净。
3. 用户说“继续开发”但未指定任务时：先汇报核对结果，再从 `TODO.md` P1 建议，
   等用户选择。**每次代码修改都 bump 版本；不要自动打包、不要自动打 tag/Release。**

## 当前状态（2026-11-05 深夜）

- 当前发布：GitHub **`v0.2.28`**（tag → `5e376a1`，资产/运行记录见
  `CURRENT-STATUS.md` §2.46）。内容：入站页 WS 速度过期保护——WS 半途
  断开超过 2×采样间隔（≤2 分钟）清空冻结速度，回退 REST 轮询速率；
  修复 0.2.25 起遗留的入站速度列断线冻结（`useClients.ts` 有该机制而
  `useInbounds.ts` 缺失）。详见 §2.45、DECISIONS D053、KNOWN-ISSUES G12。
- 包级复核已完成：包 sha256 与 GitHub digest、包内 `SHA256SUMS`、
  二进制 `LUMEN_VERSION=0.2.28` 均一致。
- 验证：`cargo test` **114/114**、clippy 零警告、fmt、前端 `npm run build`
  （Node 26，产物已嵌入 `web/dist/`）。
- **未验证**：真实 Linux/musl 运行与断线场景（代理掐长连接/睡眠恢复）；
  用户现场若 WS 存活时仍有幅度偏差，用 DevTools `__diagDump` 采集
  WS 帧 + REST 速率 + 系统状态网卡吞吐三方数据后再定位。
- 上一发布：`v0.2.27`（§2.44、D052）：WS 帧除数改真实采样间隔。

## 下一步（按用户任务二选一）

### A. 用户要求“打包/发布”
1. 确认 `Cargo.toml` / `Cargo.lock` 版本一致（当前 `0.2.26`；新改动须先按
   规则 bump），同步 `memory/CURRENT-STATUS.md`、`memory/DEVELOPMENT-LOG.md`。
2. 本地自测：前端 `npm run build`、`cargo fmt --check`、`cargo test`、
   `cargo clippy`。
3. 更新 `.github/workflows/release.yml` 的 `body`：标题固定 `## 更新内容`
   （不带版本号），只列本次更新条目；不写安装/更新说明，不启用
   `generate_release_notes`（D046/D047）。
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
- 流量/速度：`src/jobs.rs`（tick、`speed_sample`、`client_samples`、
  `traffic_speeds`、`broadcast_traffic`）、`src/api/ws.rs`（`/ws` 实时帧）、
  `src/db/client_traffic.rs`、`src/api/compat.rs` 的
  `inbounds_slim`/`client_record`、前端 `useWebSocket.ts` /
  `useInbounds.ts` / `useClients.ts`；回归脚本：
  `tmp/e2e-traffic-accuracy/e2e.py`、`noclient.py`（需先 `cargo build` 与
  `tmp/fake_stats` 构建）、`tmp/e2e-traffic-real/e2e.py`、
  `tmp/e2e-traffic-ui/ui_run.py`。
- 每次功能完成按 TODO 验收标准自测，追加 `DEVELOPMENT-LOG.md`；未验证项必须
  写“已实现，未验证”。

## 当前悬而未决的验证项

- **T26**：真实 Linux + systemd + musl 回归本次增强（在线公网 IP、日志目录
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
