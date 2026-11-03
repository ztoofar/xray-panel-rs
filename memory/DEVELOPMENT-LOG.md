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
