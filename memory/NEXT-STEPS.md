# NEXT-STEPS

> 下一次会话从这里开始。写完本文件时假设读者没有本次会话的上下文。
> 最后更新：2026-11-03 23:05

## 第 0 步（每次会话必做，不可跳过）

1. 读 `CLAUDE.md` → `memory/CURRENT-STATUS.md` → 本文件。
2. **核对真实代码状态**（不要凭记忆）：
   ```sh
   cd /d/ClaudeCode/Projects/xray-panel-rs
   cargo test 2>&1 | tail -5
   cargo clippy --all-targets -- -D warnings 2>&1 | tail -3
   grep '^version' Cargo.toml
   ls migrations
   ls -la web/dist/index.html
   find frontend-3xui/src -newer web/dist/index.html -type f | head
   ```
   期望：`cargo test` **90/90**；clippy 零警告；Cargo 版本 `0.2.16`；migrations 到
   `0009_outbound_subscriptions.sql`；`web/dist` 不比前端源码旧。
   当前源码与 GitHub v0.2.16 发布包一致（CURRENT-STATUS §2.22/§2.23）；
3. 如果用户说“继续开发”但没有指定任务：先汇报核对结果，再从 `TODO.md` 的
   P1 中给出建议，等用户选择。**不要自动打包，不要自动 bump 版本。**

## 第 1 步（按用户任务二选一）

### A. 用户要求“打包/发布”
1. 当前已发布 **0.2.16**（协议补全、节点地址、在线更新、一键安装、GitHub
   Actions）。打新版本时同步 `Cargo.toml` 与 tag，推送 `v*` 标签由
   `.github/workflows/release.yml` 构建发布。
2. 本地只改代码并自测：前端构建、`cargo fmt --check`、`cargo test`、
   `cargo clippy`。**不需要本地 musl 打包。**
3. 修改 `Cargo.toml` 的 `version`，更新文档，提交并推送 `main`。
4. 等 main 的 CI 通过后推送 tag（tag 必须等于 `v${Cargo.toml version}`）：
   ```sh
   git tag v0.2.17 && git push origin v0.2.17
   ```
5. `.github/workflows/release.yml` 自动构建 musl、打包 tar.gz/原始二进制/
   SHA256SUMS 并创建 Release；到 Releases 页面核对资产与哈希。
6. 更新 `PROGRESS.md`、`memory/CURRENT-STATUS.md`、`memory/DEVELOPMENT-LOG.md`
   记录 Release 哈希。
7. 仅当用户明确要求本地备用包或 Actions 不可用时，才在本机执行
   `cargo zigbuild --release --target x86_64-unknown-linux-musl` 并手工组装
   `dist/`；正常发布不要做这一步。

### B. 用户要求继续某项开发
- 先读 `docs/` 对应文档与相关源码，确认现状。
- 负载均衡/路由：读 `docs/ARCHITECTURE.md` §5.2、`docs/DECISIONS.md` D028、
  `src/xray/router.rs`、`src/api/compat.rs` 的 balancer 处理器。
  路由测试已按用户要求移除，不要恢复。
- 更新功能：读 `src/updates.rs`、`docs/KNOWN-ISSUES.md` B8；
  真实 Xray 回归脚本 `tmp/e2e-outbound-subs/real-e2e.py`（依赖
  `tmp/xray-real/xray.exe` 和可用的更新代理；代理地址不写进代码）。
- 出站订阅：`src/outbound_subs.rs`、`src/xray/subscription.rs`、
  `tmp/e2e-outbound-subs/e2e.py`。
- 统计相关：`docs/ARCHITECTURE.md` §6、`docs/DATABASE.md`
  `client_inbound_traffic`、`docs/DECISIONS.md` D019。
- 按 `TODO.md` 的验收标准执行；每完成一项追加 `DEVELOPMENT-LOG.md`。
- 功能完成但未验证时，`CURRENT-STATUS.md` 必须写“已实现，未验证”及缺的验证项。

## 第 2 步（会话结束前）

1. 更新 `memory/CURRENT-STATUS.md`（已验证/未验证边界）。
2. 追加 `memory/DEVELOPMENT-LOG.md`（时间、改动、文件、测试结果）。
3. 更新 `memory/TODO.md`（完成/新增）。
4. 重写本文件为下一次可直接执行的步骤。
5. 向用户汇报真实测试结果；未打包的改动要明确说“未打包”。

## 当前悬而未决（下次可直接接手的候选）

- **已随 0.2.16 发布（2026-11-03）**：协议补全（CURRENT-STATUS §2.19）。
  11 种原生入站/13 种原生出站已打通；fake xray 端到端 + 无头 Chrome 通过。
  仍缺官方 `xray run -test`/真实连接（T21），服务端订阅链接对新协议为空（T22）。
- **已随 0.2.16 发布（2026-11-03）**：节点地址设置（CURRENT-STATUS §2.20）。
  设置页新增 `nodeAddress`，分享链接/二维码优先使用它；API + 无头 Chrome 验证
  通过。仍缺真实客户端连接与反代 Host 场景验证。
- **已随 0.2.16 发布（2026-11-03）**：面板在线更新、一键安装 `install.sh`、
  GitHub Actions `ci.yml`/`release.yml`；仓库 `ztoofar/xray-panel-rs`。
- **已完成（2026-11-02 00:42）**：功能整改（见 CURRENT-STATUS §2.14）已随
  **0.2.13** 打包（§2.15）：负载均衡状态/override、ECH/ML-DSA-65/ML-KEM、
  证书哈希、Xray 版本/Geofiles/GEOdata/更新代理，及一批入口/协议/假接口清理。
  发布包与源码包 SHA256 见本文件末尾与 CURRENT-STATUS §2.15。
- **已完成并随 0.2.14 打包（2026-11-02 01:11）**：Xray 安装下载顺序修复
  （§2.16）——先下载/校验/解压，再停止 Xray 并替换，避免本地代理入站随
  Xray 停止而导致 `Connection refused`。
- **已完成并随 0.2.14 打包（2026-11-02 01:11）**：Geofiles/systemd 只读路径
  修复（§2.17）——`ReadWritePaths` 增加 `/usr/local/bin`，否则 Geofiles 与
  Xray 二进制替换在 `ProtectSystem=full` 下报 `Read-only file system`。
- **P1-T19**：真实 Linux 上回归 2026-11-02 的功能（Xray 安装、Geofiles 路径/
  `XRAY_LOCATION_ASSET`、GEOdata 自动下载、systemd 重启方式）。
- **P1-T9**：v0.2.7 统计修复 + Xray 设置修复的真机回归（清单见 `TODO.md` /
  `KNOWN-ISSUES.md` G6）。
- **P1-T16**：出站订阅真实 Linux + 真实 Xray 回归（含 socks5 代理认证）。
- **P1-T1**：README 与代码不一致（轮询默认 5 秒/范围 2–300；`limitIp` 已实现）。
- **P1-T2**：面板路径的 HTTP 层自动化测试。
- **P1-T3**：前端 devDependencies 不完整导致 `npm run typecheck` / `npm run test`
  既有报错；本次删除/修改后的测试未执行。
- **P1-T14/T15**：客户端导入导出浏览器验证；登录失败微信通知真实 HTTPS 验证。
- **P2-T17**：出站订阅支持 JSON/Clash 正文与逐条坏出站过滤。
- **P2-T18**：出站订阅重定向的逐跳 SSRF 校验。
- **P2-T20**：清理活动页面中的 Hosts/Nodes 残留数据 plumbing（无对应管理页）。
- **P2-T13**：`subSortIndex`/`excludeFromSub` 已保存但订阅服务器未实现。
- 当前源码/发布版：**0.2.16**；GitHub `ztoofar/xray-panel-rs`，Releases 由 Actions 构建。
- GitHub Release `v0.2.16`：tar.gz 5,417,245 字节
  SHA256 `ad58e7da...4b3c`；二进制 12,742,128 字节 SHA256 `f1760246...ee83`；
  原始二进制与 SHA256SUMS 已附加，面板在线更新使用这两个资产。
- 上一版归档：`dist/xray-panel-rs-linux-amd64-lumen-0.2.15.tar.gz`
  （5,398,823 字节，SHA256 `40b4b245...fb13a`）；更早 `...-0.2.14.tar.gz` 等。
- 本地源码包：`dist/xray-panel-rs-0.2.16-src.tar.gz`（4,224,342 字节，
  SHA256 `2091acf5...7628`）；GitHub Release 另提供自动源码归档。
- 一键安装：`curl -fsSL https://raw.githubusercontent.com/ztoofar/xray-panel-rs/main/install.sh | sudo bash`。
