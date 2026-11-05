# DEVELOPMENT-LOG

> **追加式日志：只允许在文件末尾追加新条目，不得修改或删除历史条目。**
> 记录粒度：一次功能/一次修复/一次会话收尾。
> 2026-10-31 建立本文件之前的历史，详见根目录 `PROGRESS.md`（不要重写它）。

条目模板：

```
## YYYY-MM-DD HH:MM — 标题
- 状态：已完成并验证 / 已实现未验证 / 进行中
- 改动：
- 涉及文件：
- 验证：
- 备注/风险：
```

---

## 2026-10-31 17:36 — 建立长期记忆管理系统
- 状态：已完成并验证（文档类，无需运行时代码验证）
- 改动：
  - 新建根目录 `CLAUDE.md`：会话启动流程、不可违反规则、核心约束、构建命令、
    记忆/文档索引、会话结束清单。
  - 新建 `memory/`：`CURRENT-STATUS.md`、`DEVELOPMENT-LOG.md`、`TODO.md`、
    `NEXT-STEPS.md`。
  - 新建 `docs/`：`ARCHITECTURE.md`、`DATABASE.md`、`API.md`、`DECISIONS.md`、
    `KNOWN-ISSUES.md`。
  - 明确“继续开发前必须核对真实代码状态，禁止仅凭记忆猜测”的规则。
- 涉及文件：上述新建文件；未修改业务代码。
- 验证：逐项对照 `src/`、`migrations/`、`README.md`、`PROGRESS.md` 实际内容撰写；
  数据库字段来自迁移文件，API 路径来自 `src/api/mod.rs`。
- 备注/风险：文档会随代码演进过期；每次功能完成/会话结束必须更新。

## 2026-10-31 17:34 — 面板路径自定义 / 版本号位置 / 定期内存释放
- 状态：已完成并验证；**未打包**（遵守“用户要求才打包”的约定）
- 改动：
  1. 面板路径可自定义（3x-ui 设置页「常规 → URI 路径」/`webBasePath`）：
     - 新增 `src/web/base_path.rs`：路径规范化、前缀剥离、路由前 URI 重写。
     - `src/api/mod.rs`：用外层 Router 的 fallback service 包住内层 Router，
       确保 guard 在路由匹配前运行（同层 `.layer` 会在路由选择后才执行）。
     - `src/web/mod.rs`：静态入口按前缀注入 `X_UI_BASE_PATH`、改写
       `/assets/...` 资源 URL；独立 `/settings` 页注入前缀。
     - `src/api/compat.rs`：保存时校验/规范化 `webBasePath`，更新内存前缀；
       `setting/all` 返回实时值。
     - 前端：设置页新增 URI 路径输入；保存路径变化后自动跳转新地址；
       侧边栏设置链接带前缀。
  2. 版本号只在系统状态页：删除侧边栏品牌/抽屉版本号与相关 CSS，
     保留系统状态页版本按钮/更新弹窗。
  3. 定期内存释放：维护任务改为每 15 分钟（会话清理、SQLite
     `shrink_memory`、内部容器 `shrink_to_fit`、glibc `malloc_trim(0)`）。
- 涉及文件：
  - 后端：`src/web/base_path.rs`（新）、`src/web/mod.rs`、`src/api/mod.rs`、
    `src/api/compat.rs`、`src/state.rs`、`src/main.rs`
  - 前端：`frontend-3xui/src/pages/settings/GeneralTab.tsx`、
    `.../SettingsPage.tsx`、`frontend-3xui/src/layouts/AppSidebar.tsx`、
    `.../AppSidebar.css`
  - 产物：`web/dist/`（重新构建）
  - 文档：`PROGRESS.md` 追加本节说明
- 验证：
  - `cargo test` 40/40（含 `web::base_path` 4 个新测试）。
  - `cargo clippy --all-targets -- -D warnings` 零警告。
  - 前端 `npm run build` 成功；改动文件 typecheck 无新增错误；改动文件 oxlint 无新增警告。
  - `cargo zigbuild --release --target x86_64-unknown-linux-musl` 成功，
    产物为静态 ELF、已 strip。
  - fake Xray + 无头 Chrome 端到端：设置页改 `/secretui/` → 自动跳到
    `/secretui/panel/settings`；旧 `/`、`/panel` 404；`/secretui/assets/*`、
    登录态 API、独立 `/secretui/settings` 正常；改回 `/` 恢复；无控制台错误。
  - 维护任务临时改 5 秒：debug 日志持续输出
    `maintenance: released idle panel memory`，无 SQLite/panic 错误；已恢复 15 分钟。
- 备注/风险：
  - 未在真实 Linux + 真实 Xray 上回归；本次仅有 Windows debug + fake Xray
    与 musl 交叉编译证据。
  - README 中轮询默认值等描述与实际代码不一致，已记入 `TODO.md`。
  - 本次未 bump 版本号、未生成发布包。

## 2026-10-31 17:42 — 打包发布 0.2.6
- 状态：已完成并验证（产物级验证；未在真实 Linux + 真实 Xray 上运行）
- 改动：
  - `Cargo.toml` 版本 `0.2.5` → `0.2.6`。
  - 重新构建前端 `web/dist`、musl release 二进制。
  - 生成正式包 `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`，目录结构沿用上一版
    （二进制、README、install.sh、service/env 示例、SHA256SUMS）。
  - 先归档上一版 0.2.5：
    `dist/xray-panel-rs-linux-amd64-lumen-0.2.5.tar.gz`（SHA256
    `f3c10e2e3a610c540a35002e179a221a849f4e7b933d2a0aa77c588efcb696f2`）。
- 涉及文件：`Cargo.toml`、`Cargo.lock`、`web/dist/`、`dist/` 下发布产物、
  `PROGRESS.md`、`memory/*`、`CLAUDE.md`。
- 验证：
  - `cargo test` 40/40；`cargo clippy --all-targets -- -D warnings` 零警告。
  - `npm run build` 成功；`cargo zigbuild --release --target
    x86_64-unknown-linux-musl` 成功。
  - 包内二进制与交叉编译产物 `cmp` 一致；`file` 为静态 ELF、已 strip；
    `gzip -t` 通过；二进制内含版本串 `0.2.6`。
- 产物哈希：
  - tar.gz：`52dfe0d993f01f4f525f363e1892eae45ac03c0899982cd853ec32b675f584be`
  - 二进制：`a67fbf5479e6dc1ee5bb97faccca2b3ceeabe1632e808ab92cc2d40d226c9425`
- 备注/风险：本次未在真实 Linux + 真实 Xray 上运行该二进制；功能行为验证来自
  此前的 Windows debug + fake Xray + 无头 Chrome 端到端。

## 2026-10-31 17:44 — 生成 0.2.6 源码包
- 状态：已完成并验证
- 改动：
  - 生成 `dist/xray-panel-rs-0.2.6-src.tar.gz`，根目录为
    `xray-panel-rs-0.2.6-src/`。
  - 内容：`Cargo.toml/Cargo.lock/build.rs/LICENSE/README/CLAUDE/PROGRESS`、
    `src/`、`proto/`、`migrations/`、`deploy/`、`docs/`、`memory/`、
    `internal/`、`web/dist/`（编译 Rust 所需的已构建前端）、
    `frontend-3xui/` 与 `frontend/` 源码。
  - 排除：`target/`、`tmp/`、`dist/`、`node_modules/`、`web/dist-3xui`、
    `web/dist-custom-v2`、`frontend/dist`。
- 涉及文件：`dist/xray-panel-rs-0.2.6-src.tar.gz`；未修改业务代码。
- 验证：
  - 解压后与工作区逐目录 `diff`：`src`、`migrations`、`deploy`、`docs`、
    `memory`、`frontend-3xui/src`、`Cargo.toml`、`Cargo.lock` 完全一致。
  - 解压目录内 `cargo metadata --no-deps --offline` 成功；版本 0.2.6；
    `web/dist/index.html` 存在。
  - 归档大小 4,531,619 字节；SHA256
    `021ce74d8f98ddc3ec99138dc8eb1eddcad52cf0a78e74b83d096b801217a3b1`。
- 备注：源码包内 `web/dist/` 是已构建前端，保证无 Node 环境时也能直接
  `cargo build`；重新构建前端需要 Node >= 26。

## 2026-10-31 21:17 — v0.2.7 流量统计与在线状态可靠性修复（未打包）
- 状态：已实现，单元/事务测试通过；真实 Xray 故障注入未验证
- 背景：ChatGPT 对 0.2.6 的静态审查列出 8 个问题（重启少计、重置失败仍清零、
  缺失统计当 0、多入站聚合基准、下载单独回绕、在线 API 重探、未知在线身份、
  在线状态仅内存）。逐条核对真实代码后确认前 7 项需要修改；第 7 项（在线状态
  仅内存）维持原设计，第 8 项（在线状态设计取舍）不修改。
- 改动：
  1. `migrations/0007_client_inbound_traffic.sql`：新增 per-attachment 基准表
     （`last_up/last_down/needs_reconcile`），回填现有 `client_inbounds`。
  2. `src/traffic.rs`：`counter_delta`；缺失/负数返回 `None`（保留基准），
     计数器下降视为重置并按当前值计新增。
  3. `src/db/client_traffic.rs`：
     - `apply_tick` 单事务完成“读基准→逐 attachment 算增量→写基准→累加客户端”，
       任一步失败整体回滚；
     - `reset_all_baselines`（generation 变化，清 `last_*` 不清 `up/down`）；
     - `reset_client_verified`（仅全部重置确认成功后清零）；
     - `mark_reconcile`（部分失败标记，下一份样本只接管基准不计增量）。
  4. `src/jobs.rs`：
     - `generation: Option<u64>`，首个 tick 与每次 generation 变化都重置基准；DB
       重置失败则本轮不跑 tick；
     - 按 `<inbound_id>:<email>` 逐 attachment 采样，裸邮箱外部配置走旧聚合路径；
     - 客户端/入站/出站全部使用 `Option` 样本，上/下行分别比较；
     - 定时客户端/入站重置：全部成功才清库，否则标记 reconcile 并记录本次尝试；
     - 在线用户解码丢弃未知统计身份（裸邮箱需存在于 clients 才接受）。
  5. `src/xray/stats.rs`：`invalidate_online_api_probe()`，generation 变化时调用。
  6. `src/db/clients.rs` / `src/db/inbounds.rs`：`touch_reset_time`。
  7. `src/api/backup.rs`：备份/恢复表清单加入 `client_inbound_traffic`。
  8. 文档：ARCHITECTURE §6、DATABASE（新表/迁移/不变量）、DECISIONS D019、
     KNOWN-ISSUES G1–G6。
- 涉及文件：`migrations/0007_client_inbound_traffic.sql`、`src/traffic.rs`、
  `src/db/client_traffic.rs`、`src/db/mod.rs`、`src/db/clients.rs`、
  `src/db/inbounds.rs`、`src/jobs.rs`、`src/xray/stats.rs`、`src/api/backup.rs`、
  `src/main.rs`、`docs/*`、`memory/*`。
- 验证：
  - `cargo test`：48 passed / 0 failed（原 40 + 新增 8）。
  - `cargo clippy --all-targets -- -D warnings`：零警告。
  - 新增测试覆盖：逐 attachment 增量、缺失方向保留基准、部分重置 reconcile、
    全量重置不动累计、确认重置清基准、计数器回绕、未知在线身份过滤。
  - **未做**：真实 Linux + 真实 Xray 重启/部分重置/gRPC 超时故障注入。
- 备注：Cargo 版本保持 0.2.6，未打包（遵守 D013）。升级到该源码后首次 tick 会
  清零全部统计基准；升级前建议备份数据库。残留 Xray 进程场景的风险已写入
  DECISIONS D019 与 KNOWN-ISSUES G6。

## 2026-10-31 21:20 — 补充：基准重置时机改为 start_epoch
- 状态：已实现，单元测试/clippy 通过
- 背景：上一条实现按 `generation` 变化重置基准。supervisor 在**停止**和启动时
  都会递增 generation，若在停止阶段重置，旧进程尚未退出时的一次采样可能被当成
  新流量重复计入。
- 改动：
  - `src/xray/supervisor.rs` 新增 `start_epoch` 原子计数，只在 `start_child`
    成功后递增；`XrayHandle::start_epoch()` 暴露给 job。
  - `src/jobs.rs` 改为：新进程启动（epoch 增加）才重置基准；generation 变化
    只清在线集合与失效在线 API 探测缓存。`start_epoch == 0`（面板运行期间尚未
    启动过子进程）保留基准，兼容仍存活的残留进程，避免把其累计计数当新流量。
- 涉及文件：`src/jobs.rs`、`src/xray/supervisor.rs`；同步修正
  `docs/ARCHITECTURE.md`、`docs/DECISIONS.md` D019、`docs/KNOWN-ISSUES.md` G1、
  `memory/CURRENT-STATUS.md`。
- 验证：`cargo test` 48/48；`cargo clippy --all-targets -- -D warnings` 零警告。
- 备注：`start_epoch` 无独立单测（需要真实子进程）；逻辑为原子计数 + 启动
  路径递增，已在代码中核对。

## 2026-10-31 21:32 — 日志查看器：面板 / Xray 错误 / Xray 访问日志（未打包）
- 状态：已实现；后端与接口冒烟测试通过，前端未做无头浏览器验证
- 需求：一个日志窗口，可切换显示面板日志或 Xray 核心日志；Xray 侧可切换
  error.log 与 access.log。默认显示面板日志。
- 改动：
  1. `src/panel_log.rs`（新）：tracing 环形缓冲（500 行），自定义
     `MakeWriter` 按事件缓冲整行，避免并发写出半行；维护任务收缩内存。
  2. `src/main.rs`：`init_tracing` 返回缓冲并用两个 fmt layer（stdout + 无 ANSI
     的 buffer layer）；`AppState.panel_logs`；`release_memory` 收缩。
  3. `src/api/logs.rs`（新）：`panel_lines`、`xray_payload`、
     `resolve_log_path`、`tail_lines`（约 1 MiB 上限）。`xray_payload` 从
     `xray_extra.log.error/access` 取路径；相对路径按面板 cwd 解析；error
     未配置时回退到 supervisor 捕获的进程输出。
  4. `src/api/compat.rs`：`server_logs` 支持 `source=panel` /
     `source=xray&file=error|access`，无 `source` 保持旧行为；删除本地
     `tail_file_lines`，复用 `api::logs::tail_lines`。
  5. 前端 `frontend-3xui/src/pages/index/LogModal.tsx`：单窗口 + 下拉切换
     面板 / Xray 错误 / Xray 访问；默认面板；移除从未生效的 level/SysLog。
     `LogModal.css` 增加 `.log-hint`、`.log-line-raw`。
  6. i18n：`internal/web/translation/{en-US,zh-CN,zh-TW}.json` 新增 5 个 key；
     其余 locale 由 en-US fallback 覆盖。
  7. 文档：API.md、ARCHITECTURE §9、KNOWN-ISSUES H1–H3。
- 验证：
  - `cargo test` 53/53（新增 panel_log 2 个、api::logs 3 个）。
  - `cargo clippy --all-targets -- -D warnings` 零警告；`cargo build` 成功。
  - 前端 `npm run build` 成功（Node 26.13.0），web/dist 已更新并重新嵌入。
  - 真实进程冒烟测试：临时 DB 启动面板 → `/api/login` → CSRF 后调用
    `/panel/api/server/logs/20`：
    - `source=panel` 返回启动期 tracing 行；
    - 向 DB 写入 `xray_extra.log.error/access` 指向临时文件后，
      error/access 分别返回文件末尾内容，`configured/exists/path` 正确。
- 备注/风险：面板日志仅当前进程内存，重启即清空；access 日志必须先在 Xray
  页面配置 `log.access`；未做浏览器端到端验证（KNOWN-ISSUES H3）。

## 2026-10-31 21:40 — 补齐 v0.2.7 统计测试缺口
- 状态：已完成并验证
- 改动：
  - `src/db/client_traffic.rs`：测试连接启用 `foreign_keys`；新增
    `a_failed_attachment_write_rolls_back_the_whole_tick`（非法 inbound 触发
    FK 失败，断言累计与基准都未部分写入）；`missing_direction...` 增加
    downlink 缺失的对称断言。
  - `src/jobs.rs`：抽出 `direction_deltas`，入站与出站共用；新增
    `missing_or_invalid_directions_keep_their_baselines`（含负数样本）。
  - `src/xray/stats.rs`：新增 `invalidating_the_probe_forces_a_fresh_connection_attempt`
    （UNSUPPORTED 缓存命中 → 失效后必须实际发起连接并失败）。
- 验证：`cargo test` 56/56；`cargo clippy --all-targets -- -D warnings` 零警告；
  `cargo build` 成功。
- 备注：真实 Debian + 真实 Xray 的故障注入仍未做（`TODO.md` T9 / G6）。

## 2026-10-31 21:47 — 面板登录审计日志 + 兼容登录限流
- 状态：已完成并验证（真实进程冒烟测试）
- 背景：用户反馈面板运行错误/登录信息“没地方看”。面板日志标签页此前只捕获
  tracing 输出，但登录成功/失败根本没有打日志；同时发现主 UI 使用的
  compat `/login` 没有速率限制（只有 `/api/login` 有）。
- 改动：
  - `src/api/auth.rs`：
    - `/api/login` 记录限流（WARN）、失败（WARN，含用户名/IP）、成功（INFO）、
      登出（INFO）、改密（INFO）。
    - `is_rate_limited` / `record_failure` / `clear_failures` / `dummy_hash`
      提升为 `pub(crate)` 供 compat 复用。
  - `src/api/compat.rs`：`/login` 与 `/api/login` 共用 per-IP 限流器；未知用户名
    也执行 Argon2 等时校验；记录失败/成功/限流日志。`/logout` 记录用户名。
  - 这些事件同时出现在 panel stdout（journald）和日志弹窗的“面板日志”。
- 验证：
  - `cargo test` 56/56；clippy 零警告；`cargo build` 成功。
  - 真实进程：错误登录→401 且面板日志出现 `panel login failed`；原生/兼容登录
    成功分别出现 `panel login succeeded`；登出出现 `panel logout`。
  - 连续 5 次错误后第 6 次 `/login` 返回 “too many login attempts”，日志出现
    `panel login rate limited`。
- 备注：面板日志仍为内存 500 行 + journald 持久化；未新增独立 panel.log 文件。

## 2026-10-31 21:50 — 打包 0.2.7
- 状态：已完成并验证（打包产物）；未在真实 Linux + 真实 Xray 上运行
- 改动：
  - `Cargo.toml` / `Cargo.lock` 版本 0.2.6 → 0.2.7。
  - 重新构建前端（Node 26.13.0），再执行 musl release 交叉编译。
  - 组装 `dist/xray-panel-rs-linux-amd64-lumen/`（二进制、install.sh、
    service/env 示例、README、SHA256SUMS）并生成 tar.gz。
  - 生成 `dist/xray-panel-rs-0.2.7-src.tar.gz`（含已构建 `web/dist`，排除
    target/tmp/node_modules/发布包）。
- 涉及文件：`Cargo.toml`、`Cargo.lock`、`dist/xray-panel-rs-linux-amd64-lumen*`、
  `dist/xray-panel-rs-0.2.7-src*`、`PROGRESS.md`、本文件、`CURRENT-STATUS.md`、
  `NEXT-STEPS.md`。
- 验证：
  - `cargo test` 56/56；clippy 零警告；前端构建成功；zigbuild 成功。
  - 包内二进制与交叉编译产物 `cmp` 一致（11,652,592 字节）。
  - `file`：ELF 64-bit x86-64，statically linked，stripped。
  - `gzip -t` 通过；二进制内含 `0.2.7`、`panel login succeeded`、`Panel Logs`。
  - 归档 SHA256 `2b2ed838...ada3e2`；二进制 `19463641...c08088`；
    源码包 `0da48c6d...28b64d`。
- 备注：本次打包时 `dist/` 中没有 0.2.6 二进制包，只有 0.2.6 源码包；
  未在真实 Linux/Xray 环境运行，相关验证仍是 `TODO.md` T9。

## 2026-10-31 21:55 — 日志按时间倒序显示（未打包）
- 状态：已实现，前端 lint/build 通过；**未重新打包**
- 改动：`frontend-3xui/src/pages/index/LogModal.tsx` 增加 `visibleLines`，
  显示时反转日志数组（最新在上）；下载仍使用原数组（时间正序）。用模块级
  空数组避免 `useMemo` 依赖每次渲染变化。
- 验证：oxlint 通过；`npm run build` 成功；`cargo test` 56/56（后端未变）。
- 备注：`dist/` 中的 0.2.7 包仍包含旧的“最新在下”前端；需要时重新打包。

## 2026-10-31 22:13 — 打包 0.2.8
- 状态：已完成并验证（打包产物）；未在真实 Linux + 真实 Xray 上运行
- 改动：
  - `Cargo.toml` / `Cargo.lock` 版本 0.2.7 → 0.2.8（包含日志倒序显示）。
  - 归档 0.2.7 包为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.7.tar.gz`。
  - 重新构建前端 + musl release；组装新的发布包与源码包。
- 涉及文件：`Cargo.toml`、`Cargo.lock`、`dist/*0.2.8*`、
  `dist/xray-panel-rs-linux-amd64-lumen*`、`PROGRESS.md`、`memory/*`。
- 验证：
  - `cargo test` 56/56；clippy 零警告；前端构建成功；zigbuild 成功。
  - 包内二进制与交叉编译产物 `cmp` 一致（11,652,576 字节）。
  - `file` 静态 ELF/stripped；`gzip -t` 通过；版本串 `0.2.8`；含 `Panel Logs`。
  - 归档 SHA256 `4403dd16...8f83b`；二进制 `e8b61c65...ebba7`；
    源码包 `f9fb8675...9a134`。
- 备注：仍未在真实 Linux/Xray 上运行（TODO T9）。

## 2026-10-31 23:05 — 科幻主题（未打包，本地预览）
- 状态：已实现，无头 Chrome 截图验证；等待用户浏览器确认后再打包
- 需求：面板颜色偏浅，改成科幻风格，默认展示更深的配色。
- 改动：
  - `src/hooks/useTheme.tsx`：默认深色（`dark-mode-v2` 一次性迁移）；新增
    light/dark/ultra 三套背景、边框、Layout/Menu/Card token；深色 primary
    `#22d3ee`，浅色 `#0891b2`。
  - 新增 `src/styles/sci-fi.css`：深空背景网格、侧栏渐变与青色描边、品牌渐变、
    卡片/表格/按钮/输入框/弹窗/Tabs/滚动条皮肤。
  - `src/styles/page-shell.css`：页面底色与 `--scifi-glow/--scifi-grid`
    变量，三套主题各一组。
  - `src/pages/login/LoginPage.css`：登录页配色/渐变/光斑同步。
  - `src/layouts/AppSidebar.css`：抽屉底色更新。
  - `main.tsx`、`entries/login.tsx`：引入 `sci-fi.css`。
- 验证：
  - `npm run build` 成功；oxlint 通过。
  - 无头 Chrome 截图：登录页（深/浅）、仪表盘（深）、入站页（深/浅），
    存于 `tmp/sci-fi-shot/`。
  - 本地预览 `127.0.0.1:28090`（debug + 临时 DB，admin/admin）。
- 备注：`dist/` 中的 0.2.8 包是在主题改动之前构建的，不含本次改动；用户确认后
  再重新打包。

## 2026-10-31 22:33 — 会话收尾
- 状态：今日工作结束，预览服务已关闭
- 今日完成：
  - v0.2.7 流量统计可靠性修复（per-attachment 基准、重启/重置/缺失统计处理）
  - 日志查看器（面板 / Xray error / Xray access，倒序显示）+ 登录审计 +
    兼容登录入口限流
  - 打包 0.2.7，随后打包 0.2.8（含日志倒序）；0.2.7 已归档
  - 科幻主题（默认深色、深空蓝黑 + 霓虹青 + 网格；浅色冰蓝变体），
    已用无头 Chrome 截图验证并本地预览，用户表示“先这样”
- 当前源码：0.2.8 + **未打包**的科幻主题
- 待办：主题发布打包（需确认 0.2.9 或重切 0.2.8）、T9 真实 Xray 验证、
  T10 日志弹窗浏览器验证
- 验证基线：`cargo test` 56/56、clippy 零警告

## 2026-11-01 09:20 — Xray 连接限制不生效修复 + 入站/出站/路由设置核对
- 触发：用户反馈 Xray 配置里的“连接限制”修改后不生效，要求核对入站/出站/
  路由/Xray 设置模块的操作是否有效（参考 3x-ui）。
- 根因：`/panel/api/xray/update` 只复制固定白名单，`policy`/`metrics` 被静默
  丢弃；`ensure_policy` 强制覆盖统计开关；默认配置接口方法与前端不匹配且返回
  当前配置。入站导入未注册、allLinks 固定空数组、5 个 3x-ui 元数据字段只展示
  不保存、disableFlow 不生效。
- 后端改动：
  - `src/api/compat.rs`：整份保存可编辑模板（忽略 inbounds/api/stats）；
    GET 返回可编辑模板；实现 inbound import/allLinks；元数据解析与视图；
    clientStats 按关联表；错误 JSON 明确报错；保存后清理 outbound_traffic。
  - `src/xray/config.rs`：`DEFAULT_EXTRA_JSON`/`default_extra()`；policy 只强制
    statsUserOnline、缺失统计默认 true、显式值保留；disableFlow 去掉 flow。
  - `src/xray/link.rs`：disableFlow 时分享链接不带 flow。
  - `src/api/backup.rs`：恢复按共有列复制，兼容旧 schema 备份。
  - `migrations/0008_inbound_meta.sql`、`src/db/{models,inbounds}.rs`、
    `src/api/inbounds.rs`、`src/api/xray_settings.rs`、`src/api/mod.rs`、
    `src/service.rs`、`src/main.rs`。
  - 去除原生模板 API 对 `policy` 的保留限制。
- 前端改动：
  - `XrayPage.tsx`：高级编辑器移除 Inbounds 视图；保存传处理后的模板。
  - `useXraySetting.ts`：`saveAll(template?)` 避免 balancer 清理后保存旧字符串。
  - `RouteTester.tsx`/`BalancersTab.tsx`：不支持的操作显示明确错误。
  - `inbound-from-db.ts`：disableFlow 时导出链接去掉 flow；新增测试。
  - `web/dist` 重新构建。
- 验证：
  - `cargo test` **61/61**；clippy 零警告；`cargo fmt --check` 通过。
  - `npm run build` 成功；前端 vitest/typecheck 仍受 devDependencies 缺失阻塞。
  - 全新临时 DB 端到端脚本 8 组全部通过（连接限制/统计/metrics 持久化、
    allLinks、元数据、关联视图、出站/路由、重置默认、导入错误路径）。
  - 真实生成 config.json 含 connIdle/bufferSize/metrics/显式 false 统计开关。
  - 共享客户端二次导入保留原流量并强制启用。
  - 删除 0008 列的旧备份可恢复，新列取默认值。
  - 无头 Chrome 验证连接限制输入框显示 123/64，高级视图无 Inbounds。
- 状态：源码 0.2.8 + 科幻主题 + 本次修复，均**未打包**；未在真实 Linux +
  真实 Xray 上验证 policy/metrics。

## 2026-11-01 09:24 — 打包 0.2.9
- 版本：`Cargo.toml` / `Cargo.lock` 0.2.8 → **0.2.9**。
- 内容：科幻主题 + Xray 设置/入站修复（policy/metrics、连接限制、默认配置、
  入站导入/allLinks/元数据、迁移 0008、旧备份恢复兼容）。
- 构建：
  - `cargo test` 61/61；clippy 零警告；`cargo fmt --check` 通过。
  - 前端 `npm run build` 成功（09:22）。
  - `cargo zigbuild --release --target x86_64-unknown-linux-musl` 成功：
    二进制 11,682,224 字节，静态 ELF、stripped。
- 发布目录/归档：
  - `dist/xray-panel-rs-linux-amd64-lumen/`：二进制、install.sh、
    xray-panel.service、xray-panel.env.example、README.md、SHA256SUMS。
  - `dist/xray-panel-rs-linux-amd64-lumen.tar.gz`：4,597,175 字节，
    SHA256 `a80c70c83ac02e9c2f005dd8fce4cccae057e8f0796ed6106476d1d868cd1caa`。
  - 二进制 SHA256 `850be8d3faf0174cff21869b493b9aa73cf1930c9229686ba515c8a0cd171993`。
  - 0.2.8 归档保留为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.8.tar.gz`。
  - 源码包 `dist/xray-panel-rs-0.2.9-src.tar.gz`：4,573,246 字节，
    SHA256 `b16461bff832d41131482392034c641109c5d502192c6484cd0bd8b7c8ce234e`；
    不含 target/tmp/node_modules/发布包，含 web/dist。
- 验证：`gzip -t` 通过；解包后 `sha256sum -c SHA256SUMS` 通过；`cmp` 包内
  二进制与交叉编译产物一致；`file` 静态 ELF/stripped；二进制含 `0.2.9`、
  `0008_inbound_meta`/`sub_sort_index`、`Panel Logs`、`connectionLimits`。
- 未验证：未在真实 Linux 上运行 musl 二进制；未在真实 Xray 上验证
  policy/metrics 的 `run -test` 与运行行为。

## 2026-11-01 11:58 — 入站/客户端入口精简 + 客户端导入导出修复
- 需求（用户逐条提出）：
  1. 去掉入站菜单「删除所有客户端」。
  2. 去掉入站菜单「将客户端添加到分组」。
  3. 去掉客户端页面「批量添加客户端」。
  4. 修复「导出客户端功能无效」，并验证导入导出正确。
- 前端改动：
  - `pages/inbounds/list/RowActions.tsx`：删除 `addToGroup`、`delAllClients`
    菜单项及 `TagsOutlined` 导入。
  - `pages/inbounds/list/types.ts`、`pages/inbounds/InboundsPage.tsx`：删除对应
    action 类型、确认回调、分支、hydrating key、`groupOpen/groupSource` 状态与
    `AddClientsToGroupModal` 渲染。
  - 删除 `pages/inbounds/clients/AddClientsToGroupModal.tsx`。
  - `pages/clients/ClientsPage.tsx`：删除 `ClientBulkAddModal` 引用、`bulkAddOpen`
    状态、「批量添加」菜单项与弹窗渲染。
  - 删除 `pages/clients/ClientBulkAddModal.tsx` 与其测试
    `test/client-bulk-calendar-renewal.test.tsx`。
  - `internal/web/translation/*.json`（14 个 locale）：删除 `pages.inbounds`
    下 `delAllClients`、`delAllClientsConfirmTitle`、`delAllClientsConfirmContent`、
    `addClientsToGroup`。
- 后端改动：
  - `src/db/clients.rs`：新增 `set_traffic`，用于导入时恢复 up/down/last_online。
  - `src/api/compat.rs`：新增 `clients_export`、`clients_import`、
    `portable_client_record`、`portable_settings`、`portable_inbound_ids`、
    `create_portable_client`；导出保留完整协议凭据与流量，导入恢复凭据、附件、
    启用状态、配额/到期/分组/备注并只对新建客户端恢复流量。
  - `src/api/mod.rs`：`GET /panel/api/clients/export` 从空数组占位改为真实导出；
    注册 `POST /panel/api/clients/import`。
  - 后端兼容接口 `bulkCreate`、`groups/bulkAdd`、`inbounds/{id}/delAllClients`
    保留（见 DECISIONS D022）。
- 文档：`docs/API.md` 客户端接口表更新；`docs/DECISIONS.md` 追加 D022、D023。
- 验证（Windows debug + fake xray，临时 DB）：
  - `cargo test` **61/61**；`cargo clippy --all-targets -- -D warnings` 零警告；
    `cargo fmt` 已执行。
  - 前端 `npm run build` 成功；`tsc --noEmit` 改动文件无新增错误
    （Storybook/devDependencies 既有报错仍在）。
  - 端到端脚本 `tmp/e2e-clients-io/e2e.py` 全部通过：登录、创建 2 个入站、
    创建启用/禁用 2 个客户端（多入站关联、flow、uuid、password、group、comment、
    limitIp/limitHwid、配额/到期）、写入流量、导出字段校验、重复导入全部跳过、
    删除、坏 JSON/非数组/空 inboundIds/空邮箱的错误处理、导入 created=2、
    二次导出逐字段对比凭据/附件/流量/禁用状态一致。
- 状态：未打包、未 bump 版本；前端 `web/dist` 已重建。
- 未验证：真实 Linux + 真实 Xray 上的导入后运行行为；无头浏览器点击导入/导出
  弹窗；3x-ui “无入站客户端” 格式不支持（明确跳过）。

## 2026-11-01 12:03 — 移除菜单栏搜索 + 打包 0.2.10
- 用户追加要求：去除菜单栏的搜索功能（侧边栏 Command Palette / Ctrl+K）。
- 前端改动：
  - `layouts/PanelLayout.tsx`：不再挂载 `CommandPalette`。
  - `layouts/AppSidebar.tsx`：删除桌面侧栏与移动抽屉的搜索按钮、
    `useCommandPalette`、`SearchOutlined`、`Tooltip`、`SHORTCUT_MODIFIER`。
  - 删除 `src/components/command-palette/`（组件、CSS、store）与
    `test/command-palette.test.tsx`；`navigation-contract.test.tsx` 移除
    `PALETTE_KEYWORDS` 导入及对应测试。
  - 14 个 locale 删除顶层 `commandPalette.*` 文案。
  - 保留入站列表与客户端页内的筛选搜索框（见 DECISIONS D024）。
- 打包：
  - `Cargo.toml` / `Cargo.lock` 0.2.9 → **0.2.10**。
  - 前端 `npm run build` 成功（12:03）；musl release 构建成功。
  - 发布目录：`dist/xray-panel-rs-linux-amd64-lumen/`（二进制 11,635,888 字节）。
  - 归档：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（4,589,521 字节，
    SHA256 `543dec0039915a8eda53b5037d2a019757880ffb3a96234902e37e5b91e7d268`）。
  - 二进制 SHA256 `d5036a05dc10cd7c4fc4f8898a0ffd65f359e5c2632c5e90e8336e761f03fb62`。
  - 0.2.9 归档保留为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.9.tar.gz`。
- 验证：`cargo test` 61/61、clippy 零警告、fmt 通过；`gzip -t`、
  `sha256sum -c`、`cmp`、`file` 均通过；二进制内含 `0.2.10`。
- 未验证：真实 Linux + 真实 Xray 运行；无头浏览器点击。

## 2026-11-01 12:17 — 登录失败微信通知 + 打包 0.2.11
- 需求：密码错误（账户/密码错误）时调用微信 webhook，显示真实 IP 和尝试账户；
  成功登录不发送；webhook 地址在面板设置配置；测试后打包。
- 后端：
  - 新增 `src/notify.rs`：`real_ip()` 解析 `X-Real-IP` → `CF-Connecting-IP` →
    `X-Forwarded-For` 首项 → TCP 对端；`spawn_failed_login()` 异步发送企业微信
    群机器人文本消息；账户名去控制字符并截断 100 字符；10 秒超时；无配置时跳过。
  - `src/api/auth.rs`、`src/api/compat.rs`：两个登录入口在校验失败后调用通知；
    成功、空凭据、限流不发送。
  - `src/api/compat.rs`：`setting_all` 默认 `loginNotifyWebhook`；
    `setting_update` 校验 URL 必须 http/https。
  - 依赖：`ureq` 3 + `rustls`（静态 musl）；Windows 开发构建需 MinGW gcc
    （`libsqlite3-sys`/`ring` 的 C 编译），已写入 `CLAUDE.md`。
- 前端：`models/setting.ts`、`schemas/setting.ts`、`pages/settings/GeneralTab.tsx`
  增加 webhook 输入框；14 个 locale 增加 `pages.settings.loginNotifyWebhook*`。
- 文档：`docs/ARCHITECTURE.md` 模块表与认证段、`docs/API.md` 登录说明、
  `docs/DECISIONS.md` D025、`docs/KNOWN-ISSUES.md` L1/L2、`TODO.md` T15。
- 验证：
  - `cargo test` **63/63**（新增 2 个单测）；clippy 零警告；fmt 通过。
  - 前端构建成功；改动文件 tsc 无新增错误。
  - `tmp/webhook-e2e/e2e.py`：原生失败（X-Real-IP）、兼容失败
    （X-Forwarded-For 首项）、成功不通知、空凭据不通知、设置读写、非法 URL
    拒绝、清空关闭，全部通过。
- 打包：0.2.10 → **0.2.11**；归档 5,217,711 字节，
  SHA256 `3bae78cd49e64f8f6810f74890398260126ec91c7655f3ab2536f829e9e0e755`；
  二进制 12,782,992 字节，SHA256
  `618caef970ad9435f1d7bfe2b2e181f72d2d381246c6150696ffc1df57f8b655`。
- 未验证：真实企业微信 HTTPS/消息格式；真实 Linux 运行。

## 2026-11-01 23:03 — 出站订阅实现（出站菜单精简 + 代理更新）

- 用户反馈“出站订阅功能无效，请完善并测试”；追加：更新订阅可通过代理；
  出站页“更多”只保留导入/导出。
- 后端：
  - 迁移 `0009_outbound_subscriptions.sql`；`db::outbound_subs` 与模型
    `OutboundSubscription`/`OutboundSubscriptionView`。
  - `src/xray/subscription.rs`：订阅正文解析（base64/纯文本）、
    vmess/vless/trojan/ss/hysteria2/wireguard 链接→Xray 出站、稳定标签分配。
  - `src/outbound_subs.rs`：CRUD、SSRF 校验、ureq 拉取（代理/TLS/大小/超时）、
    30 秒到期刷新 job、运行时 `prepend`/`append` 合并、apply 失败回滚。
  - `src/api/outbound_subs.rs` 与 `/panel/api/xray/outbound-subs...` 路由；
    `GET /panel/api/xray/` 增加订阅出站/标签；Xray 模板保存保留订阅流量行与
    路由标签；备份/恢复加入新表并处理旧备份。
  - 依赖：`url`、`percent-encoding`；ureq 增加 `socks-proxy` feature。
- 前端：
  - `OutboundsTab.tsx`：新增 `proxyUrl` 字段与“通过代理更新”输入、预览带代理；
    “更多”仅保留导入/导出，移除 WARP/NordVPN/PIA。
  - `XrayPage.tsx`：移除三个 override 弹窗挂载、状态与 `propagateOutboundTagRename`。
  - i18n：en-US/zh-CN/zh-TW 增加 `pages.xray.outboundSub.proxyUrl*`。
  - 两个既有测试移除废弃的 `onShow*` props。
- 测试：
  - `cargo test` **76/76**；clippy 零警告；fmt 通过；debug 构建成功。
  - `npm run build` 成功；改动文件 tsc 无新增错误；oxlint 通过。
  - `tmp/e2e-outbound-subs/e2e.py`：直连/预览/稳定标签/HTTP 代理 CONNECT、
    SOCKS5h、非法代理、SSRF 拒绝、启停/排序/删除、运行时配置合并、后台 job
    自动刷新，全部通过（本地 mock 订阅/代理服务器 + fake-xray）。
  - `tmp/e2e-outbound-subs/ui-run.py`（无头 Chrome）：更多菜单仅 2 项、无
    WARP/NordVPN/PIA；订阅弹窗有代理输入；控制台无报错。
- 未验证：真实 Linux + 真实 Xray 的 `run -test` 拒绝/回滚、真实公网代理与认证、
  重定向逐跳 SSRF、JSON/Clash 订阅正文。
- 未打包、未 bump 版本（仍 0.2.11）。

## 2026-11-01 23:12 — 打包 0.2.12（出站订阅）

- 版本 0.2.11 → **0.2.12**（`Cargo.toml` / `Cargo.lock`）。
- 构建：前端 `npm run build` 成功；`cargo test` 76/76；clippy 零警告；
  `cargo fmt --check` 通过；`cargo zigbuild --release --target
  x86_64-unknown-linux-musl` 成功（49s）。
- 发布目录：`dist/xray-panel-rs-linux-amd64-lumen/`（二进制 13,120,464 字节，
  静态 ELF、已 strip；含 install.sh/service/env/README/SHA256SUMS）。
- 归档：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（5,365,187 字节，
  SHA256 `3eddd23b0747e0bddfe1fe0df170ead2412dd582c2ac611aa876dbff27ccf949`）；
  二进制 SHA256 `8fc5c32fcdab08448c486e758e159e6709d34a12e87c7c6c2e1493d63fe06e78`。
- 0.2.11 归档保留为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.11.tar.gz`。
- 源码包：`dist/xray-panel-rs-0.2.12-src.tar.gz`（4,570,602 字节，SHA256
  `b734da61bae9792e4b8a8a1c130e83c904937e29f87683defa07f7ec68532332`；
  排除 target/tmp/node_modules/web/dist-3xui/web/dist-custom-v2）。
- 验证：`gzip -t`、解包 `sha256sum -c SHA256SUMS`、`cmp` 与交叉编译产物一致、
  `file` 静态 ELF/stripped；二进制内含 `0.2.12`、`outbound-subs`、
  `outbound_subscriptions`、`pages.xray.outboundSub.proxyUrl`。
- 清理：删除 `target/debug/incremental`（9.4G），target 17G → 8.9G；
  release/musl 产物保留，打包速度不受影响。
- 未验证：真实 Linux 运行该 musl 二进制；真实 Xray 的订阅拉取/注入/回滚。

## 2026-11-02 00:35 — 功能整改（真实 Xray 验证）

- 用户逐项要求：去除路由测试/目标扫描/系统历史/面板更新/客户端 IP 日志/
  Happ 与订阅 UI/批量调整多余字段/不支持入站协议与 WireGuard；实现负载均衡、
  ECH、ML-DSA-65、ML-KEM、证书哈希、Xray 版本/Geofiles/GEOdata 更新与更新代理。
- 后端：
  - 新增 `src/xray/router.rs`、`proto/router.proto`：RoutingService
    GetBalancerInfo/OverrideBalancerTarget；`api.services` 加入 RoutingService。
  - 新增 `src/updates.rs`：Release 列表、Xray zip 安装（`.dgst` 校验）、
    Geofiles 下载（`.sha256sum` 校验）、`updateProxy`、`geodataSources`。
  - 新实现 `getNewmldsa65`、`getNewmlkem768`、`getNewEchCert`、`getCertHash`；
    证书文件仅允许入站/面板已引用路径。
  - 入站协议白名单收紧为 vmess/vless/trojan/shadowsocks/http/mixed（native 与
    compat API 均校验）；移除 2FA 查询、nodes、HWID/IP/Happ/externalLinks/
    xraylogs/metricsState/xrayResult/面板更新等空或假成功路由与处理函数；
    `setting` 默认值不再返回订阅/Happ/access-log 字段。
- 前端：
  - 删除 RouteTester、目标扫描与设置、SystemHistory、PanelUpdate、XrayLog、
    AWGLog、XrayMetrics、SubLinks、ClientRenewalFields、HWID/IP 日志组件；
    删除 Hosts/Nodes/Groups/ApiDocs/Sub/Rawconfig/Overrides 页面和测试；
    删除订阅/Happ/Telegram/Incy 设置页与对应测试。
  - 入站协议选项只保留六项；客户端 QR 只保留直接分享链接；批量调整只剩
    加天数/流量；设置页运行环境新增更新代理输入；登录页移除 2FA 分支。
  - 构建脚本去掉 OpenAPI 生成（API Docs 页面已删）。
- 验证：
  - `cargo test` 83/83；clippy 零警告；fmt 通过；前端构建成功；应用代码 tsc
    无报错；改动文件 oxlint 通过。
  - 真实 Xray 26.10.31（Windows）`tmp/e2e-outbound-subs/real-e2e.py`：全部通过
    （密钥生成、证书哈希、balancer 状态/override、版本列表/安装、geoip.dat 更新）。
  - `geodata` 配置经真实 `xray run -test`：Configuration OK。
  - 订阅回归 `e2e.py`、更新/代理/协议过滤 `updates-e2e.py` 全通过。
  - 无头 Chrome：路由页无测试标签；仪表盘无更新/历史；设置页有更新代理；
    入站协议选项六项；出站菜单/订阅代理字段保持。
- 未验证：真实 Linux 的安装路径/权限与 `XRAY_LOCATION_ASSET`；GEOdata 自动下载；
  更新代理认证；override 跨重启持久化（设计上不持久化）。
- 未打包、未 bump 版本（仍 0.2.12）。

## 2026-11-02 00:42 — 打包 0.2.13（功能整改）

- 版本 0.2.12 → **0.2.13**（`Cargo.toml` / `Cargo.lock`）。
- 内容：2026-11-02 功能整改（移除路由测试/目标扫描/系统历史/面板更新/客户端
  IP 日志/Happ 与订阅 UI/不支持协议；负载均衡状态/override、ECH/ML-DSA-65/
  ML-KEM、证书哈希、Xray 版本/Geofiles/GEOdata/更新代理）+ 0.2.12 出站订阅。
- 构建：`cargo fmt --check` 通过；clippy 零警告；`cargo test` 83/83；
  前端 `npm run build` 成功（web/dist 00:38）；`cargo zigbuild --release
  --target x86_64-unknown-linux-musl` 成功（49.17s）。
- 发布目录：`dist/xray-panel-rs-linux-amd64-lumen/`（二进制 12,623,520 字节，
  静态 ELF、已 strip；含 install.sh/service/env/README/SHA256SUMS）。
- 归档：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（5,354,421 字节，
  SHA256 `11ab05a6eb3f6a220890109ecf0445e1a45cc4518c36edf7e709c9c8c4a377ac`）；
  二进制 SHA256 `d407c2499653a0acc509abee4021a61e8bd748c55218525c67155c99725db9c6`。
- 0.2.12 归档保留为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.12.tar.gz`
  （5,365,187 字节，SHA256 `3eddd23b...cff949`）。
- 源码包：`dist/xray-panel-rs-0.2.13-src.tar.gz`（4,183,211 字节，SHA256
  `086ff06bc4ac817edc58584aca110e1ed05b976b81d82c1dbdd7b24f526addd9`；
  排除 target/tmp/node_modules/web/dist-3xui/web/dist-custom-v2）。
- 验证：`gzip -t`、解包 `sha256sum -c SHA256SUMS`、`cmp` 与交叉编译产物一致、
  `file` 静态 ELF/stripped；二进制内含 `0.2.13`、`outbound-subs`、
  `outbound_subscriptions`、`geodataSources`、`updateProxy`、`RoutingService`、
  `mldsa65`、`mlkem768`。
- 未验证：真实 Linux 运行该 musl 二进制；真实 Linux 的安装/权限与
  `XRAY_LOCATION_ASSET`、GEOdata 自动下载、更新代理认证。

## 2026-11-02 — 修复 Xray 安装时代理随 Xray 停止而失效

- 现象：面板更新代理使用托管 Xray 的本地入站时，版本列表正常，但安装 zip
  报 `Connection refused (os error 111)`。
- 根因：`install_xray` 先 `state.xray.stop()` 再下载；下载代理就是该 Xray
  的入站，进程一停端口即关闭。
- 修复：下载、`.dgst` 校验、解压到临时文件都在 Xray 运行时完成；下载失败
  不再中断 Xray；仅在原子替换前停止，替换失败重启旧版本。
- 文件：`src/updates.rs`（`install_xray`、`extract_xray_temp`、`replace_xray`
  及单测）。
- 验证：`cargo fmt --check`、clippy 零警告、`cargo test` 83/83。
- 未打包；0.2.13 发布包不含此修复；未在真实环境端到端验证。

## 2026-11-02 — 修复 Geofiles 写入只读文件系统

- 现象：更新 `geosite.dat` 写入 `/usr/local/bin/.geosite.dat.*.tmp` 时报
  `Read-only file system (os error 30)`。
- 根因：Geofiles 默认写入 `XRAY_BIN` 目录；systemd 单元
  `ProtectSystem=full` 且 `ReadWritePaths` 未包含 `/usr/local/bin`。
- 修复：`deploy/xray-panel.service` 的 `ReadWritePaths` 加入
  `/usr/local/bin`；README 单元示例和 `docs/KNOWN-ISSUES.md` B8 同步。
- 已部署 0.2.13 的临时处理：编辑单元追加该路径后 daemon-reload/restart，
  或设置可写的 `XRAY_LOCATION_ASSET`。
- 未打包、未在真实 systemd 环境验证。

## 2026-11-02 01:11 — 打包 0.2.14（Xray 安装与 systemd 写入修复）

- 版本 0.2.13 → **0.2.14**（`Cargo.toml` / `Cargo.lock`）。
- 内容：0.2.13 全部功能 + Xray 安装先下载/校验/解压再停止 Xray（§2.16）+
  systemd `ReadWritePaths` 加入 `/usr/local/bin`（§2.17）。
- 构建：`cargo fmt --check`、clippy 零警告、`cargo test` 83/83；musl release
  成功（49.20s）；前端无改动，沿用 00:38 的 `web/dist`。
- 发布目录：`dist/xray-panel-rs-linux-amd64-lumen/`（二进制 12,623,456 字节，
  静态 ELF、已 strip；service 含 `/usr/local/bin` 可写路径）。
- 归档：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（5,354,579 字节，
  SHA256 `3efcf8286540415e6baa189178618591dafbc6959aeb25d4c1edbc987ae06068`）；
  二进制 SHA256 `1ab4559fa9c59f83247172512325eb5fa0fa0d1f829837b4d5c09a8777334bd2`。
- 0.2.13 归档保留为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.13.tar.gz`
  （5,354,421 字节，SHA256 `11ab05a6...a377ac`）。
- 源码包：`dist/xray-panel-rs-0.2.14-src.tar.gz`（4,185,587 字节，SHA256
  `00e3713074afc14650aa06eaf716a39e4db857dca124765ffbe8bc26607f2624`）。
- 验证：`gzip -t`、`sha256sum -c`、`cmp`、`file`（静态 ELF/stripped）；
  二进制含 `0.2.14` 及既有功能标记。
- 未验证：真实 Linux 运行 musl 二进制；真实 systemd 下 Geofiles/Xray 安装。

## 2026-11-03 22:45 — 补全 Xray 原生入站/出站协议

- 用户要求：参考 3x-ui 补全入站/出站所有协议，每个协议都能成功添加，不要只加前端。
  经确认本轮只做 Xray 原生协议；sidecar 协议（MTProto/TUIC/AmneziaWG）不列入。
- 前端：
  - `frontend-3xui/src/pages/inbounds/form/InboundFormModal.tsx`：入站下拉扩到
    11 种原生协议；补 WireGuard key 展示/重生成、Hysteria/MASQUE 传输、
    TUN/Tunnel 字段、TLS-only 安全页签、隧道不显示 FinalMask。
  - 新增/复制 `protocols/{wireguard,hysteria,masque,tun,tunnel}.tsx` 与
    `protocols/index.ts` 导出。
  - `pages/xray/outbounds/outbound-form-constants.ts`：出站下拉隐藏 amneziawg，
    保留 13 种 Xray 原生协议。
- 后端：
  - `src/xray/config.rs`：新增 `XRAY_INBOUND_PROTOCOLS`/`is_xray_inbound`；
    `build_inbound` 支持 wireguard（clients→peers）、hysteria（version 2 +
    clients）、masque（pass + 原始 email）、tunnel/tun 透传；非原生协议跳过；
    修复 Shadowsocks 2022 无客户端时丢失单用户 password。
  - `src/api/compat.rs`：入站白名单改为原生协议；`managed_protocol` 扩到
    wireguard/hysteria/masque；同步/注入客户端；客户端建号生成 WireGuard
    keypair + allowedIPs、Hysteria auth、MASQUE password；客户端记录回显
    auth/keys/allowedIPs。
  - `src/api/inbounds.rs`：原生 API 白名单同步。
  - `Cargo.toml`：新增 `x25519-dalek`（static_secrets）。
- 测试：
  - `cargo test` 89/89（新增 wireguard peers、hysteria v2、masque pass/email、
    tunnel/tun 透传、原生协议白名单、WireGuard 凭证分配 6 个测试）。
  - `cargo clippy --all-targets -- -D warnings` 零警告；`cargo fmt --check` 通过。
  - 前端 `npm run build` 成功；改动文件 oxlint 通过；typecheck 仅既有缺依赖报错。
  - fake xray + 临时 DB 端到端（`tmp/e2e-protocols/run.mjs`）：11 种入站添加成功、
    4 种非原生协议拒绝、3 种客户端凭证生成正确。
  - 无头 Chrome：入站下拉 11 项/表单渲染通过；9 种无证书协议真实保存成功；
    出站下拉 13 项且无 amneziawg；控制台无报错。
- 未验证：官方 Xray `run -test` 与真实连接/流量（本机无 Xray 二进制，下载执行
  需用户授权）；hysteria/masque 需有效 TLS 证书。见 `KNOWN-ISSUES.md` M1。
- 未打包、未 bump 版本；按用户要求等确认后再打包。

## 2026-11-03 23:15 — 面板设置“节点地址” + 分享链接/二维码主机

- 用户要求：设置里配置节点域名/IP，分享链接和二维码使用它，不要用浏览器地址
  （可能是局域网地址）。
- 后端：
  - `src/api/compat.rs`：`setting/all` 增加 `nodeAddress`（缺失时用旧
    `public_address` 回填）；`setting/update` 校验并保存 `nodeAddress`；
    `setting/defaultSettings` 返回 `nodeAddress` 并镜像到 `webDomain`；
    `factoryDefaults` 增加 `nodeAddress`。
  - `src/api/inbounds.rs`：`public_address()` 优先读取 `x3ui_settings.nodeAddress`
    （回退 `webDomain`），再回退旧 `public_address` 设置与 Host 头。
  - `sync_clients_from_settings` 保留/生成客户端 `subId`，修复入站导入或
    携带 clients 创建时二维码无链接。
- 前端：
  - `GeneralTab.tsx` 增加“节点地址”输入；`models/setting.ts`、
    `schemas/setting.ts`、`schemas/defaults.ts` 增加 `nodeAddress`。
  - `useInbounds.ts`/`useClients.ts` 优先取 `nodeAddress`；`preferPublicHost`
    改为配置值始终优先；文案加入 14 个 locale。
- 验证：
  - `cargo test` 89/89、clippy 零警告、fmt 通过；前端 `npm run build` 成功，
    改动文件 oxlint 无新增问题。
  - fake xray + 临时 DB API 端到端（`tmp/e2e-nodeaddr/run.mjs`）17/17：
    保存/读取、非法值拒绝、`subLinks`/`allLinks` 均用 `node.example.com` 而非
    Host `192.168.1.50`；IP `203.0.113.10` 同样生效。
  - 无头 Chrome（`tmp/e2e-nodeaddr/ui.mjs`）：设置页显示节点地址；入站导出
    链接、客户端二维码链接均使用配置地址，不含 127.0.0.1；控制台无报错。
- 未验证：真实客户端用生成的链接连接；真实反代 Host 场景。
- 未打包、未 bump 版本。

## 2026-11-03 22:45 — 打包 0.2.15（协议补全 + 节点地址设置）

- 版本 0.2.14 → **0.2.15**（`Cargo.toml` / `Cargo.lock`）。
- 内容：0.2.14 全部功能 + 11 种 Xray 原生入站/13 种原生出站（§2.19）+
  面板设置节点地址与分享链接/二维码主机（§2.20）。
- 构建：前端 `npm run build`（22:40）；`cargo fmt --check`、`cargo test`
  89/89、clippy 零警告；musl release 交叉编译 58.64s。
- 发布目录：`dist/xray-panel-rs-linux-amd64-lumen/`（二进制 12,691,200 字节，
  静态 ELF、已 strip；README/install.sh/service/env 示例/SHA256SUMS）。
- 归档：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz`（5,398,823 字节，
  SHA256 `40b4b2458c48fee4f8818c02e8c5f1e2e8e41a4b2e301542fca9339889afb13a`）；
  二进制 SHA256 `105a28a94e358ab2488827e13ca689b5a161fee2d3f2a865dbf34ce7070b06f2`。
- 0.2.14 归档改名为 `dist/xray-panel-rs-linux-amd64-lumen-0.2.14.tar.gz`
  （5,354,579 字节，SHA256 `3efcf828...06068`）。
- 源码包：`dist/xray-panel-rs-0.2.15-src.tar.gz`（4,207,649 字节，SHA256
  `d6e7c21eb3efb690ad62e4be280adaff451b615795626ce52599832d9545cac3`）。
- 验证：`gzip -t`、包内 `sha256sum -c`、`cmp`、`file` 静态 ELF/stripped；
  二进制内含 `0.2.15`、`nodeAddress`、`节点地址`、`wireguard`。
- 未验证：真实 Linux 运行 musl 二进制；官方 Xray `-test`（T21）；真实客户端连接。

## 2026-11-03 22:50 — GitHub 发布、一键安装与面板在线更新

- 新增 `src/panel_update.rs`：查询 `PANEL_GITHUB_REPO` 的 latest release，下载
  `xray-panel-rs` + `SHA256SUMS`，校验后原子替换当前可执行文件；systemd 下
  退出由 `Restart=on-failure` 拉起，非 systemd 返回 `restartRequired`。
  新增接口 `GET /panel/api/server/panelUpdateCheck`、
  `POST /panel/api/server/panelUpdate`。
- 设置页新增「面板更新」区块（检查更新/立即更新/当前与最新版本/Release 链接），
  14 个 locale 补齐文案。
- 新增根目录 `install.sh`：下载最新 Release、校验 SHA256、可选安装官方 Xray，
  再调用 `deploy/install.sh`；后者现在会先停止运行中的 xray-panel 服务再替换
  二进制，避免 Text file busy。
- 新增 `.github/workflows/ci.yml`（main/PR：前端构建 + fmt/test/clippy）与
  `release.yml`（`v*` 标签：Node 26 构建前端、cargo-zigbuild musl、打包
  tar.gz/原始二进制/SHA256SUMS、自动创建 Release）。编译时注入
  `PANEL_GITHUB_REPO=${{ github.repository }}`。
- 仓库初始化为 `ztoofar/xray-panel-rs`（public），main 已推送。
- 版本 0.2.15 → 0.2.16；`cargo test` 90/90、clippy 零警告、fmt 通过；
  musl release 构建成功（49.54s）。
- 本地打包：`dist/xray-panel-rs-linux-amd64-lumen.tar.gz` 5,411,231 字节
  SHA256 `a39f2923...94a3`；二进制 12,728,480 字节 SHA256 `ea183fb1...049f`；
  源码包 4,224,342 字节 SHA256 `2091acf5...7628`。0.2.15 归档保留。
- 未验证：真实 Linux systemd 下的在线更新全流程（N2）；GitHub Actions Release
  产物以下载页为准。

## 2026-11-03 23:15 — GitHub Release v0.2.16 发布完成

- 仓库 main + tag `v0.2.16`（c4c669a）推送完成。
- GitHub Actions：`ci`（fmt/test 90/90/clippy）通过；`release` 通过（Node 26 +
  cargo-zigbuild 0.24.0 + Zig，musl 构建、打包、softprops 发布）。
- Release <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.16>：
  - tar.gz 5,417,245 字节，SHA256
    `ad58e7dae135a0fdfa689a2af5935ef3989d2c28a58e007284abcad000604b3c`
  - 原始二进制 12,742,128 字节，SHA256
    `f1760246078b995118b608085c21b0f65d0d218fc472cad2b50fef228209ee83`
  - `SHA256SUMS` 内容 `f176...ee83  xray-panel-rs`
- 验证：本地 0.2.16 面板 `panelUpdateCheck` 成功返回 current/latest 0.2.16、
  `updateAvailable:false`、repo 正确；Windows 更新接口返回 Linux-only 明确错误；
  设置页 UI 显示「面板更新/检查更新/节点地址」且无控制台报错。
- 未验证：真实 Linux + systemd 在线更新（T23/N2）；一键安装在全新 Linux 执行。

## 2026-11-03 23:40 — Release 资产精简为仅 tar.gz，在线更新改为解压

- 用户要求 Release 不再单独放二进制（压缩包已包含），也不再放 SHA256SUMS。
- 后端 `src/panel_update.rs`：
  - latest release 只查找 `xray-panel-rs-linux-amd64-lumen.tar.gz`；
  - 使用 GitHub 资产 `digest`（`sha256:<hex>`）校验下载的 tar.gz；
  - 内置最小 tar.gz 解析器（`flate2` GzDecoder + 512 字节 tar header）提取
    `xray-panel-rs-linux-amd64-lumen/xray-panel-rs`，校验 ELF 头与大小上限；
  - 新增 3 个解析测试（正常提取/缺文件/非 ELF）。
- `.github/workflows/release.yml`：只上传 tar.gz；包内 SHA256SUMS 保留。
- `Cargo.toml` 增加直接依赖 `flate2`（原本通过 zip 间接存在）。
- 版本 0.2.16 → 0.2.17；`cargo test` 93/93、clippy 零警告、fmt 通过；
  musl release 44.83s。
- 兼容性：v0.2.16 更新器查找原始二进制资产，无法自动升级到 0.2.17；
  0.2.16 用户需用一键安装脚本升级一次（KNOWN-ISSUES N3）。

## 2026-11-03 23:50 — v0.2.17 发布（Release 仅 tar.gz）

- tag `v0.2.17`（c5bdaa3）推送；Actions `release` 成功。
- Release <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.17>
  唯一资产：`xray-panel-rs-linux-amd64-lumen.tar.gz` 5,422,123 字节，
  GitHub asset digest `sha256:23a4885d7d87a2354f0c4e979eb2bb0644454e1cc47c5c61736f3825d08318a9`。
- 不再上传原始二进制和 SHA256SUMS；包内仍有 SHA256SUMS。
- 验证：本地 0.2.17 debug `panelUpdateCheck` 返回 current/latest 0.2.17、
  `updateAvailable:false`；Release 资产列表只有 tar.gz。
- 兼容性：0.2.16 更新器仍查找旧二进制资产，需一键安装脚本升级一次
  （KNOWN-ISSUES N3）；Release 说明已加入该提示。

## 2026-11-04 00:05 — v0.2.18：安装/更新代理与 systemd 重启判定

- `install.sh`：检测已有安装并走更新模式；新增 `PANEL_PROXY`（HTTP/SOCKS，
  回退 HTTPS_PROXY/ALL_PROXY）；下载失败报错包含代理地址；兼容安装包内
  `install.sh` 无执行位的情况（`bash install.sh`）。
- 修复 Release 包内 `install.sh` 权限：git mode 改为 100755，release workflow
  显式 `chmod +x`。
- 面板在线更新复用 `updateProxy`，check/install 错误链加入
  `via update proxy <addr>`；服务单元新增 `PANEL_SYSTEMD=1`，旧单元用
  `INVOCATION_ID` + `systemctl MainPID` 兼容判断，修复 GitHub Actions 中
  误判 systemd 后 `exit(1)` 的问题。
- 新增 `.github/workflows/integration.yml`，手动触发：
  - online-update：v0.2.17 测试二进制真实更新到 v0.2.18；
  - installer：v0.2.18 一键安装（systemctl stub）与坏代理报错。
- 结果：integration 两个 job 均通过；`cargo test` 93/93、clippy 零警告。
- 发布 v0.2.18：唯一资产 tar.gz 5,425,000 字节，digest
  `sha256:ba59a2a2bf29e5b9f0f5a5d9cfb21df2ba83a6ba68bacd868bc827646f688496`。
- 未验证：真实 systemd 服务下的自动重启（CI 使用 stub）；TODO T23。

## 2026-11-04 00:15 — 安装幂等跳过、更新报错提示与 README 重写

- 用户反馈：已安装最新版后重跑一键脚本仍会重新安装；面板在线更新经
  `updateProxy=http://127.0.0.1:10813` 报 `Connection refused (os error 111)`，
  希望报错更可操作；同时要求 README 精简、正式化。
- `install.sh`：
  - 新增 `PANEL_FORCE=1`；解析 GitHub `releases/latest` 的 `Location` 得到目标
    版本（走 `PANEL_PROXY`）。
  - 新增 `installed_version()`：先 `grep -a LUMEN_VERSION=` 再执行
    `--version`，避免旧二进制误启动；版本一致直接退出，不下载/不停服务。
  - 无 marker 的旧二进制下载并校验后按 SHA-256 比对，一致时跳过。
  - 跳过面板重装时仍补装缺失的 Xray 核心。
- `src/main.rs`：`--version`/`-V` 在初始化前输出版本与 `LUMEN_VERSION` marker；
  新增 `is_version_arg` 单测。
- `src/updates.rs`：`network_hint` / `with_network_hint` 按类型识别传输错误
  （本地化文本兜底），接入 `xray_versions`、`install_xray`、Geofiles。
- `src/panel_update.rs`：check/install 错误前加代理可操作提示。
- `README.md`：全面重写，精简结构并修正过期描述（轮询 5 秒/2–300、
  `limitIp` 为近似执行、`/api/health` 路径等）。
- `release.yml`：发布说明补充跳过与 `PANEL_FORCE`。
- 验证：`cargo fmt --check`、`cargo test` 97/97、clippy 零警告、`cargo build`
  成功；`--version`/`-V` 实测；`tmp/install-skip-test/run.sh` 10 项断言全通过；
  `tmp/e2e-update-hint/e2e.py` 全通过（真实本机连接拒绝、fake xray + 临时 DB）。
- 文档：新增 `DECISIONS.md` D035/D036、`KNOWN-ISSUES.md` N5/N6；
  `CURRENT-STATUS.md` §2.26。
- 未验证：真实 Linux/musl、真实 systemd 重跑；代理可达但目标不可达的归因。
- 未打包、未 bump 版本（仍 0.2.18）。

## 2026-11-04 00:21 — 0.2.19 发布

- `Cargo.toml` / `Cargo.lock` 版本 0.2.18 → `0.2.19`。
- 内容：安装幂等跳过（版本 marker 或包内 SHA-256）、`PANEL_FORCE=1`、
  二进制 `--version`/`LUMEN_VERSION`、更新/下载代理失败提示、README 重写。
- 发布前验证：`cargo fmt --check`、`cargo test` 97/97、clippy 零警告、
  `cargo build` 成功；`tmp/install-skip-test/run.sh` 10/10；
  `tmp/e2e-update-hint/e2e.py` 全部通过。
- 发布方式：推送 `main`（提交 `bd2faf7`）与 tag `v0.2.19`，由 GitHub Actions
  `release.yml` run `41673502413` 构建 musl 包并创建 Release；本地不打包。
- Release <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.19>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` 5,424,758 字节，
  GitHub asset digest
  `sha256:ab81a7c79507211afe8798576bbb057ba8c1015461700db4c91847cbf18435c6`。
- 0.2.19 起安装脚本可通过 `LUMEN_VERSION` marker 零下载跳过重装；0.2.18 及更早
  仍走下载+哈希路径。

## 2026-11-04 00:36 — 代理/网络错误中文化并发布 0.2.20

- `src/updates.rs`：`network_hint` 输出中文消息（“代理不可用”“无法连接
  GitHub”）；`with_network_hint` 用中文消息替换错误链，原生链仅
  `tracing::warn`。覆盖面板更新、Xray 版本/安装、Geofiles。
- `src/outbound_subs.rs`：订阅拉取传输失败映射为中文；代理校验与 HTTP/读取
  错误文案中文化；新增 `subscription_proxy_errors_are_chinese` 单测。
- `src/api/compat.rs`：更新/下载类接口失败前缀与 `updateProxy` 校验错误改为中文。
- 验证：`cargo fmt --check`、`cargo test` 99/99、clippy 零警告、`cargo build`
  成功；`tmp/e2e-update-hint/e2e.py` 全通过（中文消息、无 `api.github.com`
  与 `os error`）。
- 文档：新增 `DECISIONS.md` D037、`KNOWN-ISSUES.md` N7；
  `CURRENT-STATUS.md` §2.28–§2.29。
- 版本 0.2.19 → `0.2.20`；推送 `main` 与 tag `v0.2.20` 由 Actions 构建。
- Release <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.20>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` 5,426,606 字节，
  GitHub asset digest
  `sha256:61f1a69f4211cc0b7aab095e4a6895dae689f87ade20f8448fead03e0f909269`。

## 2026-11-04 18:37 — 客户端 IP/位置、出站检测、日志历史等 7 项增强

- 新增 `src/api/ip_location.rs`：`POST /panel/api/clients/ipLocation` 服务端
  查询 ipwho.is（失败回退 ipapi.co），输出 country/region/city/isp/timezone。
- 新增 `src/outbound_check.rs`：按 `x3ui_settings` 的
  `outboundCheckEnabled/Tags/IntervalMinutes` 周期检测；复用
  `outbound_test::probe_real`（临时 Xray SOCKS + HTTP 204）；不可用/恢复经
  `notify::send_text` 发送企业微信通知；`GET /panel/api/xray/outboundCheckStatus`
  返回最近结果。AppState 增加 `outbound_check` 与 `outbound_check_notify`。
- `src/panel_log.rs`：新增 UTC+8 固定时区 timer、按日文件 writer
  （`panel-logs/panel-YYYY-MM-DD.log`，保留 14 天）、文件列表与安全文件名解析。
  `main.rs` 的 tracing 增加文件 layer，缓冲区/文件记录 target/file/line。
- `src/config.rs`：新增 `panel_log_dir`（`PANEL_LOG_DIR`，默认 DB 目录下
  `panel-logs`）。
- `src/api/compat.rs`：
  - 新增 `clients_online_ips`、`server_panel_log_files`；日志接口支持
    `source=panel&file=<name>` 历史读取；
  - `setting_all` 增加出站检测默认值；`setting_update` 校验检测频率/去重 tags，
    保存后唤醒检测 job；移除 `xrayTemplateConfig` 的即时 `service::apply`；
  - 增加入站/客户端增删改、Xray 重启/停止、面板重启、设置保存日志。
- `src/xray/supervisor.rs`：`parse_version_line` 提取 `26.10.31` 短版本。
- `src/error.rs` / `src/api/*` / `src/updates.rs` / `src/panel_update.rs` /
  `src/outbound_subs.rs` / `src/xray/*`：面向用户的英文错误统一中文；Xray
  原生 stdout/stderr 保持原文。
- 前端：
  - `ClientInfoModal` 在线时显示当前 IPv4/IPv6，按 IP 提供“位置”按钮（默认不查）；
  - `AdvancedTab` 新页面（启用/多选出站/频率/微信 Webhook/最近结果）；
    侧栏增加“高级”；
  - `LogModal` 增加实时/历史日志选择；
  - 设置页安全警告关闭状态持久化到 localStorage，登录成功后清除；
  - `OverviewActionBar` 显示短版本号；
  - 设置页“重启面板”按钮始终可用；i18n 新增 en-US/zh-CN/zh-TW 文案。
- 验证：`cargo test` 107/107、clippy 零警告、fmt 通过、前端构建成功、
  `tsc` 无新增错误；`tmp/e2e-advanced/e2e.py`、`ui-run.py`、
  `online-e2e.py` 全部通过（真实 Xray 26.10.31 + mock 微信 + 无头 Chrome +
  真实 VLESS 客户端连接）。
- 未打包、未 bump 版本；源码提交见相邻提交。

## 2026-11-04 18:45 — 0.2.21 发布准备

- 版本 `Cargo.toml` / `Cargo.lock`：0.2.20 → **0.2.21**。
- 内容：0.2.20 全部功能 + 2026-11-04 七项增强（CURRENT-STATUS §2.30）。
- 发布前验证：`cargo test` 107/107、clippy 零警告、`cargo fmt --check`、
  前端 `npm run build`；`tmp/e2e-advanced/e2e.py`、`ui-run.py`、
  `online-e2e.py` 全通过。
- 发布路径：更新 CLAUDE.md/README/记忆文档 → 提交 `85f85da` push `main`
  （CI `41816323238` 成功）→ 推送 tag `v0.2.21` → release run
  `41816700205` 成功创建 Release。
- Release：<https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.21>
  唯一资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` 5,465,980 字节，
  GitHub asset digest
  `sha256:143e27bfdc28fa745f433c54337c52c17daa81709d05d2296236639bf47f101a`。
- 本地下载校验：tar.gz SHA-256 与 digest 一致；包内 `sha256sum -c` 通过；
  二进制 SHA256 `997fb41ace44b6d69f4fa1502e1aae0aedb9dbffd2d8b9d44329f208cfe3dcdb`，
  12,861,232 字节，x86-64 静态 ELF、已 strip，内含 `LUMEN_VERSION=0.2.21`。
- 未验证：真实 Linux/musl 运行（TODO T26）。

## 2026-11-04 22:37 — 位置查询改用纯真库（zxinc）

- 背景：用户反馈 IP 位置查询与 tool.lu/ip 不一致。复现：ipwho.is 将
  `115.193.109.37`（浙江杭州电信）定位为江苏南京；zxinc/淘宝均返回浙江杭州。
- 改动：`src/api/ip_location.rs`
  - 主源 ipwho.is → zxinc（`ip.zxinc.org/api.php?type=json`，纯真库，免 key）；
  - 新增 `parse_zxinc`：拆分 `data.country` 的 `–`/Tab 层级（兼容 IPv6 的空层），
    `data.local` 作为运营商并折叠多余空白，多出的层级并入城市；
  - 回退链改为 zxinc → ipwho.is；移除 ipapi.co（常返回 RateLimited）；
  - 单元测试：新增 3 个 zxinc 用例，移除 2 个 ipapi 用例。
- 文档：`docs/API.md`、`docs/ARCHITECTURE.md`、`docs/KNOWN-ISSUES.md` H5、
  `docs/DECISIONS.md` D043、`memory/CURRENT-STATUS.md` §2.32。
- 验证：`cargo fmt`、`cargo test` 109/109、`cargo clippy --all-targets -- -D warnings`
  零警告；联网实测杭州电信 IP、Google DNS、IPv6 地址解析正确。
- 未打包、未 bump 版本（仍 0.2.21）。

## 2026-11-04 22:50 — IP数据云高精查询（独立设置页）

- 背景：用户要求显示 tool.lu 的“高精归属地定位（IP数据云）”字段；该接口
  需要自有 API Key，不能直接抓 tool.lu（返回 204 且页面禁止作为 API）。
- 后端：
  - `src/api/ip_location.rs`：`ip_location` 改为读 `State`，从
    `settings.x3ui_settings` 取 `ipDataCloudKey`；配置后与纯真并行查询
    `api.ipdatacloud.com/v2/query`，成功附加 `ipDataCloud`，失败附加
    `ipDataCloudError`，不影响主结果；新增 `parse_ipdatacloud` 与 3 个单测。
  - `src/api/compat.rs`：`setting/all` 默认值加入 `ipDataCloudKey`；保存时
    去空白、拒绝空白字符并按字符串存储（避免纯数字 Key 被转成 number）。
- 前端：
  - 新增 `pages/settings/IpDataCloudTab.tsx`，路由 `/settings#ipdata`，侧栏
    “IP数据云”入口，与“高级”连接检测分开；
  - `AllSetting`/`setting` Zod 增加 `ipDataCloudKey`；
  - `ClientInfoModal` 位置结果分“纯真/ipwho.is”和“IP数据云”两行显示，
    未配置 Key 只显示第一行，Key 错误显示第二行错误；新增标签文案。
- i18n：`internal/web/translation/en-US.json`、`zh-CN.json`、`zh-TW.json`
  增加 `pages.settings.ipDataCloud*` 与 `pages.clients.ipLocationPure/
  ipLocationIpDataCloud`，其余 locale 走 en-US fallback。
- 文档：`docs/API.md`、`docs/ARCHITECTURE.md`、`docs/KNOWN-ISSUES.md` H5、
  `docs/DECISIONS.md` D044、`memory/CURRENT-STATUS.md` §2.33。
- 验证：`cargo fmt --check`、`cargo test` **112/112**、clippy 零警告、
  `npm run build` 成功；`tsc` 改动文件无新增报错；oxlint 仅剩 `ClientInfoModal`
  既有 `set-state-in-effect`；假 Key 实测接口返回 `key不正确`；
  `tmp/e2e-advanced/ipdc-check.py`、`ipdc-ui.py` 全部通过。
- 未验证：有效 Key 的成功响应/真实精度、真实 Linux、额度耗尽提示。
- 未打包、未 bump 版本（仍 0.2.21）。

## 2026-11-04 22:57 — 版本策略改为每次修改 bump，0.2.22 推送

- 用户要求：推送当前代码；以后只要有修改，版本号都增加。
- 版本：`Cargo.toml` / `Cargo.lock` 从 `0.2.21` → **`0.2.22`**（未打包、
  未打 tag、未创建 Release）。
- 规则：`CLAUDE.md` 规则 3 与会话结束清单改为“每次代码修改都递增版本号”；
  `docs/DECISIONS.md` 追加 D045；`memory/NEXT-STEPS.md` 同步期望版本与策略。
- 提交：`01c2586 feat: accurate IP location with optional IPDataCloud lookup`，
  已推送 `main`（`c7ce15a..01c2586`）。
- 验证：`cargo test` **112/112**、clippy 零警告、fmt 通过；二进制版本显示
  `0.2.22`；前端构建与无头 Chrome 检查、假 Key 端到端见同日上一条。
- 仍未验证：有效 IP数据云 Key 的成功响应（T28）；真实 Linux/musl。

## 2026-11-04 23:15 — v0.2.22 发布

- 版本 `Cargo.toml` / `Cargo.lock` = 0.2.22；tag `v0.2.22` → 提交 `99612d0`。
- 发布 Actions：run `41853368185` 成功；Release
  <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.22> 创建。
- 资产：`xray-panel-rs-linux-amd64-lumen.tar.gz` 5,473,730 字节，
  GitHub asset digest
  `sha256:c3a60555bd500a17ea52a53fe39ac56436ffe529c404a081e09e2cb1044c44e9`。
- Release 说明：用户要求“只显示更新内容”。修改 `.github/workflows/release.yml`
  的 body 为更新列表并关闭 `generate_release_notes`；用一次性 workflow
  `41854912426` 将已发布 v0.2.22 的 body 改为仅更新内容，随后删除该 workflow
  （提交 `89c4c2c`、`8a82b65`）。更新说明存档
  `.github/release-notes/v0.2.22.md`。
- 验证：发布前 `cargo test` 112/112、clippy、fmt、前端构建、假 Key 端到端、
  无头 Chrome 设置页检查通过。GitHub API 已确认资产大小与 digest；本机下载
  发布包 120 秒超时（仅收到 81 KB），包内 `SHA256SUMS` 未本地复核。
- 未验证：有效 IP数据云 Key 成功响应（T28）、真实 Linux/musl（T26）、
  真实公网客户端（T27）。

## 2026-11-04 23:14 — Release 更新内容标题去掉版本号

- 用户要求“更新内容标题前面不要显示版本号”。
- 修改 `.github/workflows/release.yml` 与
  `.github/release-notes/v0.2.22.md` 的标题为 `## 更新内容`。
- 用一次性 workflow（run `41855673244`）更新已发布的 v0.2.22 body，随后删除
  该临时 workflow（提交 `336e346` 及其后续删除）。
- 验证：GitHub API 确认 v0.2.22 body 首行为 `## 更新内容`，不再包含
  “0.2.22 更新内容”或一键安装段落。

## 2026-11-04 23:30 — IP数据云并入高级标签页

- 用户要求：IP数据云放在“高级”菜单里面，原有功能叫“出站检测”，两者在
  同一页面切换，样式与常规设置页一致。
- 前端：
  - 新增 `AdvancedTabs.tsx`，用 AntD Tabs 分出“出站检测”和“IP数据云”；
  - `SettingsPage` 的 `#advanced` 改为渲染 `AdvancedTabs`，移除 `#ipdata`；
  - `AppSidebar` 移除独立“IP数据云”子项；
  - i18n 增加 `pages.settings.outboundCheckTab`（en/zh-CN/zh-TW）。
- 版本：`Cargo.toml` / `Cargo.lock` 0.2.22 → **0.2.23**（未打包/未打 tag）；
  提交 `875703f`，已推送 `main`。
- 验证：`cargo test` 112/112、clippy 零警告、fmt 通过；前端构建成功；
  `tmp/e2e-advanced/ipdc-ui.py` 无头 Chrome 验证两个标签可切换、IP数据云
  API Key 输入存在、旧出站检测内容正常（桌面视口）。

## 2026-11-04 23:42 — v0.2.23 发布

- 版本 0.2.23；tag `v0.2.23` → 提交 `a0aa557`。
- 发布 Actions：run `41859508942` 成功；Release
  <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.23> 创建。
- 资产：`xray-panel-rs-linux-amd64-lumen.tar.gz` 5,473,227 字节，
  GitHub asset digest
  `sha256:9bd16e8409f3c38098ef730f05bfb8f8e7bd49764eedab8225e9e5f40ced3811`。
- Release 说明：标题 `## 更新内容`（无版本号），只列 0.2.23 标签页调整条目；
  无安装/更新说明和 Full Changelog（D046/D047）。
- 验证：发布前 `cargo test` 112/112、clippy、fmt、前端构建、无头 Chrome
  高级页标签切换检查通过。发布包本机下载校验因网络受限未完成。
- 未验证：有效 IP数据云 Key 成功响应（T28）、真实 Linux/musl（T26）、
  真实公网客户端（T27）。

## 2026-11-05 01:35 — 流量统计/速度对齐 3x-ui（0.2.24，未打包）

- 起因：用户反馈入站/客户端流量计算不对、速度与实际使用对不上，要求对比
  3x-ui 并做到准确。
- 对比结论：3x-ui 的流量 job 只用 `reset=false` 的累计计数 + 内存基准算增量；
  重置流量只清数据库累计值，从不清零 Xray 计数器。本项目多处调用
  `reset_pattern(reset=true)` 且兼容层忽略失败，存在重复计费路径；速度由前端
  差分列表快照，全部空闲时永远不更新。
- 修改：
  - `src/db/{clients,inbounds,outbounds}.rs`：重置只清 `up/down`，保留
    `last_up/last_down` 基准。
  - `src/db/client_traffic.rs`：删除 reconcile 机制与 `reset_client_verified`/
    `mark_reconcile`；新增“重置后只计新字节”测试。
  - `src/xray/stats.rs`：`query` 不再带 reset，删除 `reset_pattern`。
  - `src/xray/identity.rs`：删除 `traffic_pattern`。
  - `src/jobs.rs`：定时重置改为只清 DB；新增 `AppState.traffic_speeds`
    速率快照（真实 `Instant` 间隔）、`speed_sample`、`client_samples`
    （MASQUE 裸邮箱）、采样前后 `start_epoch` 防重启竞态；错误/重启时清速度。
  - `src/state.rs` / `src/main.rs`：新增 `TrafficSpeeds`/`SpeedSample` 状态及
    维护期 shrink。
  - `src/api/compat.rs`：slim 入站与客户端 `traffic` 下发 `upSpeed/downSpeed`；
    所有兼容重置接口改为 DB-only。
  - 前端 `useInbounds.ts`/`useClients.ts`/`dbinbound.ts`/schemas：删除快照差分
    与 15 秒缓存，直接展示后端速率。
  - 文档：`docs/{ARCHITECTURE,DATABASE,API,KNOWN-ISSUES,DECISIONS}.md`
    （D049；G7/G8）。
- 版本：0.2.23 → **0.2.24**（未打包/未打 tag）。
- 验证：`cargo test` 113/113、clippy 零警告、fmt 通过、前端 build 成功；
  `tmp/e2e-traffic-accuracy/e2e.py` 13/13 PASS（首次计费、速度上下行比例、
  空闲清速度、客户端/入站重置不重复计费）。
- 未验证：真实 Xray 计数器口径、真实网络速度抖动、MASQUE 混合附着真实计数、
  真实 Linux/musl。

### 2026-11-05 01:34 — 追加真实 Xray 验证

- `tmp/e2e-traffic-real/e2e.py`：真实 Xray 26.10.31（Windows），客户端
  SOCKS→VLESS→本地限速 HTTP 服务器，15/15 PASS。
- 关键数据：8 MiB 下载，客户端 `down=8,388,725`，入站 `down=8,388,608`
  （差 117 字节 HTTP 头）；速度观测 `2,158,371 B/s`；空闲 3 秒归零；
  重置后再下载 2 MiB，只新增 `2,097,269`，未重计旧累计；入站重置清成员
  客户端。
- 测试环境注意：Xray 26 的 freedom 默认黑洞来自 VLESS 等入站的私有/回环
  目标（SSRF 防护），e2e 通过 direct outbound 的
  `settings.finalRules:[{action:allow,ip:[geoip:private]}]` 放开；已记录到
  `KNOWN-ISSUES.md` G7。

## 2026-11-05 02:10 — 恢复 3x-ui 实时流量 WS（0.2.25，发布）

- 用户反馈：无关联客户端的入站速度显示错误、实时速度显示有问题，要求以
  3x-ui 为准。
- 修改：
  - `Cargo.toml`：axum 加 `ws` feature；版本 0.2.24 → 0.2.25。
  - `src/state.rs` / `src/main.rs`：`AppState.ws_events` broadcast 通道。
  - `src/api/ws.rs`：会话保护的 `/ws`，转发 `{type,payload,time}` 帧；
    `src/api/mod.rs` 注册路由（base path guard 负责 `/secret/ws` 重写）。
  - `src/jobs.rs`：每轮收集入站/客户端增量，广播 3x-ui `traffic` 帧
    （含 online 集合、lastOnlineMap 毫秒、clientTrafficIntervalMs）；
    统计 RPC 失败时广播空增量清速度。
  - 前端 `useWebSocket.ts` 重新启用；`useInbounds` 用帧内真实间隔换算；
    `useClients` 的 WS 速度覆盖 REST 兜底，避免重复相加。
  - 文档：`docs/ARCHITECTURE.md` §6.1、`docs/API.md`、`CLAUDE.md`、
    `DECISIONS.md` D050。
- 验证：`cargo test` 113/113、clippy/fmt、前端 build；fake e2e 13/13；
  无客户端入站 4/4；真实 Xray 15/15；无头 Chrome UI+WS 10/10（两个入站行
  均显示速度，其中 Solo 无客户端；WS 帧含无客户端入站增量与 2s 间隔；
  空闲后两行速度均消失）。

## 2026-11-05 02:18 — v0.2.25 发布

- 版本 0.2.25；提交 `ade85bd`，tag `v0.2.25`。
- Release <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.25>
  资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` 5,516,624 字节，
  GitHub asset digest
  `sha256:d9128d0c349011dd0f8d22829857c8cf4c3b3e8002e8d3f62c0bb8d84db792a5`。
- Actions：release run `41884947044` 成功；main CI run `41884941034` 成功。
- 内容：恢复 3x-ui `/ws` 实时 traffic 推送；无客户端入站速度由入站计数器
  实时显示；REST 速率保留兜底。

## 2026-11-05 19:xx — 优化实时 WS 流量帧内存（0.2.26，待发布）

- 用户反馈：0.2.25 实时速度更新后面板内存一直很高。
- 定位：`jobs::tick` 每轮无条件收集并序列化完整 3x-ui traffic 帧，即使没有
  任何 WS 订阅者；`json!` 为每个入站/客户端条目建 Value 树；broadcast 缓冲
  保留 8 帧；WS 连接没有 ping/空闲/发送超时。
- 修改：
  - `src/jobs.rs`：`ws_enabled = ws_events.receiver_count() > 0`，为 0 时跳过
    增量收集；`broadcast_traffic` 入口再次检查。增量改用借用 `&str` 的
    `InboundTraffic` / `ClientTraffic` 结构体，整帧 `serde_json::to_string`。
  - `src/main.rs`：`broadcast::channel(8)` → `channel(2)`。
  - `src/api/ws.rs`：30s Ping、90s 空闲断开、10s 发送超时。
  - 文档：`CURRENT-STATUS.md` §2.42、`ARCHITECTURE.md` §6.1、
    `DECISIONS.md` D051、`KNOWN-ISSUES.md` G10、`PROGRESS.md`；版本 0.2.25 →
    0.2.26（Cargo.toml/Cargo.lock）。
- 内存对比（Windows debug，3000 客户端，2s 采集，每秒流量）：
  - 无 WS：0.2.25 末值 28.2 / 峰值 34.1 MiB；0.2.26 27.0 / 32.8 MiB。
  - 一个 WS 读取端：0.2.25 28.8 / 34.7 MiB；0.2.26 27.8 / 33.7 MiB。
  - 收益随活跃客户端数增加；无面板连接时不构建帧。
- 验证：`cargo test` 113/113、clippy 零警告、fmt；fake e2e 13/13、
  无客户端入站 4/4、无头 Chrome UI+WS 10/10；WS 帧字段与 0.2.25 一致。
- 未验证：真实 Linux/musl 长时 RSS、超大客户端规模的真实内存曲线。

## 2026-11-05 19:05 — v0.2.26 发布

- 版本 0.2.26；提交 `4ccbec5`，tag `v0.2.26`。
- Release <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.26>
  资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` 5,518,877 字节，
  GitHub asset digest
  `sha256:9c223ffae10e0c308c67078d90bef97dad3fb8ea02e07831114fd751c1baae2f`。
- Actions：release run `42015308253` 成功；main CI run `42015280025` 成功。
- 内容：0.2.25 实时 WS 流量帧的内存优化（无订阅者不建帧、直接序列化、
  缓冲 2 帧、WS 保活/空闲/发送超时）。

## 2026-11-05 22:10 — 实时速度除数修复（0.2.27，未打包）

- 用户反馈：入站的实时速度还是显示不准确。
- 复现：新增 `tmp/diag-traffic-ws/diag.py`（真实 `/ws` 客户端 + fake
  StatsService，计数器按固定 1.0/2.0 MB/s 增长；`tmp/fake_stats` 增加
  `FAKE_STATS_DELAY_MS` 钩子模拟慢统计 RPC）。
  - 正常 tick：帧间隔 2000ms，显示 1.000/2.000 MB/s，正确。
  - 统计 RPC 延迟 3s、轮询 2s：修复前帧间隔仍写 2000ms，显示
    **1.500/3.000 MB/s**（50% 虚高）；REST 为 0.99/1.99 MB/s。
  - 首轮无上一采样点时，帧把启动以来的累计值当成一个窗口，出现 2.75× 尖峰。
- 修改：
  - `src/jobs.rs`：帧的 `clientTrafficIntervalMs` 改用本轮真实 `elapsed_ms`；
    `elapsed_ms <= 0` 时累计照常计费但不广播速率增量（`rated_interval_ms`）；
    Xray 采样期间重启时广播空增量清速度；无速率帧回退配置轮询值。
  - `tmp/fake_stats/src/main.rs`：`FAKE_STATS_DELAY_MS` 测试钩子。
  - `tmp/e2e-traffic-ui/ui_check.mjs`：间隔断言改为 1500–4000ms 区间。
  - 文档：`docs/{ARCHITECTURE,API,DECISIONS}.md`（D052）、`CURRENT-STATUS.md`
    §2.43、`CLAUDE.md` 版本说明；版本 0.2.26 → 0.2.27
    （Cargo.toml/Cargo.lock）。
- 验证：`cargo test` **114/114**（新增 1 个）、clippy 零警告、fmt；修复后慢
  RPC 场景显示 0.998/1.995 MB/s（真实 1.0/2.0）；fake e2e 13/13、
  UI+WS 10/10、真实 Xray 15/15（max speed 2,156,219 B/s）。
- 未验证：真实 Linux/musl、真实公网抖动、超大客户端数下 tick 超时的实际比例；
  前端源码未改动，`web/dist` 沿用 0.2.26 构建。

## 2026-11-05 23:00 — v0.2.27 发布

- 版本 0.2.27；提交 `869facb`，tag `v0.2.27`。
- Release <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.27>
  资产 `xray-panel-rs-linux-amd64-lumen.tar.gz` 5,518,387 字节，
  GitHub asset digest
  `sha256:d02091227e04682c8c26328c924f03394d68318fd79b3cbdaa9cb96d5c65138b`。
- Actions：main CI run `42042345225` 成功；release run `42042854755` 成功。
- 内容：WS 帧改用真实采样间隔（修复慢 tick 下实时速度虚高）、首轮无速率
  不广播、Xray 采样期间重启时清空速度；另附 tunnel 零客户端真实流量验证。
- 本机下载发布包被网络重置，未做本地解包复核；资产大小/digest 来自
  GitHub API，包内 `SHA256SUMS` 与 `file` 由 release workflow 输出。

## 2026-11-05 — 入站 WS 速度断线冻结修复 + v0.2.28 发布

- 用户反馈：0.2.27（Linux 部署）入站列表实时速度仍虚高/偏低。
- 梳理：复读 `src/jobs.rs` 全链路（增量 ÷ 真实间隔、`counter_delta` 回退、
  epoch 丢弃、空闲清除）确认后端计算无误；向用户确认症状为
  「入站列表速度列，幅度虚高/偏低」，并给出 DevTools `__diagDump`
  诊断脚本供现场采集 WS 帧/REST 速率。
- 定位（代码审查，非复现）：`useInbounds.ts` 的 `wsInboundSpeed` 无过期
  机制（客户端页有），WS 半途断开时入站速度列永久冻结，且冻结项覆盖
  REST 兜底值——空闲时虚高挂着、随后与真实速率脱节（偏低/跳变）。
- 修改：
  - `frontend-3xui/src/pages/inbounds/useInbounds.ts`：每帧重置 2×帧间隔
    （≤2 分钟）过期计时器（版本计数门闩）；超时清空 `wsInboundSpeed`，
    回退 REST `upSpeed/downSpeed`；卸载清理计时器。
  - 版本 0.2.27 → 0.2.28（Cargo.toml/Cargo.lock），前端重新构建嵌入
    `web/dist/`。
  - 文档：`DECISIONS.md` D053、`KNOWN-ISSUES.md` G12、`CURRENT-STATUS.md`
    §2.45、`release.yml` body（更新内容改为本次两条）。
- 自测：`cargo test` **114/114**、clippy 零警告、fmt、前端 `npm run build`。

## 2026-11-05 23:00 — v0.2.28 发布（GitHub Actions + 本地包级复核）

- 版本 0.2.28；提交 `5e376a1`，tag `v0.2.28`，main CI run `42048884488`
  成功；release run `42049648625`（head_sha=`5e376a1`）成功。
- Release <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.28>
  资产 5,518,877 字节，asset digest
  `sha256:f7ebdfba2946e487c503d77d535ba36714d24164578f381be1b6e125e6d58290`。
- 复核（经 `192.168.1.251:10813` HTTP 代理下载）：包 sha256 与 digest 一致；
  解包校验包内 `SHA256SUMS` 与二进制 `074a9293...f33af05e8c` 匹配；二进制含
  `LUMEN_VERSION=0.2.28` 与 `clientTrafficIntervalMs`；解包产物存
  `tmp/pkg-0.2.28/`。
- 未验证：真实 Linux/musl 运行；真实断线场景（代理掐长连接/睡眠恢复）。

## 2026-11-05 23:40 — 按 3x-ui 模型重写实时流量取数（0.2.29，未发布）

- 状态：已完成并本地验证 / 真实部署未验证
- 用户反馈：0.2.28 部署后「还是不准确，又是大流量面板都没有显示」，
  并明确指令「看 3x-ui 是怎么做的，把他的代码用 Rust 重写」。
- 上游对照（代理拉取，存 `tmp/xui-ref/`；关键文件
  `internal/xray/api.go` GetTraffic 1028–1135 行）：
  - gRPC 接收上限 64 MiB（`maxStatsReplyBytes`），注释明说 ~45k 客户端
    的统计回复可单独超过默认 4 MiB —— 整轮失败即速度全空；
  - `statsFamilies = ["user>>>", "inbound>>>", "outbound>>>"]` 三次
    QueryStats，单族失败只 warning、其余族继续；
  - `StatsLastValues` 内存基线 + `statsBaselined` 族首过标记：未见/
    回退计数器从 0 起算；族失败保留基线；全部族成功且 map > 2×live 时
    才剪枝。
- Rust 重写：
  - `src/xray/stats.rs`：`MAX_STATS_REPLY_BYTES = 64 << 20`（connect 时
    `max_decoding_message_size`）；新增 `query_families()` 返回
    `Vec<FamilyStats>{family, stats, error}`，全部族失败才 Err 并断线
    重连；单族 RPC 前缀过滤（核心全名匹配，3x-ui 同款防御）；
    `query()` 移除，连接逻辑收敛进 `ensure_connected`。
  - `src/jobs.rs`：spawn 持有 `stats_last: HashMap<DeltaKey, i64>` 与
    `stats_baselined: HashSet<&'static str>`；Xray start_epoch 增加时
    两者与 DB 基线一起清零（重连后的首个采样只建基线）。新增
    `display_deltas()` —— WS 帧 `traffics`/`clientTraffics`、
    `AppState.traffic_speeds`、本轮流量信号（moved_emails/active_tags）
    改用其增量；DB 计费（`apply_tick`/`add_traffic`/配额到期执行/
    outbound 累计/history）保持原事务路径不动。
    低内存硬约束：`DeltaKey{kind:u8, uplink:bool, key:String}` 代替完整
    stat 名（省 ~30B/条）；只记录 traffic 计数器；`2×live` 剪枝；
    0 增量不入表；无订阅者不建帧（0.2.26 沿用）。
  - `parse_user_stats` 改为迭代器入参；计费端 in/out 原始值表按族填充
    （失败族计费同样跳过，与显示口径一致）。
- 版本 0.2.28 → 0.2.29（Cargo.toml/Cargo.lock）。文档：
  `docs/DECISIONS.md` D054、`docs/KNOWN-ISSUES.md` G13、
  `docs/ARCHITECTURE.md` §6 tick 流程重写、`CLAUDE.md` 版本说明、
  `release.yml` body（更新内容改 0.2.29 四条）。
- 验证：`cargo test` **119/119**（新增 5：首过只建基线、未见/回退从 0、
  族失败保基线并按跨度补算、身份折叠/裸邮箱回退、剪枝与失败例外）、
  clippy `--all-targets -D warnings` 零警告、`cargo fmt --check`。
- 未验证：真实 Linux/大面板部署（本次修复的目标场景）；前端未改动。

## 2026-11-05 23:55 — v0.2.29 发布（GitHub Actions + 本地包级复核）

- 用户确认「发布」。main CI run `42056105722` 成功 → tag `v0.2.29`
  （提交 `117b418`）→ release run `42057016611` 成功。
- Release <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.29>
  资产 5,523,004 字节，asset digest
  `sha256:c3b86b06cded35a39fe676eff80ab344468294dce83c6d60dfe8819c631896e3`；
  body 四条更新内容中文 UTF-8 存储正确。
- 复核（经 `192.168.1.251:10813` HTTP 代理下载）：包 sha256 与 digest
  一致；解包校验包内 `SHA256SUMS` 与二进制
  `524be1513bf29823b59c4fec74d9a9bd73a48fe3d87f159062d7addc756daedb`
  匹配；二进制含 `LUMEN_VERSION=0.2.29` 与 `clientTrafficIntervalMs`；
  解包产物存 `tmp/pkg-0.2.29/`。
- 未验证：真实 Linux/musl 大面板运行效果（等待用户部署反馈）。

## 2026-11-06 01:30 — 采样与计费解耦：串行计费 writer（0.2.30，未发布）

- 状态：已完成并本地验证 / 真实大面板现场未验证
- 用户反馈链：「tunnel 协议的流量显示还是不准确」→ 追问确认「数值偏大/
  偏小」+「rewriteAddress」+「最新核心」→「累积量会变，实时速度和服务器
  监控不一样」→「3x-ui 大很多」→ 轮询间隔「2 秒」。
- 本地复现排除 tunnel 特殊性：`tunnel_real.py`（真实 Xray）PASS——
  8,388,608 B 下载 → 面板增量 8,388,725（+82 B 头），WS 帧间隔
  [2003,2015,2016]ms 稳定、峰值 2.16 MB/s 与 REST 一致、空闲归零；
  `diag.py`（fake-stats 1.0/2.0 MB/s）帧 1.01/2.03 MB/s。
- 根因（对照 `tmp/xui-ref/`）：3x-ui 显示 = 增量 ÷ 硬编码 5s，且计费在
  独立串行 writer（`submitTrafficWrite`）里，采样节拍不沾 DB；本面板
  tick 串行「采样→逐行计费写→广播」，poll=2s + 大部署一轮 DB 写拖十几秒
  → 采样间隔被拉长 → 本轮字节 ÷ 整个积压 = 十几秒平均 ≪ 瞬时值。
- 改动（`src/jobs.rs`，D055/G14）：
  - tick 显示路径先跑完：`traffic_speeds`、在线/活跃（信号改用显示增量）、
    WS 广播；`last_stats_at` 在采样后即推进。
  - `BillingSample`（epoch/now/online_ip_counts/user_stats/inbound_*
    outbound_* 原始值/moved/online emails）→ `BillingSink` 单槽合并队列
    （`Arc<Mutex<Option<_>>>`+`Notify`，next 先装 notified 再查槽；push
    替换未落库旧样本——原始计数器互相包含，合并不重计不丢）。低内存：
    常驻上界一个样本。
  - `run_billing_writer`+`apply_billing`：重读 clients/inbounds，attachment
    计费与 DB 基准比较、legacy 聚合路径、MASQUE 双计数、配额/到期/limitIp
    禁用、reset_days/入站重置、`persist_online_heartbeat`、出站表、历史
    （逻辑从旧 tick 尾部原样迁移）；失败只 warn；epoch≠当前进程整份丢弃。
  - 失败路径（stats 全失败/epoch 中途变化）：广播先于心跳持久化，持久化
    失败只 warn。
- 版本 0.2.29 → 0.2.30（Cargo.toml/Cargo.lock）。文档：D055、G14、
  ARCHITECTURE §6、CLAUDE.md、release.yml body（0.2.30 四条）、
  CURRENT-STATUS §2.49/§2.50、NEXT-STEPS。
- 验证：`cargo test` **121/121**（新增 2：槽只留最新、唤醒等待者）、
  clippy `--all-targets -D warnings` 零警告、fmt；tunnel_real + diag.py
  两条本地链路回归 PASS（前端未改动）。
- 未验证：真实 Linux 大面板（本轮症状场景）；待用户部署 0.2.30 对照
  3x-ui/网卡监控确认收敛到包头级差异（3–8%）。

## 2026-11-06 02:20 — v0.2.30 发布（GitHub Actions + 本地包级复核）

- 用户确认「发布」。main CI run `42067360254` 成功 → tag `v0.2.30`
  （提交 `c3bd5b5`）→ release run `42068104145` 成功。
- Release <https://github.com/ztoofar/xray-panel-rs/releases/tag/v0.2.30>
  资产 5,527,291 字节，asset digest
  `sha256:d4c2c816efa440567bb9271211e675e6af1d5bb31f589d06af95f39ad70bf429`；
  body 四条更新内容中文 UTF-8 存储正确。
- 复核（经 `192.168.1.251:10813` HTTP 代理下载）：包 sha256 与 digest 一致；
  解包校验包内 `SHA256SUMS` 与二进制
  `ba98ca739144ae24bb9b453a76f417ebbe911950e9e20b3989dfc2975f7ab50c` 匹配；
  二进制 13,000,368 字节含 `LUMEN_VERSION=0.2.30` 与 `clientTrafficIntervalMs`；
  解包产物存 `tmp/pkg-0.2.30/`。
- 文档：CURRENT-STATUS §2.50（新增发布记录；§2.49 标为已发布）、
  NEXT-STEPS 当前状态改 0.2.30、CLAUDE.md「待发布→已发布」。
- 未验证：真实 Linux/musl 大面板运行效果（等待用户部署反馈）。

## 2026-11-06 03:10 — 现场四方对照脚本（deploy/panel_speed_check.py）

- 用户反馈 0.2.30 部署后「显示还是不对」，无可量化数据。为避免继续猜：
  写现场诊断脚本 `deploy/panel_speed_check.py`（python3 标准库，服务器直接
  跑），同一窗口采样四个来源并自动判定丢数层：
  ① `xray api statsquery` 原始计数器（绕开一切面板代码）；
  ② 面板 WS `traffic` 帧：累计各轮 `Tag/Up/Down`，并输出**真实帧间隔 vs
  帧携带的 `clientTrafficIntervalMs` 除数**（两数差大=采样节拍被拖长，
  速度列摊薄——这正是 G14 的机制）；
  ③ REST `list/slim` 的 `upSpeed/downSpeed`（UI 同源的显示值）；
  ④ DB `inbounds.up/down` 计费增量。
- 登录走公开 `POST /login`（表单，session cookie）；REST 取样 GET 无需
  CSRF；WS 路径修复过 `//ws` 拼接错误（会命中 SPA 兜底路由返回 200 HTML）。
  DB 只读 URI 打开避免和 writer 抢锁。
- 端到端自测 `tmp/field-check/selftest.py`（真实 Xray + 隧道入站 + 持续
  单线程下载 18 秒）PASS：tunnel-in 四列 104 B/s+11.6~11.9 MB/s 相互一致，
  帧间隔 2.0s = 除数 2.0s；api 内部入站只出现在 ①（与 3x-ui 忽略 api 一致）。
- 本次不改产品代码（无 Cargo 版本 bump）；G14 补「现场状态」段。等用户
  回传现场表格再定根因。

## 2026-11-06 04:20 — 远程登录现场服务器复核（结论：管线正确，方向列误读）

- 用户授权远程操作。本机 PuTTY 会话 + 密钥（`D:\2\镇江.ppk`），plink 连
  现场服务器（Debian，`xray-panel` active），面板基础路径 `/morphine/`，
  监听 127.0.0.1:15104。
- 部署核对：二进制 13,000,368 字节 = v0.2.30（页面 `X_UI_CUR_VER=0.2.30`），
  二进制时间 Nov 6 00:51；`traffic_poll_secs=5`；Xray 26.10.31；
  `xray api statsquery` 可用；journal 6h 无 billing/stats 警告（20 次
  `xray started` 为现场反复改配置重启，epoch 递增、基线清零属预期）。
- 实测（临时只读 session 读 REST，用后即删；DB 只读 URI）：
  - 空闲：`in-802-tcpudp` 原始 uplink 1760 B/s → REST upSpeed=1422、
    downSpeed=0，一致；
  - 用户此前的 60s 数据：原始 3.7 KB/s+1.6 MB/s = WS 1.6 MB/s = DB
    1.6 MB/s = REST 均值 1.5 MB/s，帧间隔 5.0s=除数 5.0s。
  - 结论：3.32K 是 ↑ 上行列（下载时上行只有请求/ACK）；下载在 ↓ 列
    （~1.5–1.8 MB/s，突发十余 MB/s）。监控 4 MB/s ≈ 代理两条腿 + 包头。
- 未改产品代码。G14/NEXT-STEPS 记录结论；可选后续：速度标签改用
  「上传/下载」文案（待用户确认）。
