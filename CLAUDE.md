# CLAUDE.md — xray-panel-rs / Lumen

本文件只放**不可违反的规则、项目核心约束和文档索引**。
历史细节放 `PROGRESS.md`，当前状态放 `memory/`，设计与决策放 `docs/`。

---

## 0. 会话启动流程（必须执行）

1. 读本文件 `CLAUDE.md`。
2. 读 `memory/CURRENT-STATUS.md`。
3. 读 `memory/NEXT-STEPS.md`。
4. 根据任务读 `docs/` 下相关文档（架构 / 数据库 / API / 决策 / 已知问题）。
5. **动手前先核对真实代码状态**：读相关源码、跑必要的测试或构建，再决定怎么做。

> **用户说“继续开发”时：必须先检查当前项目文件和记忆文档，确认真实代码状态，再继续工作。不能仅凭历史记忆猜测代码实现情况。**
> 如果记忆与代码冲突，以代码、测试和构建结果为准，并立即修正记忆文档。

---

## 1. 不可违反的开发规则

1. **未验证的功能不得标记为已完成。** 只有跑过对应测试/构建/手工验证，才能在
   `CURRENT-STATUS.md` 写“已完成”。无法验证时写“已实现，未验证”并说明缺什么。
2. **不要每次改动后都打包。** 改完只做自测（`cargo test`、`cargo clippy`、
   前端构建、必要的无头浏览器验证）。只有用户明确要求“打包/发布”时，才生成
   Linux musl 二进制和 `dist/*.tar.gz`。
3. **版本号只在正式打包时更新**（`Cargo.toml` 的 `version`）。未打包的源码改动
   不 bump 版本；打包时同步更新 `PROGRESS.md` / `memory` 中的版本和 SHA256。
4. **历史只追加，不覆盖。** `docs/DECISIONS.md`、`memory/DEVELOPMENT-LOG.md`
   和 `PROGRESS.md` 的历史条目不得删除或改写；新决定用新条目编号追加。
5. **每完成一个功能就更新对应文档**：架构变化 → `docs/ARCHITECTURE.md`；
   表结构 → `docs/DATABASE.md`；接口 → `docs/API.md`；取舍 → `docs/DECISIONS.md`；
   遗留问题 → `docs/KNOWN-ISSUES.md`。
6. **每次准备结束会话时**，主动更新 `memory/CURRENT-STATUS.md`、
   `memory/DEVELOPMENT-LOG.md`、`memory/TODO.md`、`memory/NEXT-STEPS.md`。
7. **不要把历史内容堆进本文件。** 本文件保持短小，只放规则和索引。
8. **不要伪造兼容 API 的成功。** 未实现的功能必须返回明确错误
   （`success:false` 或相应状态码），不要返回假数据让 UI 误以为成功。
9. **Xray 配置由面板生成和托管。** 修改配置必须走“生成 → 临时文件
   `xray run -test` → 备份 → 原子替换 → 重启”的流程；测试失败绝不能覆盖线上配置。
10. **不要同时用 systemd 启动第二个 Xray。** 本机 Xray 由面板作为子进程监管，
    否则 API 端口/入站端口会冲突。
11. **低内存是硬约束。** 新增依赖、缓存和常驻结构前先评估内存；面板空闲
    RSS 目标是个位数到二十几 MB（不含 Xray）。不要引入 ORM、重型后台服务或
    无上限内存结构。
12. **前后端构建顺序：先构建前端，再编译 Rust。** `rust-embed` 在编译期把
    `web/dist/` 嵌入二进制；只改前端源码不重新构建，界面不会变化。
13. **保持中文/英文双语文案可编译。** 新增 i18n key 必须存在于
    `internal/web/translation/*.json`（全部 locale）或被前端 fallback 覆盖；
    不要只改一个语言文件就结束。
14. **不要在文档里写猜测。** 不确定的行为先读代码或实测；写在
    `KNOWN-ISSUES.md` 的“未验证/风险”里，而不是写成事实。

---

## 2. 项目核心约束与事实

- **定位**：Linux amd64 单机、低内存的 Xray 控制面板；Rust + Axum + SQLite；
  Xray 作为面板子进程运行，由面板生成并托管其配置。
- **目标平台**：`x86_64-unknown-linux-musl` 静态 ELF（已 strip），无 glibc 依赖。
  开发/自测在 Windows 上进行。
- **当前源码版本**：`0.2.16`（以 `Cargo.toml` 为准），随 GitHub Release 发布。
  0.2.16 包含：11 种 Xray 原生入站/13 种原生出站协议补全、面板「节点地址」、
  面板在线更新（GitHub Releases + SHA256 校验 + systemd 重启）、一键安装脚本与
  GitHub Actions 构建发布。见 `memory/CURRENT-STATUS.md` §2.14–§2.23。
- **当前生效前端**：`frontend-3xui/`（3x-ui 前端的裁剪版），构建输出到 `web/dist/`。
  只暴露七个路由：系统状态 `/`、入站、客户端、出站、路由、Xray、设置。
- **非生效目录**：`frontend/`（早期自研表单源码）和 `web/dist-custom-v2/` 保留备查，
  不参与当前构建；不要误改它们。
- **后端兼容层**：`src/api/compat.rs` 实现 3x-ui 前端所需的 `/panel/api/*` 接口。
  已实现：出站订阅（`src/outbound_subs.rs` + `src/xray/subscription.rs`）、
  负载均衡实时状态/手动切换（`src/xray/router.rs`，RoutingService gRPC）、
  ECH/ML-DSA-65/ML-KEM 生成、证书 SHA-256、Xray 版本/Geofiles 更新与更新代理
  （`src/updates.rs`）。未实现：2FA、邮件、订阅服务器、节点、HWID 日志。
  已移除入口：路由测试、REALITY 目标扫描、系统历史、面板自更新、客户端 IP 日志、
  Happ/订阅链接 UI。入站支持 11 种 Xray 原生协议（含 WireGuard/Hysteria/
  Tunnel/TUN/MASQUE）；MTProto/AmneziaWG/TUIC 依赖 sidecar，不提供。
- **前端实时通道**：3x-ui WebSocket 已禁用，全部改为轮询 + 操作后刷新。
- **单管理员**：只有一个用户；Argon2id 口令哈希、会话 Cookie、CSRF、登录限流。
- **面板不提供 TLS/证书管理**；公网部署必须自行加 HTTPS 反代并设置
  `PANEL_SECURE_COOKIE=true`。
- **运行参数优先级**：`PANEL_DB`、`PANEL_LOG`、`PANEL_WORKERS` 只在环境变量；
  监听地址/端口、Xray 路径、API 地址、日志路径、安全 Cookie 可从面板设置
  （数据库）覆盖，重启后生效；面板路径（`webBasePath`）保存后立即生效。
- **面板路径**：`/` 或 `/xxx/`，保存后旧地址立即 404；`/api/health` 始终保留在根路径。
- **内存维护**：每 15 分钟清理过期会话、`PRAGMA shrink_memory`、收缩内部容器；
  glibc 下额外 `malloc_trim(0)`。musl 的 mallocng 自行归还空闲页，没有 `malloc_trim`。

### 明确不做

多节点、订阅服务器、Telegram/Discord/邮件、MTProto、AmneziaWG、TUIC、MASQUE、
WARP/Nord/PIA、SSL 证书管理、面板自更新。详细见 `README.md` 与
`docs/KNOWN-ISSUES.md`。

---

## 3. 构建、测试与验证命令

Windows 开发机上的实际命令（路径按本机固定）：

```sh
# 前端（需要 Node >= 26）
cd frontend-3xui
export PATH="/d/ClaudeCode/Tools/node-v26.13.0-win-x64:$PATH"
npm run build                 # 输出到 ../web/dist

# 后端（libsqlite3-sys/ring 需要 C 编译器）
cd ..
export PATH="/d/ClaudeCode/Tools/mingw64/bin:$PATH"
cargo test                    # 单元/集成测试（当前 89/89）
cargo clippy --all-targets -- -D warnings

# Linux musl 交叉编译（不打包，仅验证）
export PATH="/d/ClaudeCode/Tools/zig-npm/node_modules/@zigc/win32-x64/bin:$PATH"
cargo zigbuild --release --target x86_64-unknown-linux-musl
```

GitHub 发布：同步 `Cargo.toml` 版本后推 `v*` 标签，`.github/workflows/release.yml`
自动构建并上传 tar.gz、原始二进制与 SHA256SUMS 到 Releases。

注意：

- `npm run typecheck` 当前会因 `node_modules` 缺少 vitest/storybook 等
  devDependencies 报既有错误；判断自己的改动时，以**改动文件没有新增报错**为准，
  并在 `KNOWN-ISSUES.md` / `TODO.md` 跟踪。
- 打包发布（只在用户要求时）：构建前端 → musl release → 生成
  `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（含二进制与 `install.sh`）→
  记录 SHA256 并更新文档。

---

## 4. 记忆与文档索引

| 文件 | 内容 | 何时读 | 何时写 |
| --- | --- | --- | --- |
| `memory/CURRENT-STATUS.md` | 当前真实状态、已完成/未完成、验证边界 | 每次会话开始 | 每次结束/功能完成 |
| `memory/DEVELOPMENT-LOG.md` | 每次开发的时间、改动、文件、测试结果 | 需要回溯时 | 每次开发结束 |
| `memory/TODO.md` | 待开发任务、优先级、验收标准 | 规划任务时 | 状态变化时 |
| `memory/NEXT-STEPS.md` | 下一次会话的具体执行清单 | 每次会话开始 | 每次会话结束 |
| `docs/ARCHITECTURE.md` | 进程/模块/请求/Xray/前端架构 | 改动架构前 | 架构变化时 |
| `docs/DATABASE.md` | 表结构、迁移、时间单位、不变量 | 改表/查询前 | 迁移或字段变化时 |
| `docs/API.md` | 原生 API 与 3x-ui 兼容 API 约定 | 改接口/前端前 | 接口变化时 |
| `docs/DECISIONS.md` | 技术决策、原因、被否方案、后果（追加式） | 做设计选择前 | 新决策时 |
| `docs/KNOWN-ISSUES.md` | 已知限制、风险、未验证项 | 接手任务时 | 发现/解决时 |
| `PROGRESS.md` | 记忆系统建立前的详细历史长日志 | 需要深挖历史时 | 可继续追加，但不再作为主入口 |

---

## 5. 会话结束检查清单

- [ ] `memory/CURRENT-STATUS.md`：更新已验证/未验证边界，不夸大。
- [ ] `memory/DEVELOPMENT-LOG.md`：追加本次时间、改动、文件、测试结果。
- [ ] `memory/TODO.md`：完成项移走/标记，新发现问题加入。
- [ ] `memory/NEXT-STEPS.md`：写成下一次可直接执行的具体步骤。
- [ ] 改到架构/数据库/API/决策/限制时，同步更新 `docs/` 对应文件。
- [ ] 没有用户明确要求时，**不打包、不 bump 版本**。
- [ ] 向用户汇报测试真实结果（失败就写失败和输出）。
