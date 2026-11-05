# CURRENT-STATUS

> 最后更新：2026-11-05 23:00（本地时间）
> 本文件只记录**当前真实状态**。历史细节见 `DEVELOPMENT-LOG.md` / `PROGRESS.md`。
> 未验证的能力不得写成“已完成”；每项后面标注验证方式或“未验证”。

## 1. 版本与构建状态

| 项目 | 状态 |
| --- | --- |
| `Cargo.toml` 版本 | `0.2.27`（发布 v0.2.27） |
| 最近发布 | GitHub `ztoofar/xray-panel-rs` Release **`v0.2.27`**：唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` **5,518,387 字节**，asset digest `sha256:d0209122...138b`；Release 页面只显示更新内容 |
| 当前源码 | `0.2.27`：实时 WS 帧改用真实采样间隔做除数，修复慢 tick 下入站/客户端速度按比例虚高；首轮无速率不广播。详见 §2.43 |
| 上一版 | GitHub Release `v0.2.26`：tar 5,518,877 字节，asset digest `sha256:9c223ffa...ae2f` |
| 0.2.27 SHA256 | `d02091227e04682c8c26328c924f03394d68318fd79b3cbdaa9cb96d5c65138b`（tar.gz，GitHub asset digest；本机下载被网络重置，未做本地解包复核） |
| 0.2.26 SHA256 | `9c223ffae10e0c308c67078d90bef97dad3fb8ea02e07831114fd751c1baae2f`（tar.gz，GitHub asset digest） |
| 0.2.25 SHA256 | `d9128d0c349011dd0f8d22829857c8cf4c3b3e8002e8d3f62c0bb8d84db792a5`（tar.gz，GitHub asset digest） |
| 0.2.24 SHA256 | `88471d1f81d1e287c069c10b2753702dc2da7b177fa27cd3bb7cecde897330eb`（tar.gz，GitHub asset digest） |
| 0.2.23 SHA256 | `9bd16e8409f3c38098ef730f05bfb8f8e7bd49764eedab8225e9e5f40ced3811`（tar.gz，GitHub asset digest） |
| 0.2.22 SHA256 | `c3a60555bd500a17ea52a53fe39ac56436ffe529c404a081e09e2cb1044c44e9`（tar.gz，GitHub asset digest）；包内二进制校验见 §2.35 |
| 0.2.21 SHA256 | `143e27bfdc28fa745f433c54337c52c17daa81709d05d2296236639bf47f101a`（tar.gz，GitHub asset digest）；包内二进制 SHA256 `997fb41ace44b6d69f4fa1502e1aae0aedb9dbffd2d8b9d44329f208cfe3dcdb` |
| 0.2.20 SHA256 | `61f1a69f4211cc0b7aab095e4a6895dae689f87ade20f8448fead03e0f909269`（tar.gz）；包内 `SHA256SUMS` 校验解压后的二进制 |
| 源码包 | GitHub Release 自动提供 source 归档；本地 0.2.16 源码包保留在 `dist/` 仅作备份 |
| 前端构建产物 | `web/dist/` 已在 2026-11-05 22:4x 由当前 `frontend-3xui/` 源码重建（本次前端源码无改动） |
| 当前生效前端 | `frontend-3xui/`，七个路由：`/`、入站、客户端、出站、路由、Xray、设置 |
| musl 产物 | 0.2.27 Release tar 内含静态 ELF 二进制（已 strip）；本地不生成备用二进制 |
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

## 2.31 0.2.21 发布（2026-11-04）—— 已验证

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.21`。
- 内容：0.2.20 全部功能 + §2.30 的七项增强（客户端在线 IP/按需位置、
  后端错误中文化、Xray 短版本号、安全警告按登录周期、高级页出站真实连接检测
  与微信通知、面板日志 UTC+8/详细/14 天历史、设置保存不重启）。
- 发布路径：提交 `85f85da` + push `main` → CI `41816323238` 通过 → tag
  `v0.2.21` → GitHub Actions release run `41816700205` 成功创建 Release。
- Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.21>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` **5,465,980 字节**，
  GitHub asset digest
  `sha256:143e27bfdc28fa745f433c54337c52c17daa81709d05d2296236639bf47f101a`。
- 本地下载校验：tar.gz SHA-256 与 GitHub digest 一致；包内
  `sha256sum -c SHA256SUMS` 通过；二进制 SHA256
  `997fb41ace44b6d69f4fa1502e1aae0aedb9dbffd2d8b9d44329f208cfe3dcdb`，
  `file` 确认为 x86-64 静态 ELF（已 strip），二进制内含 `LUMEN_VERSION=0.2.21`。
- 发布前验证：`cargo test` 107/107、clippy 零警告、fmt、前端构建；
  `tmp/e2e-advanced/e2e.py`、`ui-run.py`、`online-e2e.py` 全通过。
- **未验证**：未在真实 Linux/musl 环境运行该二进制（见 TODO T26）。

## 2.32 位置查询主源改为纯真库（2026-11-04）—— 已实现，单测与联网实测；未打包

- 问题：用户反馈位置查询与 <https://tool.lu/ip/> 不一致。实测 ipwho.is 把
  `115.193.109.37`（浙江杭州电信）定位为“江苏南京”，而 tool.lu 的纯真/淘宝
  库、zxinc、淘宝 API 均返回“浙江杭州 电信”。
- 改动：`src/api/ip_location.rs` 主数据源改为 zxinc 免费 JSON API
  `https://ip.zxinc.org/api.php?type=json&ip={ip}`（纯真库），解析
  `data.country`（`–`/Tab 分隔的层级）与 `data.local`（运营商），多余层级
  并入城市；失败回退 ipwho.is；移除经常 `RateLimited` 的 ipapi.co。
- 返回 JSON 结构不变；zxinc 无时区/经纬度，`timezone` 为空字符串、
  经纬度为 `null`（前端未用）。
- 验证：`cargo test` **109/109**、clippy 零警告、fmt 通过；联网实测
  `115.193.109.37` → 中国/浙江/杭州/电信，`8.8.8.8` →
  美国/加利福尼亚州/圣克拉拉 山景城/谷歌公司DNS服务器，IPv6 正常。
- **未验证**：面板实际部署网络的 zxinc 可达性与延迟；未在真实 Linux/musl
  上运行，未点击前端按钮复测（前端/接口契约未变）。
- 未打包、未 bump 版本（仍 `0.2.21`）。

## 2.33 IP数据云可选高精查询（2026-11-04）—— 已实现，单测/构建通过；未打包

- 用户要求：显示 tool.lu 上“高精归属地定位（IP数据云）”字段，并在设置里
  新做一个页面，与“高级”里的出站连接检测分开。
- 设置：新增 `/settings#ipdata` 页面与侧栏“IP数据云”入口；字段
  `ipDataCloudKey` 加入 `AllSetting`/Zod/后端默认值，保存时按字符串处理
  （纯数字 Key 不转 number），去首尾空白并拒绝空白字符。
- 后端 `src/api/ip_location.rs`：配置 Key 后与纯真并行查询
  `https://api.ipdatacloud.com/v2/query`；成功附加 `ipDataCloud`
  （国家/省/市/区县/街道/运营商/场景/时区/经纬度），失败附加
  `ipDataCloudError`，不影响主结果；纯真失败时可用 IP数据云结果兜底。
- 前端 `ClientInfoModal`：位置结果分两行显示“纯真”和“IP数据云”；未配置
  Key 时只显示纯真，Key 错误时第二行显示错误。
- 验证：`cargo test` **112/112**、clippy 零警告、fmt 通过；前端 `npm run build`
  成功；`tsc` 无改动文件新增报错（既有 Storybook devDependencies 报错）；
  oxlint 仅剩 `ClientInfoModal` 既有 `set-state-in-effect`（非本次引入）。
  用假 Key 实测 IP数据云真实接口返回 `key不正确`，解析为错误；
  `tmp/e2e-advanced/ipdc-check.py`（设置保存、纯数字 Key 保持字符串、
  假 Key 返回 `ipDataCloudError`、清空 Key 后不再查询）与
  `tmp/e2e-advanced/ipdc-ui.py`（无头 Chrome 打开 `/settings#ipdata`，
  旧“高级”页仍正常）全部通过。
- **未验证**：用有效 Key 的成功响应与区县/街道精度；真实 Linux/musl 与
  真实公网客户端；IP数据云额度耗尽时的提示。
- 未打包、未 bump 版本（仍 `0.2.21`）。

## 2.34 版本策略改为每次修改都 bump + 0.2.22 推送（2026-11-04）

- 用户要求：推送当前代码；以后只要有修改，版本号都要增加。
- 版本：`Cargo.toml` / `Cargo.lock` 从 `0.2.21` → **`0.2.22`**（未打包、
  未打 tag）；本次改动包含 §2.32 的纯真主源与 §2.33 的 IP数据云功能。
- 规则：根 `CLAUDE.md` 规则 3 与会话结束清单已改为“每次代码修改都递增版本”，
  `docs/DECISIONS.md` 追加 D045；纯文档/记忆同步不单独 bump。
- 推送：提交 `01c2586`，已推送 `main`（`c7ce15a..01c2586`）。
- 验证：`cargo test` 112/112、clippy 零警告、fmt 通过；版本号随二进制显示
  `0.2.22`。**未打包、未创建 Release/tag。**

## 2.35 v0.2.22 发布（2026-11-04）—— 已验证

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.22`；tag `v0.2.22` → `99612d0`。
- Release 页面：只显示“0.2.22 更新内容”；一键安装/代理/在线更新/资产说明
  已移除，`generate_release_notes` 已关闭（见 `docs/DECISIONS.md` D046）。
- Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.22>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` **5,473,730 字节**，
  GitHub asset digest
  `sha256:c3a60555bd500a17ea52a53fe39ac56436ffe529c404a081e09e2cb1044c44e9`。
- Actions：release run `41853368185` 成功；发布说明更新 run `41854912426`
  成功；临时更新 workflow 已删除。
- 本地验证：`cargo test` **112/112**、clippy 零警告、fmt 通过、前端构建、
  假 Key 端到端与无头 Chrome 设置页检查均通过。GitHub API 已确认资产大小与
  digest；本机下载发布包因网络超时未完成，包内 `SHA256SUMS` 尚未本地复核。
- **未验证**：有效 IP数据云 Key 的成功响应（T28）；真实 Linux/musl 运行
  （T26）；真实公网客户端（T27）。

## 2.36 Release 标题去掉版本号（2026-11-04）

- 用户要求“更新内容标题前面不要显示版本号”。
- 修改 `.github/workflows/release.yml` 与 `.github/release-notes/v0.2.22.md`
  的标题为 `## 更新内容`；用一次性 workflow（run `41855673244`）更新已发布
  v0.2.22 的 body，随后删除临时 workflow。
- 验证：GitHub API 确认 v0.2.22 body 首行为 `## 更新内容`，不含
  “0.2.22 更新内容”或一键安装段落；版本由 Release 标题 `v0.2.22` 体现。
- 未打包/未 bump 版本（仍 `0.2.22`，纯 CI/文档调整）。

## 2.37 IP数据云移入高级标签页（2026-11-04）—— 已实现并 UI 验证；未打包

- 用户要求：IP数据云放在“高级”菜单里面，原有功能叫“出站检测”，两者在
  同一页面切换，样式与常规设置页一致。
- 前端：新增 `AdvancedTabs.tsx`（AntD Tabs：出站检测 / IP数据云）；
  `/settings#advanced` 渲染标签页；移除侧栏独立 IP数据云入口和 `#ipdata`
  路由；新增 `pages.settings.outboundCheckTab` 文案。
- 版本：`Cargo.toml` / `Cargo.lock` 0.2.22 → **0.2.23**（未打包/未打 tag）。
- 提交：`875703f feat: move IPDataCloud into advanced settings tabs`，已推送
  `main`（`a1bd649..875703f`）。
- 验证：`cargo test` 112/112、clippy 零警告、fmt 通过；前端构建成功；
  `tmp/e2e-advanced/ipdc-ui.py` 无头 Chrome（1440×900）验证：高级页显示
  “出站检测 / IP数据云”两个标签，可来回切换；IP数据云标签显示 API Key
  输入与提示；出站检测原有内容正常。
- 未验证：真实 Linux/musl；有效 IP数据云 Key 成功响应（T28）。

## 2.38 v0.2.23 发布（2026-11-04）—— 已验证

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.23`；tag `v0.2.23` → `a0aa557`。
- Release 页面：标题 `## 更新内容`（不带版本号），只列 0.2.23 更新条目；
  无安装/更新说明、无 Full Changelog。
- Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.23>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` **5,473,227 字节**，
  GitHub asset digest
  `sha256:9bd16e8409f3c38098ef730f05bfb8f8e7bd49764eedab8225e9e5f40ced3811`。
- Actions：release run `41859508942` 成功。
- 发布前验证：`cargo test` 112/112、clippy 零警告、fmt 通过、前端构建；
  `tmp/e2e-advanced/ipdc-ui.py` 无头 Chrome 验证高级页两个标签可切换。
- **未验证**：真实 Linux/musl 运行；发布包本地下载校验（网络受限）；
  有效 IP数据云 Key 成功响应（T28）。

## 2.39 流量统计与速度精度（2026-11-05，0.2.24）—— 已用假 StatsService e2e 验证

用户反馈入站/客户端流量“算得不对”、速度与实际使用对不上，要求以 3x-ui 为基准
核对。逐行对比后确认并修复：

**发现的问题**
1. 手动/定时/出站重置会调用 `QueryStats(reset=true)` 清零 Xray 计数器，且兼容层
   忽略失败后照样清 DB。重置失败/超时/并发时，下一轮把整段累计值重新计一遍。
2. 速度由前端对两次列表快照做差分。全部空闲时列表数据不变，effect 不执行，
   速度永久停留在最后一次值；采样相位与后端落库不对齐时刷新呈一档一档。
3. 客户端同时关联 MASQUE 与普通入站时，裸邮箱计数器被 per-attachment 路径
   吞掉（仅当没有任何 attachment 样本时才走旧聚合路径）。

**实现（对齐 3x-ui）**
- 所有重置（客户端/入站/全部/出站）只清数据库 `up/down`，**不重置 Xray 计数器、
  不清 `last_up/last_down` 基准**；删除 `needs_reconcile` 写入/读取、
  `reset_client_verified`、`mark_reconcile`、`StatsClient::reset_pattern`、
  `identity::traffic_pattern`。旧列保留兼容。
- 流量 job 每次成功采样后用本轮增量 ÷ 单调时钟真实间隔（`Instant`）计算
  `AppState.traffic_speeds`（入站 tag + 客户端邮箱，字节/秒）；空闲实体无条目。
  采样前/后比较 `start_epoch`，若期间 Xray 重启则丢弃该样本，避免新进程计数器
  减旧基准。
- API：`/inbounds/list/slim` 每行加 `upSpeed/downSpeed`；
  `/clients/list`、`/list/paged`、`/get/{email}` 的 `traffic` 加同名字段。
- 前端 `useInbounds`/`useClients` 删除差分/缓存逻辑，直接展示后端速率；
  DBInbound / ClientTraffic schema 增加对应字段。空闲时后端返回 0，速度标签
  在一个采集周期内消失。
- MASQUE 裸邮箱样本按 MASQUE attachment 统计；多 MASQUE 入站共享一个裸邮箱
  计数，只计一次。

**验证**
- `cargo test` 113/113（新增 `speed_divides_the_delta_by_the_real_sample_interval`、
  `masque_bare_email_counter_survives_a_normal_attachment`、
  `resetting_totals_keeps_baselines_and_bills_only_new_bytes`；移除 reconcile 测试）。
- `cargo clippy --all-targets -- -D warnings` 零警告；`cargo fmt --check` 通过。
- 前端 `npm run build` 成功；`tsc --noEmit` 改动文件无报错（全套仍有 429 个
  既有 devDependencies 缺失报错）。
- `tmp/e2e-traffic-accuracy/e2e.py`（真面板 + fake xray + fake StatsService）
  13 项全通过：首次计费、速度存在且上下行比例正确、空闲清速度、客户端重置
  不重复计费、入站重置不重复计费且清成员客户端累计。
- `tmp/e2e-traffic-real/e2e.py`（真实 Xray 26.10.31 + 真实
  SOCKS→VLESS→本地 HTTP，限速 ~2 MB/s）15 项全通过：8 MiB 下载后客户端
  `down=8,388,725`（响应体 + 117 字节 HTTP 头），入站 `down=8,388,608`
  （响应体），差 117 字节/0.0014%；观测速度 `2,158,371 B/s` 与限速一致；
  空闲 3 秒速度归零；客户端重置后仅再下载 2 MiB，只新增 `2,097,269`；
  入站重置同时清零成员客户端且随后只计新字节。

**已知口径**：入站计数器与客户端（user）计数器在 Xray 中不同层采集，可能相差
HTTP 头/协议帧等少量字节（本次实测 117 B），不要求逐字节相等；3x-ui 相同。
**未验证**：真实 Linux/musl、真实公网网络速度抖动、MASQUE 混合附着的真实
计数、gRPC 故障注入。见 `KNOWN-ISSUES.md` G6–G8。

## 2.40 恢复 3x-ui 实时流量 WS 推送（2026-11-05，0.2.25）—— 已验证并发布

用户反馈“入站没有关联客户端时速度显示完全错误”“实时速度显示有问题”，要求
入站/客户端的流量统计与速度显示以 3x-ui 为准。

- 后端：新增会话保护的 `/ws`（axum `ws` feature），流量 job 每个采集周期广播
  3x-ui 同款 `traffic` 帧（入站/客户端增量、在线集合、最后在线、采集间隔）。
  统计查询失败时广播空增量清空旧速度；慢客户端由 `Lagged` 跳过旧帧。
- 前端：重新启用 `useWebSocket`；入站/客户端速度按帧内
  `clientTrafficIntervalMs`（2–300s）换算，不再写死 5s；客户端速度合并时
  WS 覆盖 REST 同邮箱值，避免重复相加。0.2.24 的 REST `upSpeed/downSpeed`
  保留为 WS 不可用时的兜底。
- 无客户端入站：速度只依赖 `inbound>>>tag>>>traffic` 增量，已单独验证
  （API 与 UI 均通过）。
- 验证：
  - `cargo test` 113/113、clippy 零警告、fmt、前端 build。
  - `tmp/e2e-traffic-accuracy/{e2e,noclient}.py`：分别 13/13、4/4 PASS。
  - `tmp/e2e-traffic-real/e2e.py`：真实 Xray 26.10.31，15/15 PASS。
  - `tmp/e2e-traffic-ui/ui_run.py`（无头 Chrome + 3x-ui UI）10/10 PASS：
    两个入站行（其中一个**无客户端**）都出现速度标签；收到真实
    `type:"traffic"` WS 帧，含无客户端入站增量、客户端增量、
    `clientTrafficIntervalMs=2000`；空闲后两个速度标签都消失。
- 已发布：GitHub `v0.2.25`（详见 §2.41）。

## 2.41 v0.2.25 发布（2026-11-05）—— 已验证

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.25`；提交 `ade85bd`，
  tag `v0.2.25`。
- Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.25>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` **5,516,624 字节**，
  GitHub asset digest
  `sha256:d9128d0c349011dd0f8d22829857c8cf4c3b3e8002e8d3f62c0bb8d84db792a5`。
- Actions：release run `41884947044` 成功；main CI run `41884941034` 成功。
- 发布前验证：`cargo test` 113/113、clippy 零警告、fmt、前端 build；
  fake e2e 13/13、无客户端入站 4/4、真实 Xray 15/15、无头 Chrome UI+WS
  10/10。
- **未验证**：真实 Linux/musl 运行、真实公网网络抖动、gRPC 故障注入。

## 2.42 实时 WS 流量帧内存优化（2026-11-05，0.2.26）—— 已验证并发布

- 版本 `Cargo.toml` / `Cargo.lock` = `0.2.26`；提交 `4ccbec5`，tag `v0.2.26`
  （指向 `4ccbec5`）。Release
  <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.26>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` **5,518,877 字节**，
  asset digest
  `sha256:9c223ffae10e0c308c67078d90bef97dad3fb8ea02e07831114fd751c1baae2f`。
- Actions：release run `42015308253` 成功；main CI run `42015280025` 成功。

用户反馈 0.2.25 之后面板内存一直很高。定位到 0.2.25 的流量广播路径引入的
常驻/瞬时占用，已优化：

- **无订阅者仍构建帧**：`jobs::tick` 原先无论有没有浏览器连接都收集
  `ws_traffics`/`ws_client_traffics`，`broadcast_traffic` 还会把完整
  `{traffics, clientTraffics, onlineClients, onlineByGuid, activeInbounds,
  lastOnlineMap}` 序列化一遍，最后 `send` 无接收者返回 Err。现在 tick 开头
  读取 `ws_events.receiver_count()`，为 0 时跳过增量收集；broadcast 入口也
  再检查一次。面板页没打开时不再产生任何帧分配。
- **Value 树开销**：原实现每个入站/客户端条目都用 `json!` 建
  `serde_json::Value`（每条约 1 个 Map + 3 个 key String），一帧几千条时
  峰值和分配次数翻倍。现在增量用借用 `&str` 的 `InboundTraffic`/
  `ClientTraffic` 结构体，整个帧直接 `serde_json::to_string`，不再建树。
- **广播缓冲**：容量 8 → 2。慢客户端由 `Lagged` 跳过旧帧，只有最新帧有意义；
  每帧随活跃客户端增长，减少常驻的旧帧副本。
- **僵尸连接**：`/ws` 每 30 秒发 Ping，90 秒无任何入站帧（浏览器会自动回
  Pong）断开；所有发送包 10 秒超时。休眠笔记本、断网/半开 NAT 的连接不再
  长期占着任务和待发帧。

验证（Windows debug + fake StatsService，3000 客户端，2 秒采集，每秒有流量）：

| 场景 | 0.2.25 | 0.2.26 |
| --- | --- | --- |
| 无 WS 连接，末值 / 峰值 RSS | 28.2 / 34.1 MiB | 27.0 / 32.8 MiB |
| 一个 WS 读取端，末值 / 峰值 RSS | 28.8 / 34.7 MiB | 27.8 / 33.7 MiB |

收益与活跃客户端数近似线性（帧大小随条目数增长）；面板页未打开时 0.2.26
完全不构建帧。其他回归：`cargo test` 113/113、clippy 零警告、fmt；
fake e2e 13/13、无客户端入站 4/4、无头 Chrome UI+WS 10/10；抓取的 WS 帧
字段名/值与 0.2.25 完全一致（`Tag/Up/Down/IsInbound/IsOutbound`、
`email/up/down`、camelCase 其余字段）。
**未验证**：真实 Linux/musl 长时运行 RSS、超大客户端规模下的真实内存曲线。

## 2.43 实时速度除数修复（2026-11-05，0.2.27）—— 已验证，已随 0.2.27 发布

用户反馈“入站的实时速度还是显示不准确”。定位到 WS 帧的除数问题：

- 症状（可复现）：StatsService 慢 3 秒、轮询设为 2 秒时，实际 1.0 MB/s，
  WS/UI 显示 **1.5 MB/s**（50% 虚高）；REST `upSpeed` 正确显示 1.00 MB/s。
- 根因：`traffic` 帧的 `clientTrafficIntervalMs` 固定为配置轮询值
  （`poll_secs*1000`），而帧内增量是相对上一轮成功采样的真实窗口（本次
  `elapsed_ms`）的差分。tick 超过轮询周期、RPC 超时后恢复、设置刚改变时，
  两者不等，前端按配置值除就按比例放大。首轮（面板/Xray 刚启动）没有上一
  采样点，增量是整个启动以来的计数，也被当成一个轮询窗口广播 → 一次性尖峰。
- 修复（`src/jobs.rs`）：
  - 帧的 `clientTrafficIntervalMs` 改为本轮真实 `elapsed_ms`；
  - `elapsed_ms <= 0` 时累计照常计费，但不广播速率增量（浏览器清空速度），
    新增纯函数 `rated_interval_ms`；
  - Xray 采样期间重启时额外广播空增量，立即清掉旧速度；
  - 无速率可播的帧仍回退为配置轮询值，保持 3x-ui 客户端约定。
- 文件：`src/jobs.rs`、`docs/{ARCHITECTURE,API,DECISIONS}.md`（D052）、
  `tmp/fake_stats` 增加 `FAKE_STATS_DELAY_MS` 测试钩子、
  `tmp/e2e-traffic-ui/ui_check.mjs` 间隔断言改为区间。
- 验证：
  - `cargo test` **114/114**（新增 `ws_frame_divisor_is_the_real_window_not_the_configured_poll`）；
    clippy 零警告；fmt 通过。
  - 新增 `tmp/diag-traffic-ws/diag.py`：真实 `/ws` 客户端 + fake StatsService，
    已知计数器速率。慢 RPC 3s/2s 轮询：修复前帧间隔 2000ms、显示
    1.500/3.000 MB/s；修复后帧间隔 3010–3027ms、显示 0.998/1.995 MB/s，
    与真实 1.0/2.0 MB/s 及 REST 一致。首轮空帧无尖峰。
  - `tmp/e2e-traffic-accuracy/e2e.py` 13/13；`tmp/e2e-traffic-ui/ui_run.py`
    10/10（无客户端入站速度仍正常）；`tmp/e2e-traffic-real/e2e.py`
    真实 Xray 26.10.31 15/15（入站/客户端速度 2,156,219 B/s 与限速一致）。
  - `tmp/diag-traffic-ws/tunnel_real.py`：真实 Xray 26.10.31 的 tunnel
    （dokodemo，rewriteAddress）入站、**零客户端**，8 MiB 下载后入站
    `down=8,388,725`、`up=82`，REST 与 WS 峰值速度均 ~2,160,527 B/s，
    空闲归零。证明无客户端绑定也能按入站计数器显示实时速度。
- **未验证**：真实 Linux/musl、真实公网网络抖动、超大客户端数下 tick 超时
  的实际比例；前端未改动，构建产物沿用 0.2.26。
- 已随 0.2.27 发布（§2.44）。

## 2.50 v0.2.30 发布（2026-11-06）—— 已验证（GitHub Actions + 本地包级复核）

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.30`；提交 `c3bd5b5`，tag `v0.2.30`。
- Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.30>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` **5,527,291 字节**，
  GitHub asset digest
  `sha256:d4c2c816efa440567bb9271211e675e6af1d5bb31f589d06af95f39ad70bf429`。
- Actions：main CI run `42067360254` 成功；release run `42068104145`
  （head_sha = `c3bd5b5`）成功。Release body 四条更新内容中文 UTF-8 存储正确。
- 本地包级复核（走 `192.168.1.251:10813` HTTP 代理下载）：包 sha256 与
  digest 一致；解包校验包内 `SHA256SUMS` 与二进制
  `ba98ca739144ae24bb9b453a76f417ebbe911950e9e20b3989dfc2975f7ab50c` 匹配；
  二进制 13,000,368 字节，含 `LUMEN_VERSION=0.2.30` 与 WS 帧字段
  `clientTrafficIntervalMs`。解包产物存 `tmp/pkg-0.2.30/`。
- 发布前验证：`cargo test` **121/121**、clippy 零警告、fmt；真实 Xray tunnel
  与 fake-stats 两条本地链路 PASS（§2.49）。
- **未验证**：真实 Linux/musl 大面板运行效果——tunnel 实时速度是否追平
  3x-ui/网卡监控（允许包头 3–8% 结构差），待用户部署反馈（G14）。

## 2.49 采样与计费解耦：串行计费 writer（2026-11-06，0.2.30）—— 已发布

用户报「tunnel 协议的流量显示还是不准确」（v0.2.29 部署后），追问确认：
累计量正常、**实时速度比 3x-ui 小很多**，轮询间隔设置是 **2 秒**。

- 本地先排除 tunnel 特殊性：`tmp/diag-traffic-ws/tunnel_real.py`（真实
  Xray 26.10.31 + rewriteAddress）PASS——8,388,608 字节下载 → 面板增量
  8,388,725（+82 B 为 HTTP 头），WS 帧间隔 [2003,2015,2016]ms 稳定；
  fake-stats 全链路（`diag.py`）帧 1.01/2.03 MB/s vs 真值 1.0/2.0 MB/s。
- 对照 3x-ui 找差异（`tmp/xui-ref/`）：它的速度 = 增量 ÷ **硬编码 5s**，
  且 DB 计费在独立串行 writer（`submitTrafficWrite`），统计 job 节拍不沾
  写库。本面板 tick 是「采样 → 计费写 → 广播」串行：大部署 + poll=2s 时
  一轮 DB 写拖十几秒 → 采样间隔被拉长 → 速度 = 十几秒平均 ≪ 瞬时值，
  且一处写失败整帧丢失。用户拿 3x-ui/网卡监控做参照，遂显得"小很多"。
- 重构（D055/G14）：`src/jobs.rs` tick 显示路径先跑完（traffic_speeds、
  在线/活跃、WS 广播），随后把**原始计数器**打包 `BillingSample` 推入
  `BillingSink`（单槽合并：`Arc<Mutex<Option<_>>>`+`Notify`，新样本取代
  未落库旧样本；内存上界=一个样本，低内存约束）；独立 tokio 任务
  `run_billing_writer` → `apply_billing` 串行执行全部计费 DB 写（attachment
  基准比较、配额/到期/limitIp 禁用、reset_days 重置、last_online 心跳、
  出站表、全局历史），失败只记 warn；样本 epoch 与当前进程不符整份丢弃
  （防死进程累计对着清零基准重复计费）。在线/活跃的「本轮有流量」信号改
  用显示增量。心跳持久化移出广播之前（广播不再被 DB 写挡住）。
- 版本 0.2.29 → 0.2.30（Cargo.toml/Cargo.lock）。文档：D055、G14、
  ARCHITECTURE §6 tick 流程、CLAUDE.md、release.yml body（0.2.30 四条）。
- 验证：`cargo test` **121/121**（新增 2 个合并槽单测：最新胜出、唤醒
  等待者）、clippy `--all-targets -D warnings` 零警告、fmt；真实 Xray
  tunnel + fake-stats 两条 WS 链路 PASS。
- **未验证**：真实 Linux/大面板现场（本轮症状场景）；需用户部署 0.2.30
  确认速度列与网卡监控/3x-ui 的差距是否收敛到包头级（3–8%）。

## 2.48 v0.2.29 发布（2026-11-05）—— 已验证（GitHub Actions + 本地包级复核）

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.29`；提交 `117b418`，tag
  `v0.2.29`。
- Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.29>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` **5,523,004 字节**，
  GitHub asset digest
  `sha256:c3b86b06cded35a39fe676eff80ab344468294dce83c6d60dfe8819c631896e3`。
- Actions：main CI run `42056105722` 成功；release run `42057016611`
  （head_sha = `117b418`）成功。Release body 中文 UTF-8 存储正确
  （更新内容：3x-ui 模型重写 / 64 MiB 上限 / 分族查询 / 显示与 DB 解耦）。
- 本地包级复核（走 `192.168.1.251:10813` HTTP 代理）：资产 sha256 与
  digest 一致；解包后包内 `SHA256SUMS`
  `524be1513bf29823b59c4fec74d9a9bd73a48fe3d87f159062d7addc756daedb` 与
  包内二进制一致；二进制 12,991,472 字节，含 `LUMEN_VERSION=0.2.29` 与
  WS 帧字段 `clientTrafficIntervalMs`。解包产物存 `tmp/pkg-0.2.29/`。
- 发布前验证：`cargo test` **119/119**、clippy 零警告、fmt（§2.47）。
- **未验证**：真实 Linux/musl 运行与真实大面板部署效果——0.2.29 的
  修复目标（速度列恢复显示）需用户现场部署后确认（G13）。

## 2.47 按 3x-ui 模型重写实时流量取数（2026-11-05，0.2.29）—— 已实现，未验证

用户部署 0.2.28 后仍报「大流量面板速度没有显示」。下载并逐行对照上游
3x-ui 的取数实现（`internal/xray/api.go` GetTraffic，存于
`tmp/xui-ref/`），确认三项结构性差异并按其模型用 Rust 重写（D054/G13）：

- **(a)** gRPC 接收上限：上游显式 64 MiB（注释明说 ~45k 客户端的回复
  可单独超过默认 4 MiB，超限整轮失败）；本面板未设置——大面板速度全空
  的最大嫌疑。已在 `StatsClient::connect` 设置
  `max_decoding_message_size(64 << 20)`。
- **(b)** 分族查询：上游 `user>>>`/`inbound>>>`/`outbound>>>` 各一次
  QueryStats，单族失败其余族照常；本面板单次 `query("")`。已改为
  `query_families()` 返回 `Vec<FamilyStats>`，仅全部族失败才走失败路径。
- **(c)** 显示与 DB 解耦：上游显示增量基线在内存（`StatsLastValues`，
  每次成功查询即推进），本面板显示复用 DB `last_*`。已新增内存基线
  引擎 `display_deltas`（族首过只记基线、未见/回退从 0 起算、族失败
  保留基线下轮按跨度补算、全部族成功且 map>2×live 时剪枝）；WS 帧、
  `traffic_speeds`、本轮流量信号改用内存增量；DB 计费
  （`apply_tick`/`add_traffic`/配额执行）保持原状。
- 低内存硬约束的落实：基线键用紧凑 `DeltaKey{kind,uplink,key}`（省
  ~30 B/条）、只记录 traffic 计数器、`2×live` 阈值剪枝、map 上界 =
  现存 traffic 计数器数；无订阅者不建 WS 帧（0.2.26 优化沿用）。
- 前端 WS 帧协议与 0.2.25 一致，**未改动前端**。

**验证**：`cargo test` **119/119**（新增 5 个引擎单测：首过/未见与
回退/族失败跨度补算/身份折叠/剪枝）、clippy 零警告、`cargo fmt --check`。
**未验证**：真实 Linux/大面板部署——这正是本次要修的场景，0.2.29 发布
后需要用户现场确认速度列恢复显示。

## 2.46 v0.2.28 发布（2026-11-05）—— 已验证（GitHub Actions + 本地包级复核）

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.28`；提交 `5e376a1`，tag `v0.2.28`。
- Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.28>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` **5,518,877 字节**，
  GitHub asset digest
  `sha256:f7ebdfba2946e487c503d77d535ba36714d24164578f381be1b6e125e6d58290`。
- Actions：main CI run `42048884488` 成功；release run `42049648625`
  （head_sha = `5e376a1`）成功。Release body 中文 UTF-8 存储正确。
- 本地包级复核（走用户提供的 `192.168.1.251:10813` HTTP 代理，本次成功）：
  资产 sha256 与 digest 一致；解包后包内 `SHA256SUMS`
  `074a92939a53a48e630715012ed5f1642b2c24077bb2aba5595ea4f33af05e8c` 与
  包内二进制一致；二进制含 `LUMEN_VERSION=0.2.28` 与 WS 帧字段
  `clientTrafficIntervalMs`（前端已嵌入）。包内二进制 12,980,672 字节。
- 发布前验证：`cargo test` **114/114**、clippy 零警告、fmt、前端构建。
- **未验证**：真实 Linux/musl 运行与真实断线场景（D053/G12）。

## 2.45 入站 WS 速度断线冻结修复（2026-11-05，0.2.28）—— 已实现，未验证

用户在 0.2.27（Linux 部署）再次反馈入站列表实时速度虚高/偏低。逐行核对
`src/jobs.rs` 速率链路（增量 ÷ 真实单调间隔、计数器回退、重启丢弃样本）
确认后端计算无误，本地 fake/真实 e2e 亦覆盖幅度正确。定位到前端缺陷：

- `useClients.ts`（客户端页）有 WS 速度过期计时器；`useInbounds.ts`（入站
  页）没有。WS 半途断开时 `wsInboundSpeed` 冻结在断开瞬间的数值，并被合并
  逻辑（`{...serverInboundSpeed, ...wsInboundSpeed}`）永久覆盖 REST 兜底值
  ——空闲时旧速度不归零（虚高），期间真实速率变化也不跟随（偏低/跳变）。
- 修复（仅前端 `useInbounds.ts`）：每收到一帧 traffic 就重置 2×帧间隔
  （≤2 分钟）过期计时器（版本计数门闩）；超时清空整个 `wsInboundSpeed`，
  回退每轮刷新的 REST `upSpeed/downSpeed`；组件卸载清理计时器。语义与
  客户端页一致（DECISIONS D053、KNOWN-ISSUES G12）。

**验证**：`cargo test` 114/114、clippy 零警告、`cargo fmt --check`、前端
`npm run build` 成功。
**未验证**：真实 Linux 部署断线场景（代理掐长连接/睡眠恢复）；若 WS 存活时
仍有幅度偏差，需要用 DevTools `__diagDump`（WS 帧 + REST 速率）对照系统
状态页网卡吞吐采集现场数据后继续定位。

## 2.44 v0.2.27 发布（2026-11-05）—— 已验证（GitHub Actions）

- 版本：`Cargo.toml` / `Cargo.lock` = `0.2.27`；提交 `869facb`，tag `v0.2.27`。
- Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.27>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` **5,518,387 字节**，
  GitHub asset digest
  `sha256:d02091227e04682c8c26328c924f03394d68318fd79b3cbdaa9cb96d5c65138b`。
- Actions：main CI run `42042345225` 成功；release run `42042854755` 成功。
- 发布前验证：`cargo test` **114/114**、clippy 零警告、fmt、前端构建；
  慢 tick 诊断（3s RPC/2s 轮询）修复前 1.5× 虚高、修复后 0.998/1.995 MB/s；
  fake e2e 13/13、UI+WS 10/10、真实 Xray 15/15；真实 tunnel（dokodemo）
  零客户端入站 8 MiB，REST/WS 峰值 ~2,160,527 B/s。
- 本机下载发布包被网络重置（`curl: (35) Recv failure: Connection was reset`），
  未做本地解包/`sha256sum -c` 复核；资产大小与 digest 来自 GitHub API，
  构建与包内校验由 release workflow 完成。
- **未验证**：真实 Linux/musl 运行；tunnel followRedirect/TPROXY 模式；
  其余入站协议的真实流量统计；超大客户端数导致 tick 超时的真实比例。

## 2.51 实时速度改为 1 秒显示采样（2026-11-06，0.2.31，未打包）
- 状态：已实现并本地验证；真实部署待验证。
- 背景：0.2.30 部署后用户仍报入站/客户端实时速度不准。现场四方对照
  （Xray 计数器 / WS 帧 / REST / DB）证明字节增量与 `增量 ÷ 真实间隔`
  一致、计费累计一致；差异来自显示只有每 `traffic_poll_secs`（现场 5 秒）
  一次，速度列是 5 秒平均值，与 1 秒网卡监控在爬坡/结束/突发跨窗口时相差
  几十个百分点。本地 4 MiB/s 限速下载实测窗口边界 3.22 / 4.00 / 2.33 MB/s，
  真实值稳定 4.00 MB/s。
- 改动：
  - `src/jobs.rs`：显示 tick 与计费 tick 分离。有 `/ws` 订阅者且客户端数
    ≤2000 时显示 tick 为 1 秒；否则回到配置轮询。计费、在线 API、
    `last_online` 心跳、roster 加载仍只在计费 tick 执行。显示 tick 复用
    roster，并把 1 秒增量的移动邮箱/活跃入站 tag 累积到计费 tick。
  - `Roster` / `TrafficTick` 结构减少 tick 参数与重复 DB 读取；
    `frame_interval_hint` 让失败帧也携带 1 秒除数。
  - 文档：`docs/DECISIONS.md` D056、`docs/ARCHITECTURE.md` §6/§6.1。
- 版本：0.2.30 → 0.2.31（`Cargo.toml` / `Cargo.lock`）。
- 验证：`cargo test` **121/121**、clippy 零警告、fmt；fake-stats 计费
  e2e 13/13；无头 Chrome UI+WS 10/10（速度标签出现/清除、帧间隔与除数）；
  真实 Xray tunnel 回归 PASS（WS 帧间隔 ~1.0 秒，峰值与 REST/Xray 一致）。
- 未验证：真实 Linux/musl 大面板的 1 秒采样 CPU/内存；>2000 客户端自动
  回退到轮询的实际效果。

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
