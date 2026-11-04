# NEXT-STEPS

> 下一次会话从这里开始。写完本文件时假设读者没有本次会话的上下文。
> 最后更新：2026-11-04 18:37

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
   期望：`cargo test` 107/107、clippy 零警告、Cargo 版本仍 `0.2.20`、
   `web/dist` 不旧于前端源码；工作区干净（2026-11-04 增强已提交）。
3. 用户说“继续开发”但未指定任务时：先汇报核对结果，再从 `TODO.md` P1 建议，
   等用户选择。**不要自动打包、不要自动 bump 版本。**

## 当前状态（2026-11-04）

- 已发布：GitHub `v0.2.20`（tag → `d6f7efa`，仅 tar.gz 资产）。
- 源码 `main` 已包含 **2026-11-04 七项增强**（CURRENT-STATUS §2.30）：
  客户端在线 IP/按需位置、后端错误中文化、Xray 短版本号、安全警告按登录周期、
  高级页出站真实连接检测 + 微信通知、面板日志中国时间/详细/历史、设置保存不
  重启 + 重启面板按钮。
- 验证：`cargo test` 107/107、clippy 零警告、fmt 通过、前端 build 成功；
  `tmp/e2e-advanced/e2e.py`、`ui-run.py`、`online-e2e.py` 全通过。
- **未打包、未 bump 版本**（仍 0.2.20）；tar.gz 尚不包含本次增强。

## 下一步（按用户任务二选一）

### A. 用户要求“打包/发布”
1. 递增 `Cargo.toml` / `Cargo.lock` 版本（预计 `0.2.21`），同步
   `memory/CURRENT-STATUS.md`、`memory/DEVELOPMENT-LOG.md`。
2. 本地自测：前端 `npm run build`、`cargo fmt --check`、`cargo test`、
   `cargo clippy`。
3. 提交并推送 `main`；等 CI 通过后推送 tag `v${Cargo.toml version}`，
   由 `.github/workflows/release.yml` 构建 tar.gz 并创建 Release。
4. 到 Releases 页面核对资产大小与 digest，更新文档。**本地不需要 musl 打包。**

### B. 用户要求继续开发
- 新功能请看 `docs/API.md`、`docs/ARCHITECTURE.md` 与 `TODO.md`。
- 出站检测：`src/outbound_check.rs`、`src/api/outbound_test.rs`、
  `frontend-3xui/src/pages/settings/AdvancedTab.tsx`。
- 日志：`src/panel_log.rs`、`src/api/compat.rs` 的 `server_logs`、
  `frontend-3xui/src/pages/index/LogModal.tsx`。
- 在线 IP/位置：`src/xray/stats.rs`、`src/api/compat.rs` 的
  `clients_online_ips`、`src/api/ip_location.rs`、`ClientInfoModal.tsx`。
- 每次功能完成按 TODO 验收标准自测，追加 `DEVELOPMENT-LOG.md`；未验证项必须
  写“已实现，未验证”。

## 当前悬而未决的验证项

- **T26**：真实 Linux + systemd + musl 回归本次增强（在线公网 IP、日志目录
  写入/14 天轮转、出站检测临时 Xray 启动、设置保存 PID 不变、restartPanel
  由 systemd 拉起）。
- **T21**：真实 Xray 对 11 种原生入站的 `run -test`/实际连接。
- **T22**：hysteria2/wireguard 服务端订阅链接。
- **T23/T24**：真实 systemd 在线更新、安装幂等与代理提示。
- **KNOWN-ISSUES H4/H5**：英文界面后端错误仍中文；Xray OnlineMap 丢弃回环
  IP；位置服务依赖外网。
- 既有问题：`npm run typecheck`/`npm run test` 因 devDependencies 缺失仍有
  既有报错（非本次改动引入）。
