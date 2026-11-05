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
   期望：`cargo test` 119/119、clippy 零警告、Cargo 版本 `0.2.29`、
   `web/dist` 不旧于前端源码；工作区除本次未提交改动外干净。
3. 用户说“继续开发”但未指定任务时：先汇报核对结果，再从 `TODO.md` P1 建议，
   等用户选择。**每次代码修改都 bump 版本；不要自动打包、不要自动打 tag/Release。**

## 当前状态（2026-11-06）

- **当前发布：GitHub `v0.2.30`**（tag → `c3bd5b5`，§2.50）：采样与计费解耦
  （§2.49、DECISIONS D055、KNOWN-ISSUES G14）——`src/jobs.rs` tick 显示路径
  （`traffic_speeds`/在线活跃/WS 广播）先跑完，原始计数器装 `BillingSample`
  推入 `BillingSink`（单槽合并、新样本取代旧样本、内存上界一个样本），独立
  tokio 任务 `run_billing_writer`→`apply_billing` 串行落库（计费/禁用/重置/
  心跳/出站/历史），失败只记 warn；epoch 不符的旧进程样本丢弃。修复 poll=2s
  大部署下速度列被计费尾工摊薄（用户报 tunnel「比 3x-ui 小很多」）。
- 发布复核：main CI `42067360254`、release `42068104145` 成功；包 sha256 与
  GitHub digest、包内 `SHA256SUMS`、`LUMEN_VERSION=0.2.30` 一致（§2.50）。
- 源码验证：`cargo test` **121/121**（含 2 个合并槽单测）、clippy 零警告、
  fmt；真实 Xray tunnel 与 fake-stats 两条本地链路 PASS。前端未改动，
  WS 帧协议与 0.2.25 一致。
- **现场验证完成（2026-11-06）**：SSH 登录现场服务器实测 v0.2.30（二进制
  13,000,368 字节）、`traffic_poll_secs=5`、Xray 26.10.31。`in-802-tcpudp`
  60s 对照：Xray 原始 3.7 KB/s+1.6 MB/s = WS 帧 1.6 MB/s = DB 计费
  1.6 MB/s = REST 均值 1.5 MB/s，帧间隔 5.0s = 除数 5.0s；空闲复核
  `upSpeed` 1422 B/s ≈ 原始 uplink 1760 B/s。journal 无 billing/stats
  警告。**结论：管线正确**——用户看到的 3.32K 是 ↑ 上行列（下载时上行
  只有请求/ACK），下载量在 ↓ 列（~1.5–1.8 MB/s）；监控 4 MB/s ≈ 代理
  两条腿（客户端侧+源站侧）+ 包头/TLS，面板只算单条腿 payload（§G14）。
- 可选后续：把速度标签从纯箭头改为「上传/下载」显式文案（需前端构建，
  等用户确认要不要）。
- 上一发布：`v0.2.29`（3x-ui 模型重写取数）、`v0.2.28`（入站 WS 过期保护）。

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
