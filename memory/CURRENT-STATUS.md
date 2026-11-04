# CURRENT-STATUS

> 最后更新：2026-11-04 18:37（本地时间）
> 本文件只记录**当前真实状态**。历史细节见 `DEVELOPMENT-LOG.md` / `PROGRESS.md`。
> 未验证的能力不得写成“已完成”；每项后面标注验证方式或“未验证”。

## 1. 版本与构建状态

| 项目 | 状态 |
| --- | --- |
| `Cargo.toml` 版本 | `0.2.20` |
| 最近发布 | GitHub `ztoofar/xray-panel-rs` Release `v0.2.20`：唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` 5,426,606 字节，SHA256 `61f1a69f...9269`（GitHub asset digest） |
| 当前源码 | `0.2.20` + 2026-11-04 功能增强（已提交，未打包；tag `v0.2.20` 仍指向 `d6f7efa`） |
| 上一版 | GitHub Release `v0.2.19`：tar 5,424,758 字节，asset digest `sha256:ab81a7c7...35c6` |
| 0.2.20 SHA256 | `61f1a69f4211cc0b7aab095e4a6895dae689f87ade20f8448fead03e0f909269`（tar.gz，GitHub asset digest）；包内 `SHA256SUMS` 校验解压后的二进制 |
| 源码包 | GitHub Release 自动提供 source 归档；本地 0.2.16 源码包保留在 `dist/` 仅作备份 |
| 前端构建产物 | `web/dist/` 已在 2026-11-04 18:2x 由当前 `frontend-3xui/` 源码重建（含本次增强） |
| 当前生效前端 | `frontend-3xui/`，七个路由：`/`、入站、客户端、出站、路由、Xray、设置 |
| musl 产物 | 0.2.20 Release tar 内含静态 ELF 二进制；本地不生成备用二进制 |
| 运行验证环境 | Windows debug + fake xray 端到端；真实 Xray 26.10.31（Windows）+ 无头浏览器；**未在真实 Linux 上运行 musl 二进制** |
| 记忆系统 | 已启用：根 `CLAUDE.md` + `memory/` + `docs/`（2026-10-31 建立） |

## 2. 本次发布新增（2026-10-31，已随 0.2.6 打包）

### 2.1 面板路径可自定义 —— 已验证

- 设置页「常规 → URI 路径」新增 `webBasePath` 输入框，格式 `/xxx/`。
- 保存后立即生效（无需重启）；浏览器自动跳转新地址；旧地址立即 404。
- 前缀覆盖 SPA、静态资源、登录、带前缀 API、独立 `/settings` 页；`/api/health`
  始终保留在根路径。
- 验证方式：`cargo test` 中的 `web::base_path` 单元测试 4 个；无头 Chrome 实测
  修改 `/secretui/` 后跳转、旧路径 404、静态资源/API/独立设置页正常、改回 `/` 恢复。
- 代码位置：`src/web/base_path.rs`、`src/api/mod.rs`、`src/web/mod.rs`、
  `src/api/compat.rs`、`frontend-3xui/src/pages/settings/{GeneralTab,SettingsPage}.tsx`。

### 2.2 版本号只在系统状态页 —— 已验证

- 删除侧边栏品牌区/抽屉中的面板版本号及对应 CSS。
- 版本号只保留在「系统状态」页（概览操作栏版本按钮、更新弹窗）。
- 验证方式：无头 Chrome DOM 断言 `.brand-version`、`.sider-version` 不存在，
  `.ov-panel-version` 在系统状态页存在；控制台无报错。

### 2.3 面板内存定期释放 —— 已验证

- 每 15 分钟：清理过期会话、`PRAGMA shrink_memory`、收缩在线/活跃/日志/
  流量历史/登录限流容器；glibc 下额外 `malloc_trim(0)`。
- musl 的 mallocng 自行归还空闲组，无 `malloc_trim`。
- 验证方式：临时把周期改为 5 秒运行 debug 构建，日志连续出现
  `maintenance: released idle panel memory`，无 SQLite/panic 报错；随后已恢复 15 分钟。
- 代码位置：`src/main.rs`（`spawn_maintenance` / `release_memory` / `trim_heap`）。

## 2.4 流量统计可靠性修复（2026-10-31）—— 已随 0.2.7/0.2.8 打包，部分验证

来源：ChatGPT 对 0.2.6 源码的静态审查（8 个问题）+ 本会话逐条核对真实代码后的实施。
已随 `0.2.7` 打包。

已实现：

- 新增 `migrations/0007_client_inbound_traffic.sql`：per-attachment 基准表。
- 新增 `src/traffic.rs`：`counter_delta(Option<i64>, last)` —— 缺失/负数保留基准，
  计数器回绕按当前值计新增。
- 新增 `src/db/client_traffic.rs`：
  - `apply_tick`：一个事务内按 attachment 读基准→算增量→写基准→汇总进
    `clients.up/down`，失败整体回滚。
  - `reset_all_baselines`：generation 变化时清 `last_*`，不动累计流量。
  - `reset_client_verified`：仅在全部 Xray 计数器确认重置后清累计+基准。
  - `mark_reconcile`：部分重置失败时标记，下一份样本只接管基准不计增量。
- `src/jobs.rs`：
  - 用 `XrayHandle::start_epoch()` 判断“新进程启动”，启动后、采样前重置基准；
    generation 变化只清在线集合/在线 API 探测缓存。`start_epoch == 0`（无子进程）
    时保留基准，兼容残留进程。
  - 按 `<inbound_id>:<email>` 逐 attachment 计算；裸邮箱外部配置保留旧聚合路径。
  - 客户端/入站/出站都消费 `Option`，同时比较上/下行。
  - 定时客户端/入站重置失败时不再清零数据库。
  - 在线 API 返回的未知身份直接丢弃。
- `src/xray/supervisor.rs`：新增 `start_epoch`（只在子进程成功启动时增加），
  供流量 job 精确判断基准失效时机。
- `src/xray/stats.rs`：`invalidate_online_api_probe()`，generation 变化时调用。
- `src/api/backup.rs`：备份/恢复表清单加入 `client_inbound_traffic`。
- 文档：`docs/ARCHITECTURE.md`、`docs/DATABASE.md`、
  `docs/DECISIONS.md`（D019）、`docs/KNOWN-ISSUES.md`（G1–G6）。

验证结果（Windows debug）：

- `cargo test`：统计修复新增 8 个测试，当时 **48/48** 通过（含 in-memory SQLite
  事务级测试）；补齐事务/在线重探/缺失方向测试后当前总数 **56/56**，见 2.5。
- `cargo clippy --all-targets -- -D warnings`：零警告。
- **未验证**：真实 Debian + 真实 Xray 上的重启/部分重置/gRPC 超时故障注入；
  在线 API 探测失效无独立单测。见 `KNOWN-ISSUES.md` G6 和 `TODO.md` T9。

## 2.5 日志查看器（2026-10-31）—— 已随 0.2.8 打包，后端已验证

- 面板日志：新增 `src/panel_log.rs`。`tracing` 使用两个 fmt layer：stdout 保持
  原样，第二个（无 ANSI）写入最多 500 行的内存环形缓冲；维护任务会收缩它。
- 接口：沿用 `POST /panel/api/server/logs/{rows}`：
  - `source=panel` → 面板 tracing 环形缓冲；
  - `source=xray&file=error|access` → 读取 Xray 配置 `log.error` / `log.access`
    指向的文件末尾，返回 `{kind, configured, path, exists, fallback, lines}`；
  - 不带 `source` → 保持旧行为（Xray 进程日志）。
- 文件读取：相对路径按面板 cwd 解析（Xray 子进程继承同一 cwd）；tail 最多约
  1 MiB；error 未配置时回退到 supervisor 捕获的 stdout/stderr。
- 前端 `LogModal`：单窗口，下拉切换“面板日志 / Xray 错误日志 / Xray 访问日志”，
  **默认面板日志**；日志按时间**倒序**显示（最新在上），下载仍按时间正序；
  移除了从未生效的 level / SysLog 控件；保留行数、自动刷新、下载。
- i18n：新增 `panelLogs`、`xrayLogs`、`logNotConfigured`、`logFileMissing`、
  `logProcessFallback`，写入 en-US / zh-CN / zh-TW；其余 locale 走 en-US 回退。
- 登录审计：`/api/login` 与 `/login` 都记录登录成功（INFO）、失败（WARN）、
  限流（WARN）、登出（INFO）和改密（INFO），包含用户名与来源 IP。两个入口现在
  共用同一 per-IP 限流器；未知用户名也做等时 Argon2 校验。
- 验证：
  - `cargo test` 56/56；clippy 零警告；`cargo build` 成功。
  - `frontend-3xui` 的 `npm run build` 成功，`web/dist` 已更新（release/musl
    构建在编译期嵌入；debug 构建运行时从磁盘读取）。
  - 新增测试：事务失败整体回滚、在线 API 探测缓存失效后强制重连、
    入站/出站缺失方向保留基准（`direction_deltas`）、下载方向缺失的对称场景。
  - 真实进程冒烟测试（临时 DB + 登录 + CSRF）：
    `source=panel` 返回真实 tracing 行；临时写入的 error.log/access.log 分别
    返回对应内容，路径/configured/exists 字段正确。
  - 登录审计冒烟测试：错误登录、原生/兼容登录成功、登出均出现在面板日志中；
    连续 5 次错误后第 6 次 `/login` 返回 “too many login attempts”。
- **未验证**：无头浏览器打开弹窗验证下拉/换行；其余 11 个 locale 显示英文回退。
  见 `KNOWN-ISSUES.md` H3。

## 2.6 科幻主题（2026-10-31）—— 已实现，截图验证；已随 0.2.9 打包

- 默认深色：`dark-mode-v2` 存储键一次性切换；深空蓝黑底 + 霓虹青主色
  `#22d3ee` + 青色网格/辉光；浅色为冰蓝变体（primary `#0891b2`）。
- AntD token：新增 light/dark/ultra 三套背景、边框、Layout、Menu、Card 颜色；
  新增 `src/styles/sci-fi.css`（侧栏、卡片、表格、按钮、输入框、弹窗、Tabs、
  滚动条）；登录页配色同步。
- 默认深色下 `.ant-btn-primary` 为青色渐变+辉光；侧栏品牌字为渐变；选中菜单
  有青色指示条。
- 验证：`npm run build`、oxlint 通过；无头 Chrome 截图
  `tmp/sci-fi-shot/{login-dark,panel-dark,inbounds-dark,inbounds-light}.png`。
- 本地预览：`127.0.0.1:28090`（debug 构建 + 临时 DB，admin/admin）；
  用户预览后表示“先这样”，主题保持当前状态，**未打包**；预览已于 22:33 关闭。
- **已打包**：随 0.2.9 发布（`dist/...0.2.8` 包不含本次主题改动）。

## 2.7 Xray 设置 / 入站操作修复（2026-11-01）—— 已验证，已随 0.2.9 打包

来源：用户反馈“连接限制修改后不生效”，并要求核对入站/出站/路由/Xray 设置模块。

根因与修复：

- `POST /panel/api/xray/update` 之前只保存固定白名单，`policy`（连接限制、
  统计开关）与 `metrics` 被静默丢弃。现在整份保存可编辑模板，只忽略面板托管的
  `inbounds`/`api`/`stats`；`policy`、`metrics` 及未来顶层块都会持久化。
- `ensure_policy` 之前强制覆盖所有统计开关。现在只强制每个 level 的
  `statsUserOnline`，缺失时补 uplink/downlink 默认 true，显式 false 保留。
- `GET /panel/api/xray/` 现在返回“生成配置去掉 inbounds/api/stats”，与 3x-ui
  模板语义一致；高级编辑器移除无效的 Inbounds 视图（保留 全部/出站/路由规则）。
- 两个 `getDefaultJsonConfig` 同时接受 GET/POST，返回真正的
  `DEFAULT_EXTRA_JSON`；重置按钮不再是 405/返回当前配置。
- 保存后按新 outbounds 清理已删除 tag 的 `outbound_traffics` 行。
- 迁移 `0008_inbound_meta.sql`：inbounds 增加 `sub_sort_index`、
  `exclude_from_sub`、`share_addr_strategy`、`share_addr`、`disable_flow`。
  兼容/原生入站 API 均读写；`disableFlow` 在生成 VLESS 客户端和分享链接时去掉
  flow（前端导出链接同步处理）。
- 实现 `POST /panel/api/inbounds/import` 与 `GET /panel/api/inbounds/allLinks`。
  导入支持 3x-ui 单个 inbound JSON、入站累计流量、clientStats 流量、共享邮箱保留
  第一次导入计数、强制启用客户端；allLinks 尊重 shareAddrStrategy/shareAddr。
  allLinks 覆盖 vless/vmess/trojan/shadowsocks；WireGuard/AmneziaWG 仍用单入站导出。
- 入站列表/详情的 clientStats 改为按 `client_inbounds` 关联，修复多入站客户端
  只在主入站出现的问题。
- 备份恢复改为按两库共有列复制，旧 schema 备份（含 0007 及更早）可恢复，新列
  取默认值。
- 前端：`saveAll` 接受本次处理后的模板，避免 balancer 清理后仍保存旧字符串；
  路由测试/balancer override 失败时显示明确错误。

验证（Windows debug + fake xray；不是真实 Xray）：

- `cargo test` **61/61** 通过（新增 5 个测试）；`cargo clippy --all-targets -- -D warnings`
  零警告；`cargo fmt --check` 通过。
- 前端 `npm run build` 成功，`web/dist` 已更新。
- 全新临时 DB 的端到端脚本覆盖：连接限制/统计开关/metrics 保存与回读、运行时
  config.json 含 `connIdle=123`/`bufferSize=64`/`statsOutboundUplink=false`/metrics、
  allLinks 自定义地址与 disableFlow、入站元数据更新、多入站关联视图、出站/路由
  保存、重置默认、恢复、重复/非法导入错误。
- 共享客户端导入：第二次导入不会覆盖第一次的流量，且强制启用。
- 旧 schema 备份（删除 0008 五列后）通过 `/panel/api/server/importDB` 恢复成功，
  新列取默认值。
- 无头 Chrome：连接限制页显示已保存的 123/64；高级视图只剩“全部/出站/路由规则”。
- 前端 `npm run test` / `typecheck` 仍因 devDependencies 缺失无法运行（既有问题 E1）；
  新增的 `inbound-from-db` disableFlow 测试已写入但本次未执行。
- **未验证**：真实 Xray 对 policy/metrics 的 `run -test` 与运行行为；真实 Linux 回归。

## 2.8 0.2.9 发布打包（2026-11-01）—— 已验证

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.9`。
- 内容：科幻主题 + 2026-11-01 Xray 设置/入站修复（policy/metrics 持久化、
  连接限制、默认配置、入站导入/allLinks/元数据、迁移 0008、旧备份恢复兼容）。
- 构建：`cargo test` 61/61、clippy 零警告、`npm run build` 成功、
  `cargo zigbuild --release --target x86_64-unknown-linux-musl` 成功。
- 发布目录：`dist/xray-panel-rs-linux-amd64-lumen/`（二进制 11,682,224 字节，
  静态 ELF、已 strip；含 install.sh/service/env/README/SHA256SUMS）。
- 发布归档：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4,597,175 字节，
  SHA256 `a80c70c83ac02e9c2f005dd8fce4cccae057e8f0796ed6106476d1d868cd1caa`）；
  二进制 SHA256 `850be8d3faf0174cff21869b493b9aa73cf1930c9229686ba515c8a0cd171993`。
- 上一版归档已保留为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.8.tar.gz`。
- 验证：`gzip -t` 通过；解开后 `sha256sum -c SHA256SUMS` 通过；
  `cmp` 包内二进制与交叉编译产物一致；`file` 静态 ELF/stripped；二进制内含
  `0.2.9`、`0008_inbound_meta`/`sub_sort_index`、`Panel Logs`、`connectionLimits`。
- **未验证**：未在真实 Linux 上执行该 musl 二进制；未在真实 Xray 上验证
  policy/metrics 的 `run -test` 与运行行为。
- 源码包：`dist/xray-panel-rs-0.2.9-src.tar.gz`（4,573,246 字节，SHA256
  `b16461bff832d41131482392034c641109c5d502192c6484cd0bd8b7c8ce234e`）。
- 升级说明：替换二进制即可；首次启动自动执行迁移 0008；旧备份仍可恢复，
  新列取默认值。

## 2.9 入口精简 + 客户端导入导出（2026-11-01）—— 已验证，未打包

- 移除入站菜单「删除所有客户端」「将客户端添加到分组」；移除客户端页「批量添加客户端」。
  同时删除 `AddClientsToGroupModal`、`ClientBulkAddModal` 及其测试，
  并删除 14 个 locale 中对应的 4 个 `pages.inbounds` 文案 key。
- 后端 3x-ui 兼容接口保留：`inbounds/{id}/delAllClients`、`clients/bulkCreate`、
  `clients/groups/bulkAdd`（DECISIONS D022）。
- 修复客户端导入/导出：
  - `GET /panel/api/clients/export` 返回 `[{client, inboundIds, traffic}]`，
    保留协议凭据（uuid/password/flow/subId/security/alterId/user/pass）、
    配额/到期/启用/分组/备注与 `up`/`down`/`lastOnline`。
  - `POST /panel/api/clients/import` 注册并实现：按 `inboundIds` 创建关联客户端，
    恢复凭据/属性/流量；重复邮箱跳过；空 `inboundIds` 明确跳过（schema 不支持
    无入站客户端）；最后统一 apply 一次。
  - 新增 `db::clients::set_traffic`；导出/导入逻辑与工具函数在 `src/api/compat.rs`。
- 验证（Windows debug + fake xray + 临时 DB）：
  - `cargo test` **61/61**；clippy 零警告；`cargo fmt` 通过。
  - 前端 `npm run build` 成功；改动文件 `tsc --noEmit` 无新增错误（Storybook 既有报错仍在）。
  - `tmp/e2e-clients-io/e2e.py` 全通过：多入站/启用禁用/凭据/流量导出校验、
    重复导入跳过、删除、坏 JSON/非数组/空 inboundIds/空邮箱错误路径、
    导入 created=2、二次导出逐字段一致。
- **未验证**：真实 Linux + 真实 Xray 上导入后的运行行为；无头浏览器点击弹窗；
  3x-ui 无入站客户端格式不支持（明确跳过）。
- 未打包、未 bump 版本。

## 2.10 0.2.10 发布打包（2026-11-01）—— 已验证

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.10`（0.2.9 的补丁递增）。
- 内容：
  1. 入站菜单移除「删除所有客户端」「将客户端添加到分组」；客户端页移除「批量添加」。
  2. 修复客户端导出/导入：真实 `GET /panel/api/clients/export` 与
     `POST /panel/api/clients/import`，保留凭据/关联/流量（见 §2.9）。
  3. 移除菜单栏全局搜索（Command Palette / Ctrl+K），删除组件、样式、测试与
     `commandPalette.*` 文案；页面内列表筛选搜索保留（DECISIONS D024）。
- 构建：`cargo test` 61/61、clippy 零警告、`cargo fmt --check` 通过；
  前端 `npm run build` 成功；musl release 构建成功。
- 发布目录：`dist/xray-panel-rs-linux-amd64-lumen/`（二进制 11,635,888 字节，
  静态 ELF、已 strip；含 install.sh/service/env/README/SHA256SUMS）。
- 发布归档：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4,589,521 字节，
  SHA256 `543dec0039915a8eda53b5037d2a019757880ffb3a96234902e37e5b91e7d268`）；
  二进制 SHA256 `d5036a05dc10cd7c4fc4f8898a0ffd65f359e5c2632c5e90e8336e761f03fb62`。
- 0.2.9 归档保留为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.9.tar.gz`。
- 源码包：`dist/xray-panel-rs-0.2.10-src.tar.gz`（4,542,738 字节，SHA256
  `0c63cd697016730d157cc99e5417975238d99b36b0873fa4e21842ce5dc1ba70`）。
- 验证：`gzip -t`、解包 `sha256sum -c SHA256SUMS`、`cmp` 包内二进制与交叉编译
  产物、`file` 静态 ELF/stripped、二进制内含 `0.2.10` 与本次功能串。
- **未验证**：未在真实 Linux 上运行该 musl 二进制；未在真实 Xray 上验证客户端
  导入后的运行行为；未做无头浏览器点击验证。

## 2.11 登录失败微信通知 + 0.2.11 打包（2026-11-01）—— 已验证（本地 mock）

- 需求：密码错误时通过微信 webhook 发送尝试账户与真实 IP；成功登录不发送；
  webhook 地址在面板设置配置。
- 实现：
  - 新增 `src/notify.rs`：失败登录异步发送企业微信群机器人
    `{"msgtype":"text","text":{"content":...}}`；内容含账户、真实 IP、时间、入口；
    账户名清理控制字符并截断。成功登录、空凭据、限流不发送。
  - 两个登录入口均接入：`POST /api/login` 与 `POST /login`。
  - 真实 IP 顺序：`X-Real-IP` → `CF-Connecting-IP` → `X-Forwarded-For` 首项 →
    TCP 对端。
  - 设置项 `loginNotifyWebhook` 存于 `x3ui_settings`；设置页「面板设置」新增输入框，
    保存时校验 `http://` / `https://`，空值关闭。
  - HTTP 客户端：`ureq` + rustls（静态 musl 可用）；每次失败新建短超时连接。
- 验证（Windows debug + 本地 HTTP mock + 临时 DB）：
  - `cargo test` **63/63**（新增 real_ip / clean_field 单测）；clippy 零警告；fmt 通过。
  - 前端 `npm run build` 成功；改动文件 `tsc --noEmit` 无新增错误。
  - 端到端 `tmp/webhook-e2e/e2e.py` 全部通过：原生失败通知（X-Real-IP）、
    兼容失败通知（X-Forwarded-For 首项）、成功登录不发送、空凭据不发送、
    设置读写、非法 URL 拒绝、清空后关闭。
- 打包：版本 0.2.10 → `0.2.11`；归档 5,217,711 字节，
  SHA256 `3bae78cd49e64f8f6810f74890398260126ec91c7655f3ab2536f829e9e0e755`；
  二进制 12,782,992 字节，SHA256
  `618caef970ad9435f1d7bfe2b2e181f72d2d381246c6150696ffc1df57f8b655`；
  源码包 4,551,487 字节，SHA256
  `79d0a719efc765086a71d2937242f5441d73aa9f4f59748515e51fe3bb6ad15d`。
- **未验证**：真实企业微信群机器人 HTTPS 与消息格式（`KNOWN-ISSUES.md` L1）；
  未在真实 Linux 运行；真实 IP 头可伪造（L2）。

## 2.12 出站订阅（2026-11-01）—— 已实现并本地端到端验证；已随 0.2.12 打包

用户反馈“出站订阅功能无效”，要求完善并测试；追加要求：更新订阅可选代理、
出站页“更多”菜单只保留导入/导出。

实现：

- 迁移 `0009_outbound_subscriptions.sql`：URL、启停、`proxy_url`、
  `allow_private`/`allow_insecure`、UA、`tag_prefix`、`update_interval`、
  `priority`、`prepend`、`last_updated`、`last_error`、原始 outbounds 与
  identity 映射。
- `src/xray/subscription.rs`：base64/纯文本订阅正文；解析 vmess/vless/trojan/
  ss/hysteria2/wireguard 链接（传输 ws/grpc/httpupgrade/xhttp/tcp/kcp、TLS/
  REALITY、finalmask、ss obfs-local）；identity 生成与标签稳定分配。
- `src/outbound_subs.rs`：CRUD、SSRF 校验（私有/回环/链路本地，`allowPrivate`）、
  ureq 拉取（30s 超时、8 MiB、10 次重定向）、每订阅 HTTP/SOCKS 代理、
  到期刷新 job（30 秒检查）、运行时合并（prepend/priority）、apply 失败回滚。
- `src/api/outbound_subs.rs` + 路由：list/create/update/delete/move/refresh/parse
  （`/panel/api/xray/outbound-subs...`）。
- `GET /panel/api/xray/` 返回 `subscriptionOutbounds`/`subscriptionOutboundTags`。
- 保存 Xray 模板/出站/路由时保留订阅标签的流量行与路由校验；备份/恢复加入新表。
- 前端：订阅弹窗增加“通过代理更新”输入框（`proxyUrl`，en-US/zh-CN/zh-TW 文案）；
  出站页“更多”只保留导入/导出，移除 WARP/NordVPN/PIA 菜单与 XrayPage 弹窗挂载。
- 依赖：`url`、`percent-encoding`（均已在 lock 中）；ureq 启用 `socks-proxy`。

验证（Windows debug + fake xray + 本地 mock 订阅/HTTP 代理）：

- `cargo test` **76/76**（新增 subscription 8 + outbound_subs 5 单测）；
  `cargo clippy --all-targets -- -D warnings` 零警告；`cargo fmt` 通过。
- `npm run build` 成功（`web/dist` 已更新）；改动文件 `tsc --noEmit` 无新增错误
  （仅既有 vitest/testing-library 缺失报错）；oxlint 通过。
- `tmp/e2e-outbound-subs/e2e.py` 全通过：直连/预览/稳定标签/HTTP 代理/
  SOCKS5h 代理（本地 mock）/非法代理/启停/排序/删除/SSRF 拒绝/空 URL；
  运行时 `config.json` 含订阅出站；未手动刷新的订阅被 30 秒后台 job 自动
  拉取并生效。
- 无头 Chrome（`tmp/e2e-outbound-subs/ui-run.py`）：出站页“更多”仅 2 项且无
  WARP/NordVPN/PIA；订阅弹窗存在代理输入框；控制台无报错。
- **未验证**：真实 Linux + 真实 Xray（`xray run -test` 拒绝/回滚未注入）；
  重定向逐跳 SSRF；JSON/Clash 订阅正文；真实公网 HTTP/SOCKS5 代理与认证
  （本地 mock 已测）。

## 2.13 0.2.12 发布打包（2026-11-01）—— 已验证

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.12`。
- 内容：出站订阅（迁移 0009、CRUD/预览/刷新/排序、SSRF、30 秒 job、运行时
  注入、标签稳定、失败回滚、每订阅 HTTP/SOCKS 代理）；出站页“更多”仅保留
  导入/导出。
- 构建：`cargo test` 76/76、clippy 零警告、`cargo fmt --check` 通过；
  前端 `npm run build` 成功；`cargo zigbuild --release --target
  x86_64-unknown-linux-musl` 成功。
- 发布目录：`dist/xray-panel-rs-linux-amd64-lumen/`（二进制 13,120,464 字节，
  静态 ELF、已 strip；含 install.sh/service/env/README/SHA256SUMS）。
- 发布归档：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（5,365,187 字节，
  SHA256 `3eddd23b0747e0bddfe1fe0df170ead2412dd582c2ac611aa876dbff27ccf949`）；
  二进制 SHA256 `8fc5c32fcdab08448c486e758e159e6709d34a12e87c7c6c2e1493d63fe06e78`。
- 0.2.11 归档保留为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.11.tar.gz`。
- 源码包：`dist/xray-panel-rs-0.2.12-src.tar.gz`（4,570,602 字节，SHA256
  `b734da61bae9792e4b8a8a1c130e83c904937e29f87683defa07f7ec68532332`）。
- 验证：`gzip -t` 通过；解包 `sha256sum -c SHA256SUMS` 通过；`cmp` 包内二进制
  与交叉编译产物一致；`file` 确认静态 ELF、已 strip；二进制内含 `0.2.12`、
  `outbound-subs`、`outbound_subscriptions`、`pages.xray.outboundSub.proxyUrl`。
- **未验证**：未在真实 Linux 上运行该 musl 二进制；未在真实 Xray 上验证订阅
  拉取/注入/回滚；真实公网代理认证未测。

## 2.14 功能整改（2026-11-02）—— 已验证（真实 Xray Windows）；未打包

用户逐项要求：

- 移除：路由测试；REALITY 目标扫描（含候选扫描弹窗与设置）；系统历史图表；
  面板自更新；客户端在线 IP 日志/清 IP；客户端 Happ 链接与订阅链接 UI；
  批量调整中的 flow/limitHwid/adTag；不支持的入站协议与 WireGuard。
- 实现：负载均衡实时状态/手动切换；ECH 密钥生成；ML-DSA-65 / ML-KEM-768 生成；
  从证书计算并填入 pinned SHA-256；Xray 版本切换、Geofiles 更新、GEOdata 标准源，
  并新增「更新代理」设置；批量调整只保留加天数/流量。

实现要点：

- 新增 `src/xray/router.rs` + 最小 `proto/router.proto`：RoutingService
  `GetBalancerInfo` / `OverrideBalancerTarget`；生成配置的 `api.services`
  增加 `RoutingService`。新增兼容接口 `xray/balancerStatus`、`xray/balancerOverride`。
- 新增 `src/updates.rs`：GitHub Release 列表（≥26.6.27，经 `updateProxy`）、
  Xray zip 下载/`.dgst` 校验/解压/原子替换/重启；Geofiles 白名单下载与
  `.sha256sum` 校验；`geodataSources` 标准源。
- `getNewmldsa65` / `getNewmlkem768` / `getNewEchCert` 调用真实 Xray 子命令；
  `getCertHash` 支持 PEM 内容或已引用证书文件，返回 DER SHA-256 数组。
- 前端：删除 RouteTester、RealityTargetScannerModal、SystemHistoryModal、
  PanelUpdateModal、XrayLogModal、AmneziaWGLogModal、XrayMetricsModal、
  SubLinksModal、ClientRenewalFields、HWID/IP 日志组件、Hosts/Nodes/Groups/
  API Docs/Sub/Rawconfig/Overrides 页面及其测试；入站协议下拉只保留
  `vmess/vless/trojan/shadowsocks/http/mixed`；后端入站接口拒绝其他协议。
- 设置页「运行环境」新增 `updateProxy`（http/socks5，校验后保存）；默认值
  不再返回订阅/Happ/access-log 等已移除功能的字段。
- 清理：移除面板更新、xraylogs、metricsState、HWID/IP/Happ/externalLinks、
  节点、扫描等空/假成功路由与处理函数；`getXrayResult` 等未使用接口删除。

验证：

- `cargo test` **83/83**；clippy 零警告；`cargo fmt --check` 通过。
- 前端 `npm run build` 成功；应用代码 `tsc --noEmit` 无报错（仅既有
  storybook/vitest 依赖缺失）；改动文件 oxlint 通过。
- 真实 Xray 26.10.31（Windows）端到端 `tmp/e2e-outbound-subs/real-e2e.py`：
  REALITY/ML-DSA-65/ML-KEM/ECH/x25519 生成、证书哈希、负载均衡状态与 override
  清空/设置、Release 列表、Xray 安装（SHA2-256 校验/解压/替换/重启）、
  `geoip.dat` Geofiles 更新（SHA-256 校验），全部通过。
- `geodata` 配置块经真实 `xray run -test` 验证为 `Configuration OK`。
- 订阅出站回归 `e2e.py` 全通过；更新/代理/协议过滤 `updates-e2e.py` 全通过
  （版本列表经 192.168.1.251:10813 代理获取）。
- 无头 Chrome：路由页无测试标签；仪表盘无面板更新/历史入口；设置页存在
  更新代理输入；入站协议选项恰为六项；出站“更多”仅导入/导出、订阅代理字段存在。
- **未验证**：真实 Linux 上的安装/Geofiles 路径权限与 asset 目录
  （`XRAY_LOCATION_ASSET` 未处理）；GEOdata 自动下载实际执行；更新代理认证；
  balancer override 在 Xray 重启后的持久化（设计上不持久化）。
- **未打包、未 bump 版本**（仍 0.2.12）。

## 2.15 0.2.13 发布打包（2026-11-02）—— 已验证

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.13`。
- 内容：2026-11-02 功能整改（见 §2.14）及此前 0.2.12 的全部内容。删除大量
  前端页面/后端假接口后，musl 二进制由 13,120,464 字节降至 12,623,520 字节。
- 构建：`cargo test` 83/83；clippy 零警告；`cargo fmt --check` 通过；
  前端 `npm run build` 成功（`web/dist` 时间 2026-11-02 00:38）；
  `cargo zigbuild --release --target x86_64-unknown-linux-musl` 成功（49.17s）。
- 发布目录：`dist/xray-panel-rs-linux-amd64-lumen/`（二进制 12,623,520 字节，
  静态 ELF、已 strip；含 `install.sh`、service/env 示例、README、SHA256SUMS）。
- 发布归档：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（5,354,421 字节，
  SHA256 `11ab05a6eb3f6a220890109ecf0445e1a45cc4518c36edf7e709c9c8c4a377ac`）；
  二进制 SHA256 `d407c2499653a0acc509abee4021a61e8bd748c55218525c67155c99725db9c6`。
- 0.2.12 归档保留为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.12.tar.gz`
  （5,365,187 字节，SHA256 `3eddd23b...cff949`）。
- 源码包：`dist/xray-panel-rs-0.2.13-src.tar.gz`（4,183,211 字节，SHA256
  `086ff06bc4ac817edc58584aca110e1ed05b976b81d82c1dbdd7b24f526addd9`；
  排除 `target/tmp/node_modules/web/dist-3xui/web/dist-custom-v2`，含已构建
  `web/dist`）。
- 验证：`gzip -t` 通过；解包 `sha256sum -c SHA256SUMS` 通过；`cmp` 包内二进制
  与交叉编译产物一致；`file` 确认静态 ELF、已 strip；二进制内含 `0.2.13`、
  `outbound-subs`、`outbound_subscriptions`、`geodataSources`、`updateProxy`、
  `RoutingService`、`mldsa65`、`mlkem768`。
- **未验证**：未在真实 Linux 上运行该 musl 二进制；真实 Linux 的安装/权限与
  `XRAY_LOCATION_ASSET`、GEOdata 自动下载、更新代理认证仍未回归（同 §2.14）。

## 2.16 Xray 安装下载顺序修复（2026-11-02）—— 已修复并单测，未打包

- 现象：`updateProxy` 使用面板托管 Xray 自己的本地入站（如本地 HTTP/SOCKS）
  时，版本列表能正常加载，但安装 Xray 在下载 zip 时报
  `io: Connection refused (os error 111)`。
- 原因：`install_xray` 原来在下载前调用 `state.xray.stop()`，代理入站随
  Xray 进程停止而消失，随后下载被拒绝。版本列表在停止前获取，所以看似正常。
- 修复：下载 zip、`.dgst` 校验、解压到临时文件全部在 Xray 仍运行时完成；
  下载失败不再停 Xray；仅在原子替换前停止 Xray，失败时重启旧版本。
- 代码：`src/updates.rs` 的 `install_xray` / `extract_xray_temp` /
  `replace_xray`；单测改为先暂存再替换。
- 验证：`cargo fmt --check` 通过、clippy 零警告、`cargo test` 83/83。
- **未验证**：真实环境端到端（需重新编译/打包）；0.2.13 发布包**不包含**此修复。

## 2.17 Geofiles / Xray 写入只读文件系统修复（2026-11-02）—— 已修复，未打包

- 现象：更新 `geosite.dat` 时在 `/usr/local/bin/.geosite.dat.*.tmp` 报
  `Read-only file system (os error 30)`。
- 原因：`XRAY_LOCATION_ASSET` 未设置时 Geofiles 写入 `XRAY_BIN` 目录
  （默认 `/usr/local/bin`）；0.2.13 的 systemd 单元有
  `ProtectSystem=full`，但 `ReadWritePaths` 只包含 `/etc/xray-panel` 与
  `/usr/local/etc/xray`，导致 Xray 二进制替换和 Geofiles 写入都被拒绝。
- 修复：`deploy/xray-panel.service` 的 `ReadWritePaths` 增加
  `/usr/local/bin`；README 单元示例同步；`docs/KNOWN-ISSUES.md` B8 记录。
- 临时处理（已部署 0.2.13）：在 `/etc/systemd/system/xray-panel.service`
  的 `ReadWritePaths` 追加 `/usr/local/bin`，`systemctl daemon-reload &&
  systemctl restart xray-panel`；或设置可写的 `XRAY_LOCATION_ASSET`
  （Xray 子进程会继承），但 Xray 二进制替换仍需可写目录。
- **未验证**：真实 systemd 环境端到端；需下一个发布包包含此修复。

## 2.18 0.2.14 发布打包（2026-11-02）—— 已验证

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.14`。
- 内容：0.2.13 全部功能 + Xray 安装先下载后停止（§2.16）+ systemd
  `ReadWritePaths` 加入 `/usr/local/bin`（§2.17）；无前端改动。
- 构建：`cargo fmt --check` 通过；clippy 零警告；`cargo test` 83/83；
  前端沿用 2026-11-02 00:38 的 `web/dist`；
  `cargo zigbuild --release --target x86_64-unknown-linux-musl` 成功（49.20s）。
- 发布目录：`dist/xray-panel-rs-linux-amd64-lumen/`（二进制 12,623,456 字节，
  静态 ELF、已 strip；`xray-panel.service` 含 `/usr/local/bin` 可写路径）。
- 发布归档：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（5,354,579 字节，
  SHA256 `3efcf8286540415e6baa189178618591dafbc6959aeb25d4c1edbc987ae06068`）；
  二进制 SHA256 `1ab4559fa9c59f83247172512325eb5fa0fa0d1f829837b4d5c09a8777334bd2`。
- 0.2.13 归档保留为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.13.tar.gz`
  （5,354,421 字节，SHA256 `11ab05a6...a377ac`）。
- 源码包：`dist/xray-panel-rs-0.2.14-src.tar.gz`（4,185,587 字节，SHA256
  `00e3713074afc14650aa06eaf716a39e4db857dca124765ffbe8bc26607f2624`；
  排除 `target/tmp/node_modules/web/dist-3xui/web/dist-custom-v2`）。
- 验证：`gzip -t`、解包 `sha256sum -c SHA256SUMS`、`cmp` 与交叉编译产物一致、
  `file` 静态 ELF/stripped；二进制内含 `0.2.14` 及既有功能标记；发布包内
  service 文件含 `/usr/local/bin`。
- **未验证**：真实 Linux 运行该 musl 二进制；真实 systemd 下 Geofiles 写入与
  Xray 安装端到端。

## 2.19 入站/出站协议补全（2026-11-03）—— 已实现并本地端到端验证；已随 0.2.15 打包

用户要求参考 3x-ui 补全入站/出站协议，且每个协议都能成功添加（不要只改前端）。
经确认：本轮只补 Xray 原生协议，不引入 sidecar 运行时。

已实现：

- **入站 11 种**：`vless`、`vmess`、`trojan`、`shadowsocks`、`http`、`mixed`、
  `wireguard`、`hysteria`、`tunnel`、`tun`、`masque`。
  - 前端 `InboundFormModal` 下拉与协议表单全部放开；新增 WireGuard/
    Hysteria/MASQUE/TUN/Tunnel 字段组件（从 3x-ui 前端复制并裁剪）。
  - 后端 `inbound_input_from_params`、原生 `/api/inbounds` 白名单、
    `build_config` 全部支持。
  - `build_inbound` 按协议整形：WireGuard 用数据库客户端重建 `peers`；
    Hysteria 注入 `clients` 并强制 `version:2`（settings 与
    streamSettings.hysteriaSettings）；MASQUE 用 `pass` 且邮箱不含 `:`;
    Tunnel/TUN 原样透传。Shadowsocks 2022 无客户端时保留单用户 `password`
    （此前会被删除导致无 key）。
  - 客户端建号：WireGuard 后端生成 X25519 keypair 并按
    `subnetIp/subnetCidr` 分配 `allowedIPs`；Hysteria 生成/保留 `auth`；
    MASQUE 生成/保留 `password`。新增依赖 `x25519-dalek`。
- **出站 13 种**：freedom、blackhole、dns、vmess、vless、trojan、shadowsocks、
  wireguard、hysteria、masque、socks、http、loopback；下拉隐藏 amneziawg。
- **明确不提供**：mtproto/tuic/amneziawg 入站、amneziawg 出站（需要
  mtg-multi/tuic-server/amneziawg-go sidecar，本面板不内置），后端继续返回
  `unsupported protocol`，不伪造成功。

验证：

- `cargo test` **89/89**（新增 4 个配置生成测试 + 2 个入站/客户端凭证测试）；
  `cargo clippy --all-targets -- -D warnings` 零警告；`cargo fmt --check` 通过。
- 前端 `npm run build` 成功，`web/dist` 已重建；改动文件 oxlint 通过；
  `npm run typecheck` 仅剩既有 vitest/storybook 缺依赖报错。
- 后端 fake xray + 临时 DB 端到端（`tmp/e2e-protocols/run.mjs`）：11 种入站
  全部添加成功；4 种非原生协议被明确拒绝；WireGuard/Hysteria/MASQUE 建客户端
  后密钥/auth/password 与生成配置正确。
- 无头 Chrome（`tmp/e2e-protocols/ui-test.mjs`、`ui-save.mjs`、
  `ui-outbound.mjs`）：入站下拉 11 项且每种表单渲染预期字段；9 种无需证书的
  协议用真实前端表单保存成功；出站下拉 13 项且无 amneziawg；控制台无报错。
- **未验证**：官方 Xray 26.10.31 的 `xray run -test` 与真实连接/流量。
  本机无 Xray 二进制，下载/执行外部二进制需要用户授权。见
  `KNOWN-ISSUES.md` M1；hysteria/masque 还需要有效的 TLS 证书。
- 已随 0.2.15 打包（§2.21）。

## 2.20 节点地址设置（2026-11-03）—— 已实现并端到端验证；已随 0.2.15 打包

用户要求面板设置增加“节点地址”，分享链接和二维码使用设置里的域名/IP，
不要使用浏览器当前地址（可能是局域网地址）。

已实现：

- 设置页「面板设置」新增 `节点地址` 输入（`GeneralTab`），域或 IP，拒绝
  `://`、`/` 和空格；保存到 `settings.x3ui_settings` 的 `nodeAddress`。
- 后端 `setting/all` 在 `nodeAddress` 缺失时用旧 `public_address` 行回填；
  `setting/defaultSettings` 同时返回 `nodeAddress` 与 `webDomain`（兼容旧前端）。
- 前端 `preferPublicHost` 改为配置值始终优先：入站导出链接、入站信息弹窗链接、
  客户端二维码均使用 `nodeAddress`，不再写入浏览器的局域网地址；入站自己的
  `shareAddrStrategy`（listen/custom）仍优先。
- 后端 `public_address()`（`/panel/api/clients/subLinks`、
  `/panel/api/inbounds/allLinks`）优先级：`nodeAddress` → 旧 `public_address`
  设置 → 请求 Host。
- 顺带修复：通过入站 JSON 导入/携带 clients 创建的客户端此前没有 `subId`，
  导致二维码无链接；`sync_clients_from_settings` 现在保留或生成 `subId`。

验证（Windows debug + fake xray + 临时 DB，`tmp/e2e-nodeaddr/`）：

- `cargo test` 89/89、clippy 零警告、fmt 通过；前端构建成功，改动文件 oxlint
  无新增问题。
- API 端到端 17/17：保存/读取 nodeAddress、defaultSettings 映射、拒绝非法值、
  `subLinks` 和 `allLinks` 均使用 `node.example.com` 而非请求 Host
  `192.168.1.50`，IP 形式 `203.0.113.10` 同样生效。
- 无头 Chrome：设置页显示并带出 `节点地址` 值；入站导出链接、客户端二维码
  接口链接均使用 `node.example.com`，不含 `127.0.0.1`；控制台无报错。
- 未验证：真实反向代理/真实客户端连接（本次只验证链接文本）。
- 已随 0.2.15 打包（§2.21）。

## 2.21 0.2.15 发布打包（2026-11-03）—— 已验证

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.15`。
- 内容：0.2.14 全部功能 + 协议补全（§2.19）+ 节点地址设置（§2.20）。
- 构建：前端 `npm run build`（22:40）；`cargo fmt --check`、`cargo test` 89/89、
  clippy 零警告；musl release 交叉编译成功（58.64s）。
- 发布目录：`dist/xray-panel-rs-linux-amd64-lumen/`（二进制 12,691,200 字节，
  静态 ELF、已 strip）。
- 归档：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（5,398,823 字节，SHA256
  `40b4b2458c48fee4f8818c02e8c5f1e2e8e41a4b2e301542fca9339889afb13a`）；
  二进制 SHA256 `105a28a94e358ab2488827e13ca689b5a161fee2d3f2a865dbf34ce7070b06f2`。
- 0.2.14 归档改为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.14.tar.gz`
  （5,354,579 字节，SHA256 `3efcf828...06068`）。
- 源码包：`dist/xray-panel-rs-0.2.15-src.tar.gz`（4,207,649 字节，SHA256
  `d6e7c21eb3efb690ad62e4be280adaff451b615795626ce52599832d9545cac3`）。
- 验证：`gzip -t`、包内 `sha256sum -c`、二进制与交叉编译产物 `cmp`、`file`
  （静态 ELF/stripped）、二进制内含 `0.2.15` / `nodeAddress` / `节点地址` /
  `wireguard`。
- 未验证：真实 Linux 运行 musl 二进制；官方 Xray `run -test`（T21）；真实客户端
  用生成的分享链接/二维码连接。

## 2.22 GitHub 发布、一键安装与面板在线更新（2026-11-03）—— 已实现，本地验证；发布验证进行中

- 仓库：`ztoofar/xray-panel-rs`（public），main 已推送。
- 面板在线更新：
  - `src/panel_update.rs` + `GET /panel/api/server/panelUpdateCheck` /
    `POST /panel/api/server/panelUpdate`。
  - latest release 的原始二进制 `xray-panel-rs` + `SHA256SUMS`，校验后原子替换；
    systemd 下 1.5s 后退出，由 `Restart=on-failure` 拉起；非 systemd 返回
    `restartRequired:true`。
  - 设置页「面板更新」区块：当前/最新版本、检查更新、立即更新、Release 链接；
    14 个 locale 文案已补齐。
- 一键安装：根目录 `install.sh` 下载最新 Release、校验 SHA256、可选安装官方
  Xray，再调用 `deploy/install.sh`；支持 `PANEL_VERSION`、`PANEL_REPO`、
  `SKIP_XRAY`、`XRAY_VERSION`。
- GitHub Actions：
  - `ci.yml`：main/PR 前端构建 + `cargo fmt/test/clippy`。
  - `release.yml`：`v*` 标签触发，Node 26 + cargo-zigbuild musl 构建，
    发布 tar.gz、原始二进制、SHA256SUMS；编译时注入当前仓库名。
- 本地验证：`cargo test` 90/90、clippy 零警告、fmt 通过、前端构建成功、
  musl release 成功；包内 `sha256sum -c`/`gzip -t`/`file` 通过。
- 未验证：GitHub Actions 首次发布、真实 Linux systemd 在线更新全流程（N2）。
  详见 `KNOWN-ISSUES.md` N1/N2。

## 2.23 GitHub Release v0.2.16（2026-11-03）—— 已验证

- 仓库：<https://github.com/ztoofar/xray-panel-rs>（public），main 与 tag `v0.2.16`
  已推送；Actions `ci` 通过（fmt/test/clippy），`release` 通过。
- Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.16>
  - `xray-panel-rs-linux-amd64-lumen.tar.gz`：5,417,245 字节，SHA256
    `ad58e7dae135a0fdfa689a2af5935ef3989d2c28a58e007284abcad000604b3c`
  - `xray-panel-rs`：12,742,128 字节，SHA256
    `f1760246078b995118b608085c21b0f65d0d218fc472cad2b50fef228209ee83`
  - `SHA256SUMS`：`f176...ee83  xray-panel-rs`
- 一键安装（README 顶部）：
  `curl -fsSL https://raw.githubusercontent.com/ztoofar/xray-panel-rs/main/install.sh | sudo bash`
- 验证：
  - 本地 panel（0.2.16 debug）调用 `panelUpdateCheck` 成功返回
    `current=0.2.16, latest=0.2.16, updateAvailable=false, repo=ztoofar/xray-panel-rs`。
  - Windows 调用 `panelUpdate` 返回明确错误
    `online panel update is supported on Linux only`（符合预期）。
  - 无头 Chrome：设置页显示「面板更新 / 检查更新 / 节点地址」，控制台无报错。
  - GitHub Actions release 成功产出 tar.gz、原始二进制、SHA256SUMS。
- 未验证：真实 Linux + systemd 在线更新全流程（TODO T23 / KNOWN-ISSUES N2）；
  一键安装脚本未在全新 Linux 上实际执行（只做了语法与资产校验）。

## 2.24 Release 资产精简为 tar.gz，在线更新解压（2026-11-03）—— 已验证

- 用户要求 Release 不再单独放二进制和 SHA256SUMS，压缩包已包含。
- `src/panel_update.rs`：
  - latest release 只查找 `xray-panel-rs-linux-amd64-lumen.tar.gz`；
  - 用 GitHub 资产 `digest`（`sha256:<hex>`）校验下载的 tar.gz；
  - 内置最小 tar.gz 解析器（`flate2` + tar header）解出
    `xray-panel-rs-linux-amd64-lumen/xray-panel-rs`，校验 ELF 头和大小上限；
  - 新增 3 个解析测试（正常/缺文件/非 ELF）。
- `.github/workflows/release.yml`：只上传 tar.gz；包内 SHA256SUMS 保留。
- 版本 0.2.17；`cargo test` 93/93、clippy 零警告、fmt 通过、musl release 成功。
- GitHub Release `v0.2.17` 唯一资产：tar.gz 5,422,123 字节，digest
  `sha256:23a4885d7d87a2354f0c4e979eb2bb0644454e1cc47c5c61736f3825d08318a9`。
- 验证：本地 0.2.17 面板 `panelUpdateCheck` 成功返回 current/latest 0.2.17、
  `updateAvailable:false`；Release 只有 tar 资产，无二进制/SHA256SUMS。
- **兼容性**：v0.2.16 更新器查找旧原始二进制资产，无法自动升级到 0.2.17；
  0.2.16 用户需用一键安装脚本升级一次（KNOWN-ISSUES N3）。
- 未验证：真实 Linux + systemd 在线更新与解压全流程（T23/N2）。

## 2.25 安装/更新代理与 systemd 重启判定（2026-11-03）—— 已在 GitHub Actions Linux 验证

- `install.sh`：
  - 检测已有 `/usr/local/bin/xray-panel-rs`，打印更新模式；DB 与
    `/etc/xray-panel.env` 保留（复用 `deploy/install.sh`）。
  - 新增 `PANEL_PROXY`（HTTP/SOCKS；回退 `HTTPS_PROXY`/`ALL_PROXY`），所有面板与
    Xray 下载统一走 `download()`；失败时报错包含代理地址。
  - 修复 Release 包内 `install.sh` 缺少执行位的问题（git mode 100755 +
    release workflow `chmod +x`），下载端也改用 `bash install.sh`。
- 面板在线更新：
  - 复用设置页「更新代理」（`updateProxy`）；check 与 install 的错误链加入
    `via update proxy <addr>`，前端直接显示。
  - 重启判定改为 `PANEL_SYSTEMD=1`（服务单元注入）；旧单元回退
    `INVOCATION_ID` + `systemctl show MainPID == 当前 PID`，避免在 systemd 会话
    里手动运行也误退出。
- 新增 `.github/workflows/integration.yml`（手动触发）：
  - `online-update`：构建 v0.2.17 测试面板 → 真实下载 v0.2.18 tar.gz、按 digest
    校验、解压替换 → 重启后版本 0.2.18；坏代理时错误包含代理地址。
  - `installer`：坏代理报错；真实执行一键脚本安装 v0.2.18（systemctl stub），
    校验二进制版本、service 和 env 文件。
- 结果：两个 job 均通过；`cargo test` 93/93、clippy 零警告。
- 发布：GitHub Release v0.2.18，唯一资产 tar.gz 5,425,000 字节，digest
  `sha256:ba59a2a2...8496`。
- 未验证：真实 systemd 服务下的自动重启（CI 用 stub）；见 TODO T23。

## 2.26 安装幂等、更新报错与 README 重写（2026-11-04）—— 已实现并本地验证；未打包

用户要求：
1. 已安装最新版时重跑一键脚本不应重复安装，并提供强制重装开关。
2. 面板在线更新经代理连接失败时，报错应可操作（实测
   `update proxy http://127.0.0.1:10813` → `Connection refused (os error 111)`）。
3. README 精简、正式化。

已实现：

- `install.sh`：
  - 新增 `PANEL_FORCE=1`（强制重装）。
  - 通过 `https://github.com/<repo>/releases/latest` 的 `Location` 解析目标版本，
    请求走 `PANEL_PROXY`。
  - `installed_version()` 先 `grep -a LUMEN_VERSION=` 再执行 `--version`，避免
    旧二进制被探测时启动服务/执行迁移。
  - 版本一致时直接退出：不下载、不停服务；无 marker 的旧二进制下载校验后按
    包内二进制 SHA-256 比对，一致时同样跳过。
  - 跳过面板重装时仍会在 `/usr/local/bin/xray` 缺失时安装 Xray。
- `src/main.rs`：`--version` / `-V` 在初始化前输出
  `xray-panel-rs <ver> (LUMEN_VERSION=<ver>)`；新增参数识别单测。
- `src/updates.rs`：新增 `network_hint` / `with_network_hint`，按
  `io::ErrorKind` 与 `ureq::Error` 变体识别传输失败，本地化文本
  （`os error 111`/`10061` 等）兜底；用于 `xray_versions`、`install_xray`、
  Geofiles。
- `src/panel_update.rs`：check/install 的 GitHub 查询与下载错误前加可操作提示：
  有代理时说明 `127.0.0.1` 指面板服务器自身、需从面板主机可达；无代理时提示
  检查网络或配置代理。原始错误链保留。
- `README.md`：重写为功能概览/安装/更新/配置/部署/构建/限制；修正轮询默认值
  （5 秒，2–300）、`limitIp` 口径、健康检查路径等过期描述。
- `release.yml` 发布说明补充“已最新版跳过重装 / `PANEL_FORCE`”。

验证：

- `cargo fmt --check` 通过；`cargo test` **97/97**；clippy 零警告；
  `cargo build` 成功。
- `./target/debug/xray-panel-rs.exe --version` 与 `-V` 均输出带
  `LUMEN_VERSION=` 的版本行；二进制内可 `grep` 到该 marker。
- `tmp/install-skip-test/run.sh`（离线桩，假 root/假 Release，10 项断言）全通过：
  marker 匹配跳过且不下载/不部署、无 marker 时哈希匹配跳过、`PANEL_FORCE`
  强制部署、版本不匹配进入安装路径。
- `tmp/e2e-update-hint/e2e.py`（Windows debug + fake xray + 临时 DB）全通过：
  `updateProxy=127.0.0.1:10813` 时 `panelUpdateCheck` 返回 `success:false`，
  消息包含代理地址、`127.0.0.1 is the panel server itself` 与
  `os error 10061`；非法代理仍被拒绝。
- **未验证**：真实 Linux / musl 二进制；真实 systemd 下重跑脚本；代理可达但
  目标不可达时的提示归因；0.2.18 及更早版本没有 marker，首次重跑仍需下载
  5.4 MB 后按哈希跳过。
- 已随 0.2.19 发布（§2.27）；发布前状态为未打包、未 bump 版本。

## 2.27 0.2.19 发布（2026-11-04）—— 已验证

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.19`（0.2.18 的补丁递增）。
- 内容：§2.26 的安装幂等跳过、`--version`/`LUMEN_VERSION` marker、更新/下载
  代理报错提示、README 重写；无前端改动。
- 发布路径：commit `bd2faf7` + push `main` → tag `v0.2.19` → GitHub Actions
  `release.yml` run `41673502413` 成功并创建 Release；本地不打包。
- Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.19>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` **5,424,758 字节**，
  GitHub asset digest
  `sha256:ab81a7c79507211afe8798576bbb057ba8c1015461700db4c91847cbf18435c6`。
- 验证（发布前）：`cargo fmt --check` 通过、`cargo test` **97/97**、clippy
  零警告、`cargo build` 成功；`install.sh` 离线桩 10/10；面板更新报错 e2e
  全部通过。
- 0.2.19 起：面板二进制支持 `--version`/`LUMEN_VERSION`，重跑一键脚本可零下载
  跳过；0.2.18 及更早仍走下载+哈希跳过（KNOWN-ISSUES N5）。
- 未验证：真实 Linux/musl 运行、真实 systemd 重跑（TODO T24）。

## 2.28 代理/网络类报错改为中文（2026-11-04）—— 已实现并 e2e 验证；随 0.2.20 发布

用户要求：代理导致的报错直接提示“代理不可用”，全局改为中文，不再把
`io`/`ureq` 原生错误返回给前端。

- `src/updates.rs`：
  - `network_hint` 返回中文用户消息：有代理 → `代理不可用：无法连接 <proxy>...`；
    无代理 → `无法连接 GitHub：...`。
  - `with_network_hint` 用中文消息**替换**整个错误链；原生错误链
    （URL、`io: Connection refused (os error ...)`）只写入 `tracing::warn`。
  - 覆盖面板更新 check/install、Xray 版本/安装、Geofiles。
- `src/outbound_subs.rs`：订阅拉取的传输失败同样映射为中文
  （有代理 → `代理不可用`；无代理 → `无法连接订阅地址`）；订阅响应、代理校验
  文案改为中文。
- `src/api/compat.rs`：更新/下载类接口的失败前缀改为中文
  （检查面板更新失败、更新面板失败、获取 Xray 版本失败、安装 Xray 失败、
  更新 Geofiles 失败）；保存 `updateProxy` 的校验错误改为
  `更新代理设置无效：...`。
- 验证：
  - `cargo test` **99/99**（新增 `network_hint_hides_the_native_error`、
    `subscription_proxy_errors_are_chinese`）；clippy 零警告；fmt 通过。
  - `tmp/e2e-update-hint/e2e.py` 全通过：`panelUpdateCheck` 返回
    `检查面板更新失败：代理不可用：...`，包含代理地址与“面板服务器自身”，
    且不含 `api.github.com`、`os error`；非法 `updateProxy` 返回中文校验错误。
- **范围说明**：非网络类错误仍可能包含英文原始信息（如 GitHub 返回非 200、
  Release 缺资产）。见 `KNOWN-ISSUES.md` N7。

## 2.29 0.2.20 发布（2026-11-04）—— 已验证

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.20`。
- 内容：§2.28 的代理/网络错误中文化；无前端改动。
- 发布路径：提交 `d6f7efa` + push `main` → tag `v0.2.20` → GitHub Actions
  `release.yml` 成功并创建 Release；本地不打包。
- Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.20>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` **5,426,606 字节**，
  GitHub asset digest
  `sha256:61f1a69f4211cc0b7aab095e4a6895dae689f87ade20f8448fead03e0f909269`。
- 发布前验证：`cargo fmt --check`、`cargo test` 99/99、clippy 零警告、
  `cargo build` 成功；更新报错 e2e 全通过。
- 未验证：真实 Linux/musl 运行、真实代理不可达场景的 UI 展示（本地 e2e 覆盖
  返回文本，未用真实反向代理/浏览器确认）。

## 2.30 2026-11-04 功能增强（未打包、未 bump 版本）

用户逐项要求：客户端在线 IP/位置、后端错误中文化、Xray 版本短格式、
安全警告按登录周期显示、高级设置 + 出站真实连接检测通知、面板日志中国时间/
详细/历史、设置保存不重启。全部实现并本地验证（Windows debug + 真实
Xray 26.10.31 + 无头 Chrome）；**未在真实 Linux/musl 上运行，未打包**。

实现：

- **客户端详情 IP/位置**：`POST /panel/api/clients/onlineIps` 返回
  `email -> [ip...]`（来自 Xray `GetUsersStats`）；`POST
  /panel/api/clients/ipLocation {ip}` 服务端查询 ipwho.is（失败回退
  ipapi.co），仅点击“位置”时请求。`ClientInfoModal` 只在客户端在线时显示
  “当前 IP”行，多个 IP 全部列出，每个 IP 旁有“位置”按钮。
- **后端错误中文化**：`fail(`/`AppError`/`bail!`/`context` 等面向用户的英文
  文案统一改为中文；Xray 原生输出（如 `xray run -test` 的 stdout/stderr）保持
  原文。`AppError` 的 unauthorized/CSRF/限流/内部错误也改为中文。
- **Xray 版本短格式**：`supervisor::parse_version_line` 从
  `Xray 26.10.31 (...)` 提取 `26.10.31`；系统状态页只显示该版本号。
- **安全警告按登录周期**：设置页关闭警告后写入
  `localStorage.lumen-settings-security-alert-dismissed`；刷新保持隐藏，
  登录页登录成功后清除，下一次登录重新显示。
- **高级设置页**：`/settings#advanced`（侧栏“高级”）新增出站真实连接检测：
  启用开关、多选出站（含订阅出站 tag）、1-1440 分钟检测频率、微信 Webhook
  输入与最近检测结果。后台 `outbound_check` job 复用临时 Xray SOCKS+HTTP
  204 真实探测；不可用时经企业微信群机器人发送通知（状态变化或同一故障
  每 30 分钟重复一次），恢复也通知。微信 Webhook 文案改为“默认通知方式”。
- **面板日志**：
  - 时间固定 UTC+8（`panel_log::ChinaTime`），与主机时区无关；
  - 新增按日文件 `panel-logs/panel-YYYY-MM-DD.log`（默认与数据库同目录，
    可用 `PANEL_LOG_DIR` 覆盖），保留 14 天；
  - 文件/实时缓冲区记录 target、文件、行号；新增登录、设置保存、入站/客户端
    增删改、Xray 重启/停止、面板重启、出站检测等审计日志；
  - `GET /panel/api/server/panelLogFiles` 列出历史文件；日志接口
    `source=panel&file=<name>` 读取历史；日志弹窗新增“实时日志/历史日志”选择。
- **设置保存不重启**：`setting_update` 不再因 `xrayTemplateConfig` 调用
  `service::apply`（Xray 页面仍走 `/panel/api/xray/update`）；设置保存后
  Xray/面板进程均不重启。“重启面板”按钮改为始终可点击，只有点击后才
  `restartPanel`。

验证：

- `cargo fmt --check`、`cargo test` **107/107**、`cargo clippy --all-targets
  -- -D warnings` 零警告、`cargo build` 成功。
- 前端 `npm run build` 成功；改动文件 `tsc --noEmit` 无新增错误（既有
  vitest/storybook 缺依赖报错仍在）。
- `tmp/e2e-advanced/e2e.py`（真实 Xray 26.10.31 + 本地 mock 微信 webhook）：
  版本短格式、设置保存不重启 Xray、高级设置持久化、blocked 出站检测失败、
  webhook 通知、日志历史/中国时间/详细字段、在线 IP 接口、非法 IP 中文错误、
  重启面板退出进程，全部通过。
- `tmp/e2e-advanced/ui-run.py`（无头 Chrome）：高级页渲染/出站选项、
  安全警告关闭→刷新保持→重新登录恢复、离线客户端不显示 IP/位置、日志历史
  选择与实时日志、日志中国时间，全部通过。
- `tmp/e2e-advanced/online-e2e.py`（真实 Xray + 真实 VLESS 客户端连接）：
  `onlineIps` 返回局域网源 IP；在线客户端信息弹窗显示“当前 IP/位置”，
  默认不查询位置，点击后才调用 `ipLocation`，全部通过。
- **未验证**：真实 Linux/musl；真实公网客户端 IP（测试用局域网 IP，Xray
  OnlineMap 会丢弃回环地址 127.0.0.1）；第三方位置服务在面板网络中的可达性；
  英文界面下后端错误仍为中文（见 `KNOWN-ISSUES.md` H4）。
- **未打包、未 bump 版本**（`Cargo.toml` 仍 0.2.20）。

## 3. 既有功能（历史记录，本次会话未逐项重测）

以下能力在 `PROGRESS.md` / README 中有实现与历史验证记录，本次只做了编译与
相关单元测试回归，**不宣称本次重新验证**：

- 管理员登录（Argon2id、会话 Cookie、CSRF、登录限流）、改密、单管理员。
- 入站 CRUD、批量操作、fallbacks、启停、分享链接、QR；多协议字段表单
  （VLESS/VMess/Trojan/Shadowsocks/HTTP/Mixed/WireGuard 等）。
- 客户端 CRUD、批量增删改/关联、分组、流量/到期/重置天数、在线状态、
  最后在线、`subId` 分享链接。
- 出站流量统计、出站 TCP/HTTP/真实延迟测试。
- 路由 / DNS / 日志 / Xray 额外配置编辑；配置生成、`xray run -test`、
  子进程监管、异常自动重启、配置备份。
- 系统状态仪表盘：CPU/内存/交换/磁盘/网络/连接数/运行时间/Xray 状态/
  面板 RSS 与线程/公网 IP；面板日志与 Xray 日志。
- 流量轮询（默认 5 秒，可设 2–300 秒）、配额/到期/重置/`limitIp` 近似执行。
- 在线归属：per-attachment 统计身份 `<inbound_id>:<email>`，`GetUsersStats`。
- SQLite `.db` 备份/恢复；独立 `/settings` 页；面板设置页（常规/运行环境/时间）。
- 静态 musl 单二进制、systemd 部署脚本、Lumen 青蓝主题。

## 4. 当前未支持 / 已知限制

详见 `docs/KNOWN-ISSUES.md`，摘要：

- 3x-ui 高级接口未实现：2FA、邮件、订阅服务器、节点、HWID 日志；
  调用返回明确错误。出站订阅已实现（§2.12）；路由测试、扫描、系统历史、
  面板更新、客户端 IP 日志、Happ/订阅 UI 已移除（§2.14）。
- 负载均衡状态/override 已实现（RoutingService）；override 不跨 Xray 重启持久化。
  见 `KNOWN-ISSUES.md` B8、`TODO.md` T19。
- Xray 模板里的 `inbounds` 不参与运行时；高级编辑器已隐藏该视图。
- 无 WebSocket，全部轮询。
- WireGuard/Hysteria/MASQUE 客户端管理与配置生成已实现（§2.19）；服务端
  `subLinks` 仍不生成这三种链接。`limitHwid` 只保存不强制；`limitIp` 为近似。
- mtproto/tuic/amneziawg 入站与 amneziawg 出站不提供（sidecar 运行时未内置）。
- 在线列表依赖 Xray 26+ 的 `GetUsersStats` / `statsUserOnline`；旧核心为空。
- 客户端/入站变更通过重启 Xray 生效，会短暂中断连接。
- 面板不提供 TLS/证书管理；单管理员；DB 恢复要求 schema 一致。
- README 有若干过期描述（轮询默认值/范围、IP 限制口径、`/settings` 说明），
  见 `TODO.md`。
- `npm run typecheck` 因 devDependencies 不全有既有报错（非本次改动引入）。

## 5. 快速自检命令

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cd frontend-3xui && export PATH="/d/ClaudeCode/Tools/node-v26.13.0-win-x64:$PATH" && npm run build
export PATH="/d/ClaudeCode/Tools/zig-npm/node_modules/@zigc/win32-x64/bin:$PATH"
cargo zigbuild --release --target x86_64-unknown-linux-musl
```

0.2.8 打包结果（2026-10-31 22:13）：`cargo test` 56/56、clippy 零警告、
前端构建成功、musl 交叉编译成功；包内二进制与交叉编译产物 `cmp` 一致，
`file` 确认静态 ELF、已 strip，`gzip -t` 通过，二进制内含 `0.2.8`，
并确认嵌入前端含 `Panel Logs`、后端含登录审计串。0.2.7 归档为
`dist/xray-panel-rs-linux-amd64-lumen-0.2.7.tar.gz`。**未在真实 Linux +
真实 Xray 上运行。**

0.2.9 打包结果（2026-11-01 09:22）：`cargo test` 61/61、clippy 零警告、
前端构建成功、musl 交叉编译成功；包内二进制与交叉编译产物 `cmp` 一致，
`file` 确认静态 ELF、已 strip，`gzip -t` 与包内 `sha256sum -c` 通过，
二进制内含 `0.2.9`、`0008_inbound_meta`/`sub_sort_index`、`Panel Logs`、
`connectionLimits`。0.2.8 归档为
`dist/xray-panel-rs-linux-amd64-lumen-0.2.8.tar.gz`。**未在真实 Linux +
真实 Xray 上运行。**
