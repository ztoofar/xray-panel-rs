# 技术决策记录（DECISIONS）

> **追加式文档：不得修改/删除已有决策。** 新决策在文件末尾追加 Dxxx，并写清
> 背景、选择、被否方案、原因和后果。后续推翻某决策时，新增一条“取代 Dxxx”，
> 不要改写原文。最后更新：2026-10-31。

---

## D001 Rust + Axum + SQLite，而不是继续用 Go/3x-ui 或 Node
- 日期：2026-10-30（项目启动）
- 背景：目标是在小内存 VPS 上运行 Xray 单机面板。
- 决策：后端用 Rust + Axum + sqlx(SQLite)，不引入 ORM/外部服务。
- 被否方案：直接使用 3x-ui（Go，功能全但内存/依赖更重）；Node/Python 面板
  （运行时依赖与内存不可控）。
- 原因：单静态二进制、无运行时依赖、可控内存；sqlx 比 ORM 更轻。
- 后果：需要自己实现 3x-ui 兼容 API；高级功能无法“白拿”。

## D002 发布目标为 `x86_64-unknown-linux-musl` 静态 ELF
- 日期：2026-10-30
- 背景：希望在任意 Linux amd64 上直接运行，不依赖目标机 glibc 版本。
- 决策：用 musl 静态链接、strip；开发期用 zig 交叉编译。
- 被否方案：gnu 动态链接（兼容性差）；提供多发行版包（维护成本高）。
- 原因：单一产物、部署简单。
- 后果：musl 没有 `malloc_trim`；定时内存维护不能依赖它（见 D012）。

## D003 Xray 由面板作为子进程管理，配置也由面板生成
- 日期：2026-10-30
- 背景：面板需要应用入站/路由/出站变更并统计流量。
- 决策：面板生成完整 Xray 配置，`xray run -test` 校验后原子替换，supervisor
  启动/停止/退避重启；不依赖外部 systemd 管理 Xray。
- 被否方案：面板只改配置、由 systemd 重启 Xray；面板通过命令行 `xray api` 统计。
- 原因：单进程管理生命周期、能保证测试失败不覆盖线上配置；常驻 gRPC 比每次
  拉起 `xray api` 子进程省内存。
- 后果：不可同时用 systemd 启动第二个 Xray；客户端/入站变更会重启 Xray，
  连接短暂中断；面板异常崩溃可能遗留 Xray 子进程。

## D004 采用 3x-ui 前端的裁剪副本，而不是自研表单或完整 3x-ui
- 日期：2026-10-31
- 背景：自研表单覆盖多协议/传输/TLS/REALITY 等工作量巨大；完整 3x-ui
  又带大量后端不支持的页面。
- 决策：复制 3x-ui React 前端，裁剪为七个路由（系统状态/入站/客户端/出站/
  路由/Xray/设置），Rust 后端实现这些页面需要的 `/panel/api/*`。
- 被否方案：继续手写自研表单（用户明确否决）；完整 3x-ui 前端全量接入
  （用户否决，后端不支持的页面会大量报错）。
- 原因：用户明确要求“入站/出站/客户端/路由/Xray 配置抄过来”；成熟表单能
  避免大量协议字段错误。
- 后果：前端与后端存在兼容层债务；未实现的高级页面/按钮会报错；
  `frontend/`（自研）与 `web/dist-custom-v2/` 保留但不生效。

## D005 兼容 API 不伪造成功
- 日期：2026-10-31
- 背景：裁剪前端仍会请求一些后端没有实现的 3x-ui 接口。
- 决策：写操作未实现返回 `success:false` 明确错误；只读接口可返回空列表/
  `null` 以便页面渲染，但绝不伪造业务成功。
- 被否方案：全部返回空成功，让 UI 看起来“能用”。
- 原因：避免管理员误以为 2FA/订阅/节点等功能已生效，掩盖真实风险。
- 后果：部分按钮仍会弹出错误；已在 `KNOWN-ISSUES.md` 记录。

## D006 统计身份使用 `<inbound_id>:<email>`（per-attachment）
- 日期：2026-10-31
- 背景：一个客户端可关联多个入站；Xray 的统计按入站 attachment 分开计数。
- 决策：写入运行配置的 client `email` 用 `<inbound_id>:<email>`；数据库和 API
  仍使用面板邮箱；统计汇总时按映射还原，在线归属可精确到入站。
- 被否方案：只用邮箱作为统计身份（多入站会混在一起，无法归属入站）；
  只统计主入站（丢失其他关联）。
- 原因：精确在线归属、正确汇总多 attachment 流量；重置时必须重置全部
  attachment，否则累计值下降会被 `delta()` 当成回绕重复计费。
- 后果：直接读 Xray stats 的外部工具会看到 `id:email`；routing 的 user 规则
  写盘时需展开身份（API 返回仍显示邮箱）。

## D007 不使用 WebSocket，改为轮询
- 日期：2026-10-31
- 背景：3x-ui 前端原本依赖 WebSocket 推送。
- 决策：禁用前端 WebSocket，数据通过轮询/操作后失效缓存刷新；后端 job 负责
  周期采集。
- 被否方案：实现完整 WS 推送。
- 原因：减少常驻连接与内存、缩短实现周期；低内存优先。
- 后果：数据最多滞后一个轮询周期；前端需要自行处理刷新。

## D008 设置存储：`settings` 键值表 + `x3ui_settings` JSON blob
- 日期：2026-10-31
- 背景：3x-ui 设置页一次提交大量字段，且字段随版本变化。
- 决策：3x-ui 设置整体存为 `x3ui_settings` JSON 字符串；轮询周期等后端直接
  消费的值用独立键（如 `traffic_poll_secs`）；启动时读取并覆盖部分环境变量。
- 被否方案：为每个设置建列/表（迁移成本高）；只信环境变量（无法页面修改）。
- 原因：复用 3x-ui 设置页、避免频繁迁移；后端只校验真正使用的字段。
- 后果：blob 无 schema，非法旧值需在读取时兜底；`PANEL_DB`/`PANEL_LOG`/
  `PANEL_WORKERS` 仍只能环境变量。

## D009 面板路径用“路由前中间件 + 外层 Router 包内层 Router”
- 日期：2026-10-31
- 背景：需要把面板挂到可配置前缀（`webBasePath`），且不重启进程。
- 决策：新增 `base_path::guard`：先判断 `/api/health`，再剥前缀并重写 URI；
  用外层 `Router::fallback_service(内层 Router)` 保证 guard 在路由匹配前执行。
- 被否方案：
  1. 在每个路由注册处拼接前缀（运行时无法改，且路由表巨大）；
  2. 同层 `Router::layer`（实测/文档：layer 在路由选择之后执行，
     `/secret/panel/...` 会先落到 fallback，改写后也无法再匹配 API）；
  3. 重建 Router（需要重启）。
- 原因：一次实现、所有路由和静态资源自动继承；保存即生效。
- 后果：所有请求多一次 URI 重写；静态 HTML 中的 `/assets/` 必须按前缀改写；
  `/api/health` 作为探活例外保留在根路径。

## D010 面板路径保存后立即生效并自动跳转
- 日期：2026-10-31
- 背景：3x-ui 一般要求重启面板后新路径生效。
- 决策：设置保存后立即替换内存前缀；前端检测到路径变化后
  `location.replace()` 到新地址。
- 被否方案：保存后要求用户手动重启/手动访问新地址。
- 原因：用户要求“可自定义在面板设置里面”，立即生效体验更直接；旧 URL 404
  能立刻验证。
- 后果：旧书签/打开中的页面立即失效；路径是混淆而非安全边界。

## D011 版本号只显示在系统状态页
- 日期：2026-10-31
- 背景：版本号原本显示在侧边栏品牌区（每个页面可见）。
- 决策：移除侧边栏/抽屉版本号；只保留系统状态页的版本按钮和更新弹窗。
- 原因：用户明确要求“版本号只显示在系统状态列表”。
- 后果：其他页面无法直接看到版本；升级/排障需进系统状态页。

## D012 每 15 分钟做一次内存维护
- 日期：2026-10-31
- 背景：面板是长期驻留服务，SQLite 页缓存/临时内存和内部容器可能保留峰值。
- 决策：维护任务每 15 分钟执行：清理过期会话、`PRAGMA shrink_memory`、
  收缩在线/日志/历史/限流容器；glibc 目标额外 `malloc_trim(0)`。
- 被否方案：每次请求都 trim（开销大）；定时重启面板（中断服务）；
  做成可配置项（用户未要求，增加复杂度）。
- 原因：低成本、无需中断；覆盖面板自身可控内存。
- 后果：musl 产物没有 `malloc_trim`，依赖 mallocng 在 free 时 madvise 归还；
  该维护不控制系统外部（如 Xray）内存。

## D013 改完不自动打包，版本只在打包时 bump
- 日期：2026-10-31
- 背景：用户连续迭代多个修复，每次打包浪费时间并污染 `dist/`。
- 决策：日常改动只自测；只有用户明确要求“打包/发布”时才 bump
  `Cargo.toml`、重建 musl 二进制并生成 tar.gz。
- 原因：用户明确要求（“你先不要每次改完都打包”）。
- 后果：源码可能领先最近发布包；`CURRENT-STATUS.md` 必须标明未打包改动。

## D014 单管理员、无 2FA / API Token
- 日期：2026-10-31
- 背景：低内存单机面板的目标用户只需要一个管理员。
- 决策：只实现单用户密码登录；不实现 2FA、API token、多用户权限。
- 被否方案：完整 3x-ui 用户/2FA/Token 体系。
- 原因：减少攻击面与实现量；`api_tokens` 表是 0001 遗留，不使用。
- 后果：兼容 API 的 2FA/Token 相关端点返回未实现；备份仍包含该表。

## D015 保留早期自研前端目录
- 日期：2026-10-31
- 背景：`frontend/` 是 3x-ui 前端替换前的自研实现。
- 决策：保留源码与 `web/dist-custom-v2/`，不参与构建、不主动删除。
- 原因：便于回溯/对比；删除是不可逆操作。
- 后果：目录容易让人误改；根 `CLAUDE.md` 明确标注“非生效”。

## D016 在线状态用 `GetUsersStats` + 流量增量取并集
- 日期：2026-10-31
- 背景：仅靠流量增量会漏掉“已连接但本轮没跑流量”的客户端；仅靠日志解析
  依赖日志格式/文件轮转。
- 决策：以 Xray `GetUsersStats`（需核心支持）的在线用户为主信号，并集本轮
  流量有增量的邮箱；窗口 = `2×poll`，限制 20–60 秒；Xray 重启时清空。
- 被否方案：只看流量增量（误判离线）；解析 access log（历史上曾用于在线归属，
  因日志格式/轮转依赖、额外 I/O 而由 per-attachment 统计取代）。
- 原因：更准确、无日志格式耦合。
- 后果：旧核心（无 `GetUsersStats`/`statsUserOnline`）在线列表为空；客户端
  断开后最多保留 60 秒（宽限窗口）。

## D017 SQLite 小连接池 + 小页缓存
- 日期：2026-10-30
- 背景：低内存 VPS。
- 决策：连接池 max 2/min 1，`cache_size=-2000`，WAL + `synchronous=NORMAL`。
- 原因：SQLite 写串行，连接多无益；小缓存显著降低 RSS。
- 后果：复杂查询可能更慢；大结果集依赖临时内存。

## D018 备份用 `VACUUM INTO`，恢复用事务内表复制
- 日期：2026-10-31
- 背景：需要一键下载/恢复 SQLite 数据；WAL 模式下直接拷贝 .db 可能不一致。
- 决策：下载用 `VACUUM INTO` 生成一致性快照；恢复要求 SQLite 文件头与四张
  核心表，按固定顺序在事务内清空/插入，失败回滚；成功后重启 Xray。
- 被否方案：直接复制 .db 文件（WAL 未合并）；整库文件替换（连接池/句柄复杂）。
- 原因：一致性、可校验、失败不破坏当前库。
- 后果：备份必须与当前 schema 兼容；不自动迁移旧备份。

## D019 per-attachment 流量基准 + 仅确认成功后清零
- 日期：2026-10-31
- 背景：0.2.6 把同一客户端所有入站的累计值求和后只保存一组 `last_up/last_down`。
  单个 attachment 重置或暂时缺失统计项时，聚合基准会被写坏；Xray 重启后基准
  也仍属于旧进程，可能永久少计。定时重置失败时还会直接清零数据库累计流量，
  下一轮重复计费。
- 决策：
  1. 新增 `client_inbound_traffic`，按 `(client_id, inbound_id)` 保存独立基准；
     面板生成的 `<inbound_id>:<email>` 统计逐 attachment 算增量后汇总。
  2. 缺失统计项（`None`）和负数样本一律保留原基准，不再 `unwrap_or(0)`。
  3. 客户端/入站/出站都同时比较上传与下载基准，单独一个方向回绕也能被发现。
  4. Xray 子进程新启动时（`start_epoch` 增加，含首次启动；停止不增加）先在
     同一事务里清零所有 `last_*` 基准，不动累计 `up/down`。generation 变化
     （启动或停止）只负责清在线集合和失效在线 API 探测缓存。这样不会在旧进程
     尚未退出时误用新基准采样；面板启动时若无子进程则保留基准以兼容残留进程。
  5. 定时重置只有在全部 Xray 计数器确认成功后才清数据库；部分失败只写
     `needs_reconcile=1` 并记录本次尝试，下一份完整样本接管新基准、不计增量。
     失败窗口内可能少计一小段流量，但不会重复计费。
- 被否方案：继续用聚合基准只修补 if 条件（无法处理单 attachment 重置）；
  部分失败时回滚 Xray（已经清零的计数器无法恢复）；失败后每 tick 重试
  （会对不可用核心反复发起 RPC 并拖慢轮询）。
- 原因：把“统计身份、采集基准、累计数据”分开，才能同时保证不丢历史、不重复计。
- 后果：升级后第一次 tick 会清零全部基准；若面板被 SIGKILL 且 Xray 残留运行，
  面板重启会把残留进程的累计计数当成新流量（项目约束是 Xray 必须由面板托管，
  systemd 默认会随服务清理子进程）。新增字段不进入 API，前端无改动。

## D020 Xray 可编辑模板：整份保存，policy/metrics 不再被静默丢弃
- 日期：2026-11-01
- 背景：Xray 设置页的“连接限制”（`policy.levels["0"].connIdle/bufferSize`）、
  统计开关（`policy.system.*`）和 `metrics` 修改后不生效。`/xray/update` 只复制
  固定白名单（log/dns/outbounds/routing/observatory/…），`policy`/`metrics`
  被静默丢弃；`ensure_policy` 还会强制覆盖所有统计开关；读取接口返回生成配置，
  高级模板里的 inbounds 修改同样无效；重置默认接口方法不匹配且返回当前配置。
- 决策：
  1. `/panel/api/xray/update` 整份保存可编辑模板：除 `inbounds`、`api`、`stats`
     外所有顶层块都写入 `xray_extra`，与 3x-ui 的 `SaveXraySetting` 一致。
  2. `/panel/api/xray/` 返回生成配置去掉上述面板托管块；高级编辑器只保留
     全部/出站/路由规则三种视图，不再暴露无效的 Inbounds 编辑。
  3. `ensure_policy` 只强制 `statsUserOnline`；level 0 的 uplink/downlink 和
     system 统计开关仅在缺失时补默认 `true`，显式 false 保留。
  4. 两个 `getDefaultJsonConfig` 路由同时接受 GET/POST，返回
     `DEFAULT_EXTRA_JSON`（含 policy 默认值），重置按钮才真正回到默认模板。
  5. 保存后按新 outbounds 清理 `outbound_traffics` 中已删除 tag 的行。
- 被否方案：继续白名单、只补 policy/metrics（未来新增块仍会静默丢失）；
  继续强制所有统计开关（统计页开关永远无效）；在 GET 中保留生成 inbounds
  （用户编辑后保存被丢弃，属于假成功）。
- 后果：模板保存是替换而非合并，页面未返回的键会被删除；`inbounds` 仍由入站页
  和数据库管理，模板内 inbounds 不参与运行时；保存前仍会跑 `xray run -test`。

## D021 入站 3x-ui 元数据落库、导入/批量链接与旧备份恢复
- 日期：2026-11-01
- 背景：入站表单的 shareAddrStrategy/shareAddr/subSortIndex/excludeFromSub/
  disableFlow 之前只展示不保存；`inbounds/import` 未注册，`allLinks` 固定空数组；
  `disableFlow` 不影响生成配置和分享链接；新增列后旧备份 `SELECT *` 恢复会列数不匹配。
- 决策：
  1. 迁移 0008 为 inbounds 增加 5 个元数据列；兼容/原生入站 API 都读写这些列。
  2. `disableFlow` 在生成 VLESS 客户端和分享链接时去掉 flow；前端导出链接同步处理。
  3. 实现 `inbounds/import`：接受 3x-ui 导出的单个 inbound JSON，导入客户端和
     入站累计 up/down；clientStats 流量只加到导入前不存在的邮箱（共享客户端保留
     第一次导入的计数），导入的客户端强制启用；`allLinks` 生成全部启用客户端
     链接并尊重分享地址策略。
  4. 入站列表/详情的 clientStats 改用 `client_inbounds` 关联，不再只看主入站。
  5. 备份恢复按两库共有列复制，新增列使用 schema 默认值。
- 被否方案：把元数据塞进 settings JSON（会被协议 schema 清理）；忽略这些表单字段；
  恢复时要求备份 schema 完全一致（用户升级后无法恢复旧备份）。
- 后果：订阅服务器仍未实现，`subSortIndex`/`excludeFromSub` 只保存，服务端订阅
  输出尚未使用；恢复旧备份时缺失列取默认值而不是报错。

## D022 精简入站/客户端页面的三个操作入口
- 日期：2026-11-01
- 背景：用户要求去掉入站菜单中的「删除所有客户端」「将客户端添加到分组」，
  以及客户端页面的「批量添加客户端」。
- 决策：
  1. 只移除前端入口、对应弹窗组件和 i18n 文案；后端 3x-ui 兼容接口
     （`inbounds/{id}/delAllClients`、`clients/bulkCreate`、`clients/groups/bulkAdd`）
     保留，外部调用方与旧书签接口不受影响。
  2. 分组功能仍可从客户端页和分组页使用；入站页不再提供该批量入口。
- 被否方案：同时删除后端路由（会破坏 3x-ui 兼容调用，且需求只针对界面入口）。
- 后果：`ClientBulkAddModal` 及其测试删除；后端 `clients/bulkCreate` 暂无界面入口。

## D023 客户端导出/导入按 3x-ui 格式实现
- 日期：2026-11-01
- 背景：`GET /panel/api/clients/export` 之前固定返回空数组，`POST /panel/api/clients/import`
  未注册，界面上的导入/导出实际不可用。
- 决策：
  1. 导出返回 `[{client, inboundIds, traffic}]`，`client` 保留完整协议凭据
     （vless/vmess 的 `id`、trojan/ss 的 `password`、http 的 `user`/`pass`、
     `flow`、`subId` 及 vmess 的 `security`/`alterId`），`traffic` 含
     `up`/`down`/`lastOnline`（毫秒）。
  2. 导入接受同一结构，按 `inboundIds` 创建并关联客户端；已存在邮箱跳过并返回
     原因；只对本次新建的客户端恢复流量计数；最后统一触发一次 Xray apply。
  3. 仅支持本面板数据模型：`inboundIds` 为空的客户端无法落库（clients.inbound_id
     非空且有外键），导入时明确跳过，不伪造“无入站客户端”。
- 被否方案：沿用空数组假成功；只导出列表视图字段（会丢失凭据，无法回灌）。
- 后果：3x-ui 的无入站客户端无法往返；真实 Xray 上的导入后运行行为尚未验证。

## D024 移除菜单栏全局搜索（命令面板）
- 日期：2026-11-01
- 背景：用户要求去掉“菜单栏的搜索功能”，即侧边栏的 Command Palette 入口。
- 决策：
  1. 删除侧边栏桌面/抽屉中的搜索按钮、`CommandPalette` 组件、`useCommandPalette`
     store、样式与相关测试；移除 `PanelLayout` 中的挂载。
  2. 移除 14 个 locale 的 `commandPalette.*` 文案。
  3. 保留入站列表和客户端页面内的筛选搜索框（它们不是菜单栏入口）。
- 被否方案：只隐藏按钮但保留 Ctrl+K 监听（用户仍可打开搜索，不符合“去除”）。
- 后果：失去全局跨页搜索/快捷命令；页面内搜索不受影响。

## D025 登录失败微信 Webhook 通知
- 日期：2026-11-01
- 背景：用户要求密码错误时通过微信 webhook 通知真实 IP 和尝试账户；成功登录不通知；
  webhook 地址可在面板设置配置。
- 决策：
  1. 在 `POST /api/login` 和 `POST /login` 的凭据校验失败分支异步发送；限流、
     空凭据、成功登录均不发送。通知不阻塞登录响应。
  2. 配置存于 `x3ui_settings` 的 `loginNotifyWebhook`，复用设置页保存流程；
     保存时校验必须为 `http://` 或 `https://`，空值关闭。
  3. 消息为企业微信群机器人 `{"msgtype":"text","text":{"content":...}}` 格式，
     内容含尝试账户、真实 IP、时间、入口来源；账户名清理控制字符并截断。
  4. “真实 IP”按 `X-Real-IP` → `CF-Connecting-IP` → `X-Forwarded-For` 首项 →
     TCP 对端 的顺序解析。面板通常部署在反向代理后；直接暴露面板时这些头可伪造。
  5. HTTP 客户端用 `ureq` + rustls（静态 musl 可用的纯 Rust TLS 根证书），
     每次失败新建短超时 agent，不保留常驻连接池。
- 被否方案：`reqwest`（体积/依赖更大）；ring 之外的纯 Rust 实验性 TLS provider；
  同步阻塞登录响应；把 webhook 地址放在环境变量（用户要求在面板设置）。
- 后果：Windows 开发构建需要 MinGW gcc（`libsqlite3-sys`/`ring` 的 C 编译）；
  未在真实企业微信接口上验证，只做了本地 HTTP mock 端到端。

## D026 出站订阅（运行时注入 + 每订阅代理）
- 日期：2026-11-01
- 背景：用户反馈“出站订阅功能无效”，要求完善并测试；同时要求更新订阅时可选择
  通过代理更新，以解决面板网络不通的问题。
- 决策：
  1. 新增 `outbound_subscriptions` 表（迁移 0009）与 3x-ui 兼容接口；订阅出站
     **只注入运行时配置**，不写入 `xray_extra`，因此 Xray 页面保存模板不会丢订阅。
  2. 解析器移植 3x-ui 的链接列表订阅语义：base64/纯文本、vmess/vless/trojan/ss/
     hysteria2/wireguard；identity 去 remark，标签按 identity → 同位置 → 前缀+slug
     分配，保证刷新后路由/balancer 引用尽量稳定。
  3. 代理采用**每订阅代理 URL**（用户选择），支持 http(s)/socks4/5(h)，留空直连；
     而不是 3x-ui 的全局“面板出站 + 注入 SOCKS 桥”。代理场景下本地无法解析的
     域名交给代理解析。
  4. 刷新后内容有变化才重启 Xray；新配置必须先通过完整 `xray run -test`，失败时
     回滚订阅列表并记录 `lastError`，绝不覆盖上一个可用列表。
  5. 备份/恢复表清单加入新表；旧备份恢复时清空当前订阅。
- 被否方案：全局面板出站（需要配置注入与设置页，且依赖 Xray 正在运行）；
  只在后台任务里“假成功”不校验配置；把订阅 outbounds 写入模板（保存时会污染）。
- 后果：订阅刷新会短暂重启 Xray；只支持链接列表正文，JSON/Clash 订阅不解析；
  出站标签的“同位置回退”可能让新服务器沿用旧标签。真实 Linux + 真实 Xray 待回归。

## D027 出站页“更多”菜单只保留导入/导出
- 日期：2026-11-01
- 背景：用户要求出站列表的“更多”下拉只保留导出和导入。
- 决策：移除 WARP / NordVPN / PIA 菜单项及 XrayPage 中对应的弹窗挂载和状态；
  `overrides/` 组件源码保留（仍被单元测试引用），但不再打进主页面包。
- 被否方案：只隐藏按钮而保留弹窗挂载（无用代码仍在页面里）。
- 后果：WARP/Nord/PIA 入口从 UI 消失；底层仍属于“明确不做”的集成功能。

## D028 功能整改：实现负载均衡/密钥/更新，移除未实现入口
- 日期：2026-11-02
- 背景：用户逐项要求去除“存在但未实现”的功能，并实现负载均衡、ECH/ML-DSA-65/
  ML-KEM、证书哈希、Xray 版本/Geofiles/GEOdata 更新（支持更新代理）。
- 决策：
  1. 路由测试入口整体移除；负载均衡保留配置编辑，新增 RoutingService gRPC
     实时状态与手动 override（`src/xray/router.rs`，`api.services` 增加
     `RoutingService`）。未持久化 override，Xray 重启后清除，与“配置变更重启”
     模型一致。
  2. 移除 REALITY 目标扫描（含候选扫描弹窗与扫描候选设置）、系统历史、
     面板自更新、客户端在线 IP 日志、客户端 Happ 链接/订阅链接 UI、
     批量调整中的 flow/limitHwid/adTag、不支持的入站协议与 WireGuard。
  3. 实现 ECH/ML-DSA-65/ML-KEM 生成（调用真实 Xray 子命令）与证书 SHA-256；
     证书文件只允许入站或面板设置已引用的路径。
  4. 实现 Xray 版本列表/安装、Geofiles 下载（官方摘要校验、原子替换、重启）
     与 GEOdata 标准源；新增 `updateProxy` 设置，用于 GitHub 与 Geofiles 下载。
  5. 清理未挂载页面（hosts/nodes/groups/api-docs/sub/rawconfig/overrides）、
     对应测试及空/假成功路由；保留仍被活动页面使用的主机标签读取。
- 被否方案：实现 RoutingService.TestRoute 后保留路由测试（用户要求去除）；
  为 GEOdata 另写面板下载器（Xray 原生下载器已能通过 outbound 选代理）；
  保留假成功空接口以避免前端 404（用户要求去除假实现）。
- 后果：真实 Xray 26.10.31 + Windows 已验证负载均衡状态/override、密钥生成、
  证书哈希、版本安装、单个 Geofile 更新；Linux、GEOdata 自动下载与真实公网
  代理认证仍未验证。源码版本仍为 0.2.12，上述改动**未打包**。

## D029 补全 Xray 原生入站/出站协议，sidecar 协议不列入
- 日期：2026-11-03
- 背景：用户要求参考 3x-ui 补全入站/出站协议，且每个协议都能成功添加；
  明确不要只改前端。3x-ui 的 14 种入站中有 3 种依赖随包外部 sidecar。
- 决策：
  1. 入站支持 11 种 Xray 原生协议：vless、vmess、trojan、shadowsocks、http、
     mixed、wireguard、hysteria、tunnel、tun、masque；前端下拉、协议表单、
     后端校验和 `build_config` 全部打通。
  2. 出站下拉暴露 13 种 Xray 原生协议，只隐藏 amneziawg。
  3. mtproto、tuic、amneziawg 入站与 amneziawg 出站在本项目没有可用的
     mtg-multi / tuic-server / amneziawg-go 运行时，本轮按用户确认**不列入**
     UI，后端继续明确拒绝，不返回假成功。
  4. 客户端凭证：wireguard 由后端生成 X25519 keypair 并按入站
     `subnetIp`/`subnetCidr` 分配 `allowedIPs`；hysteria 生成/保留 `auth`；
     masque 生成/保留 `password`。MASQUE 用户邮箱使用原始面板邮箱
     （Xray 拒绝含 `:` 的 email）。
- 被否方案：把 sidecar 协议也列入下拉并仅存库（实际不承载流量，违反项目
  “不伪造成功”规则）；为三种 sidecar 在本轮直接实现完整运行时（工作量与
  可靠性都不现实）。
- 后果：新增依赖 `x25519-dalek`（仅入站客户端建号时使用）。配置结构已按
  Xray 源码字段核对，并用 fake xray 完成面板端到端；真实 `xray run -test`
  与真实流量仍待在装有官方 Xray 的环境验证。hysteria/masque 仍需用户提供
  有效 TLS 证书，否则 `xray -test` 会拒绝（与 3x-ui 一致）。

## D030 面板设置新增“节点地址”，分享链接/二维码不再使用浏览器地址
- 日期：2026-11-03
- 背景：用户从局域网 IP 打开面板时，客户端二维码和分享链接会把该局域网地址写进
  vless/trojan 等链接，客户端无法连接；用户要求用设置里的域名/IP。
- 决策：
  1. 3x-ui 设置页“面板设置”新增 `nodeAddress` 输入（域名或 IP，不带 scheme/path）。
     保存在 `x3ui_settings`，与 `webBasePath` 同一套读写路径。
  2. `setting/defaultSettings` 同时返回 `nodeAddress` 和 `webDomain`（旧字段），
     前端 `useInbounds`/`useClients` 优先取 `nodeAddress`。
  3. 前端 `preferPublicHost` 改为“配置值始终优先”，入站导出链接、入站信息链接、
     客户端二维码全部生效；每个入站的 listen/custom 地址仍优先。
  4. 后端 `public_address()` 用于 `/panel/api/clients/subLinks` 与
     `/panel/api/inbounds/allLinks`，优先级改为 `nodeAddress` → 旧
     `public_address` 设置 → 请求 Host。
  5. 设置页保存时后端校验：拒绝 `://`、`/` 和空格；`setting/all` 在
     `nodeAddress` 缺失时用旧 `public_address` 行回填，升级后无需重填。
- 被否方案：只在前端替换 `window.location.hostname`（客户端二维码走后端
  subLinks，仍会写 Host 头）；直接复用 `webDomain` 而不加新字段（语义是面板
  域名，无法表达独立节点地址）。
- 后果：未配置 `nodeAddress` 时行为与之前一致（回退旧设置/Host/浏览器地址）。
  客户端 QR 对 wireguard/hysteria/masque 仍为空链接（`link.rs` 未实现，见 M3）。

## D031 GitHub 发布、一键安装与面板在线更新
- 日期：2026-11-03
- 背景：用户要求把项目放到 GitHub，提供一键安装脚本、GitHub Actions 编译、
  Releases 下载打包产物，并让面板支持在线更新。
- 决策：
  1. 仓库 `ztoofar/xray-panel-rs`（public）；源码提交，`dist/`、`tmp/`、
     `target/`、`web/dist*`、`node_modules` 不入库。
  2. GitHub Actions：`ci.yml` 在 main/PR 构建前端并跑 fmt/test/clippy；
     `release.yml` 在推送 `v*` 标签时用 Node 26 + cargo-zigbuild（musl）构建，
     生成 tar.gz、原始二进制与 SHA256SUMS，并用 `softprops/action-gh-release`
     发布。
  3. 面板自更新使用 Release 中的**原始二进制** `xray-panel-rs` 与 `SHA256SUMS`，
     不引入解压依赖；校验通过后写同目录临时文件、备份旧二进制为 `.bak`、
     原子 rename 覆盖，再由 systemd 重启。非 systemd 只提示手动重启。
  4. `PANEL_GITHUB_REPO` 编译时注入（CI 为 `${{ github.repository }}`），
     本地默认为 `ztoofar/xray-panel-rs`，方便 fork 后自更新。
  5. 一键安装 `install.sh`：下载最新 Release、校验包内 SHA256SUMS、可选安装
     官方 Xray 核心，然后调用包内 `deploy/install.sh`；`PANEL_VERSION` 可固定
     版本，`SKIP_XRAY=1` 跳过 Xray。
- 被否方案：把二进制提交进仓库（体积/历史膨胀）；在线更新时下载 tar.gz
  （需要新增 tar/gzip 依赖，且不如原始二进制 + SHA256SUMS 简单）。
- 后果：在线更新依赖 GitHub 可达或设置页 `updateProxy`；仅支持 Linux；
  非 systemd 运行方式需要手动重启。

## D032 Release 只发布 tar.gz，在线更新用 GitHub 资产 digest
- 日期：2026-11-03
- 背景：用户要求 Release 资产不要单独放二进制，压缩包里已有；也不必额外发布
  SHA256SUMS 文件。
- 决策：
  1. Release 只上传 `xray-panel-rs-linux-amd64-lumen.tar.gz`；包内仍保留
     `SHA256SUMS` 用于离线校验。
  2. 面板在线更新从 GitHub Releases API 读取该资产的 `digest`
     （`sha256:<hex>`），下载 tar.gz 后按其校验，再用内置最小 tar 解析器解出
     `xray-panel-rs`；不下载额外校验文件。
  3. 新增直接依赖 `flate2`（原本已由 zip 间接引入），不增加新的重量级依赖。
- 被否方案：继续发布原始二进制 + SHA256SUMS（用户明确要求精简资产）；调用系统
  `tar` 解包（依赖宿主机工具，不如内置解析可靠）。
- 后果：**v0.2.16 的在线更新仍按旧的原始二进制逻辑查找资产，无法自动升级到
  0.2.17**；0.2.16 用户需用一键安装脚本升级一次，之后 v0.2.17+ 可正常在线更新。

## D033 一键脚本支持安装/更新与代理，面板更新复用 Xray 更新代理
- 日期：2026-11-03
- 背景：用户要求一键脚本同时支持首次安装和升级、可走代理；面板在线更新也走
  Xray 更新代理，代理不通时必须明确报错。
- 决策：
  1. `install.sh` 检测 `/usr/local/bin/xray-panel-rs`：存在则打印更新模式，仍由
     `deploy/install.sh` 停服务、替换二进制并保留 DB/`/etc/xray-panel.env`。
  2. 新增 `PANEL_PROXY`（支持 `http(s)://`、`socks4/5(h)://`），未设置时回退
     `HTTPS_PROXY`/`ALL_PROXY`；所有面板与 Xray 下载统一走 `download()`，失败时
     输出包含代理地址的明确错误。
  3. 面板在线更新的 check/install 均使用设置页 `updateProxy`（与 Xray 更新同一
     地址）；`panel_update` 在错误链中加入 `via update proxy <addr>`，前端直接
     显示；`updates::agent` 对非法代理 URL 也带地址报错。
- 被否方案：脚本单独维护另一个代理变量而不复用面板设置（用户明确要求同一地址）；
  代理失败只显示 curl 原始错误（无法区分网络/代理问题）。
- 后果：`PANEL_PROXY` 只影响脚本下载，不写入面板设置；面板运行期更新仍读
  `updateProxy`。安装脚本未设置代理时行为不变。

## D034 在线更新重启判定：PANEL_SYSTEMD + MainPID
- 日期：2026-11-03
- 背景：集成测试发现 GitHub Actions 继承 `INVOCATION_ID`，面板误判为 systemd
  托管并 `exit(1)`，实际不会被拉起。
- 决策：`deploy/xray-panel.service` 显式设置 `Environment=PANEL_SYSTEMD=1`；
  更新器优先检查该变量。旧单元没有该变量时，仅在 `INVOCATION_ID` 存在且
  `systemctl show -p MainPID --value xray-panel` 等于当前进程 PID 时才自动退出。
- 后果：旧版单元仍可自动重启；在 systemd 会话里手动启动的进程不会误退出。

## D035 一键脚本按版本幂等跳过，二进制暴露 LUMEN_VERSION
- 日期：2026-11-04
- 背景：用户发现已安装最新版后重跑一键脚本仍会下载、停止面板并覆盖二进制。
- 决策：
  1. 脚本解析 `releases/latest` 跳转的 `Location` 得到目标版本，请求同样走
     `PANEL_PROXY`；
  2. 新版本二进制支持 `--version` / `-V`，输出
     `xray-panel-rs <ver> (LUMEN_VERSION=<ver>)`；脚本先 `grep -a` 检查
     `LUMEN_VERSION=` 再执行，避免用旧二进制探测时误启动服务或执行迁移；
  3. 版本一致时直接退出，不下载、不停服务；`PANEL_FORCE=1` 绕过；
  4. 没有 marker 的旧二进制（0.2.18 及更早）下载 Release tar.gz 后，按包内
     二进制 SHA-256 与已安装文件比对，一致时同样跳过；
  5. 无论是否跳过面板重装，`/usr/local/bin/xray` 缺失时仍安装 Xray 核心。
- 被否方案：直接执行旧二进制探测版本（可能启动服务/执行迁移）；只比较 GitHub
  tag 与日志（依赖日志权限与时间窗口）；跳过时完全不做 Xray 检查（破坏修复场景）。
- 后果：0.2.19 起重复执行脚本可零下载跳过；0.2.18 及更早需一次 5.4 MB 下载后
  按哈希跳过。跳过路径不会重写 systemd 单元，修复单元需 `PANEL_FORCE=1`。

## D036 更新/下载失败时给出可操作的代理提示
- 日期：2026-11-04
- 背景：用户实测 `updateProxy=http://127.0.0.1:10813`，面板检查更新只显示
  `io: Connection refused (os error 111)`，无法判断 `127.0.0.1` 指面板服务器自身。
- 决策：`updates::network_hint` 先按 `io::ErrorKind` 与 `ureq::Error` 变体识别
  传输失败，再用本地化文本（`os error 111`/`10061` 等）兜底；`panel_update`
  check/install、`xray_versions`、`install_xray`、Geofiles 的错误链前加提示：
  有代理时说明代理需从面板主机可达、`127.0.0.1` 是面板服务器自身；无代理时
  提示检查网络或配置代理。原始 URL、代理地址与系统错误保留在链尾。
- 被否方案：只匹配英文 `Connection refused`（Windows 本地化文本不匹配）；只返回
  简化提示（丢失排障信息）；把提示写死在前端（其他入口无提示）。
- 后果：错误信息更长，但可直接定位代理配置问题；代理可达但目标不可达时提示
  可能把原因归到代理（见 `KNOWN-ISSUES.md` N6）。

## D037 代理/网络类错误对用户只返回中文提示
- 日期：2026-11-04
- 背景：用户实测 `updateProxy=http://127.0.0.1:10813`，界面显示
  `io: Connection refused (os error 111)`；同时 `127.0.0.1` 指面板服务器自身
  的语义不清晰。
- 决策：传输层失败（按 `io::ErrorKind` / `ureq::Error` 类型识别，辅以
  `os error 111`/`10061` 文本兜底）在 `updates.rs` 统一转换为中文消息；
  `with_network_hint` **替换**错误链而不是追加 context，原生链只写
  `tracing::warn`。出站订阅代理复用同一识别与“代理不可用”消息。
  非网络类错误保持原错误链。
- 后果：前端不再显示原生 io/ureq 错误；排障需要查看面板日志。非网络类错误
  仍可能是英文（`KNOWN-ISSUES.md` N7）。

## D038 后端面向用户错误统一中文，Xray 原生输出保留
- 日期：2026-11-04
- 背景：兼容 API 大量返回英文前缀 + 原生错误链，中文面板中出现英文报错；
  Xray 的 `run -test` / 子命令输出本身是本面板无法控制的第三方文本。
- 决策：`fail()`、`AppError`、`bail!`、`anyhow!`、`context` 中由本面板产生的
  面向用户文案统一改为中文；嵌入的 Xray 原生 stdout/stderr、系统 `io`/`ureq`
  原始错误仍保留在链尾或只写日志。
- 被否方案：后端多语言资源包（14 个 locale 维护成本高，且当前面板实际以中文
  运维）；前端字符串映射（无法覆盖所有错误来源）。
- 后果：英文界面下后端错误仍为中文（`KNOWN-ISSUES.md` H4）；后续新增接口
  必须用中文文案。

## D039 面板日志固定 UTC+8 并持久化按日文件
- 日期：2026-11-04
- 背景：tracing 默认时间按主机时区/UTC 显示，用户看到的日志时间不是中国时间；
  内存环形缓冲只有 500 行，重启即丢，无法查看历史。
- 决策：新增 `panel_log::ChinaTime` 固定 UTC+8 格式化所有 fmt layer；新增按日
  文件 writer，默认 `<DB 目录>/panel-logs/panel-YYYY-MM-DD.log`，可用
  `PANEL_LOG_DIR` 覆盖，保留 14 天；文件/缓冲层记录 target、文件、行号。
  实时视图仍读内存缓冲，历史视图列出文件并 tail 读取；历史文件名只接受
  `panel-YYYY-MM-DD.log`，防止路径穿越。
- 被否方案：写 journald（依赖部署方式）；单文件无限增长（磁盘不可控）；
  按大小轮转（跨天查找不直观）。
- 后果：日志时间与主机时区无关；磁盘最多约 14 个文件；进程重启后实时视图
  仍是空的，但历史视图可读旧文件。

## D040 出站真实连接检测与微信通知
- 日期：2026-11-04
- 背景：用户需要定期确认出站真实可用，并在不可用时收到通知；微信 webhook
  已用于登录失败提醒。
- 决策：新增高级设置页与 `outbound_check` 后台 job。每次检测复用出站页
  “真实模式”的临时 Xray SOCKS + HTTP 204 探测，不触碰正在运行的 Xray；
  选中的 tag 支持单个或多个（含订阅出站），频率 1–1440 分钟。不可用/恢复
  经企业微信群机器人发送；同一故障每 30 分钟最多重复一次。微信 webhook
  文案改为默认通知方式。
- 被否方案：直接在主 Xray 进程内探测（需要额外入站/路由，可能干扰线上流量）；
  每个周期都无条件重复通知（噪音过大）；引入 Telegram/Discord 通知（当前
  范围只要求微信）。
- 后果：每次检测会短暂启动一个 Xray 进程（低内存目标下可接受）；webhook
  未配置时只记录日志/UI 提示，不发通知；检测结果保存在内存，面板重启后清空。

## D041 设置保存不重启，重启面板显式触发
- 日期：2026-11-04
- 背景：`setting/update` 收到前端回传的 `xrayTemplateConfig` 时会调用
  `service::apply`，导致每次保存常规设置都重启 Xray；同时“重启面板”按钮在
  保存后因无未保存改动而禁用。
- 决策：设置保存接口完全忽略 `xrayTemplateConfig`，只持久化；Xray 模板保存
  继续由 Xray 页 `/panel/api/xray/update` 负责；“重启面板”按钮始终可用，
  只有点击后才调用 `restartPanel`。需要重启后生效的设置由页面提示说明。
- 后果：保存常规设置不再中断 Xray 连接；运行环境/端口等改动必须点“重启面板”
  才生效；测试通过对比 Xray `startedAt` 与服务进程存活验证。

## D042 客户端位置查询按需、服务端代理
- 日期：2026-11-04
- 背景：客户端详情需要显示在线 IP 并可选查询归属地；浏览器直接调用第三方
  接口会遇到 CORS，且默认查询会产生不必要的外部请求和隐私暴露。
- 决策：在线 IP 来自 Xray `GetUsersStats`（离线客户端不渲染 IP 行；多个 IP
  全部显示）；位置按钮逐个 IP 触发 `POST /panel/api/clients/ipLocation`，
  后端固定访问 ipwho.is（失败回退 ipapi.co），前端默认不请求。
- 后果：第三方服务不可达时返回中文错误，不影响面板其他功能；Xray OnlineMap
  丢弃回环地址，本机来源客户端不会显示 IP（`KNOWN-ISSUES.md` H5）。

## D043 位置查询改用纯真库（zxinc）优先
- 日期：2026-11-04
- 背景：D042 使用 ipwho.is 作为主数据源，但国际库对国内 IP 的省市不准。
  实测 `115.193.109.37`（浙江杭州电信）被 ipwho.is 定位为“江苏南京”，
  与 tool.lu 的纯真/淘宝库结果不符；ipapi.co 回退还经常返回 `RateLimited`。
- 决策：主数据源改为 zxinc 的免费 JSON API（基于纯真库，无需 key），
  失败时回退 ipwho.is；移除 ipapi.co 回退。解析 zxinc 的
  `data.country`（`–`/Tab 分隔的层级）与 `data.local`（运营商），
  多出的层级并入城市。
- 后果：国内 IP 的省/市/运营商更准确；zxinc 不提供时区与经纬度，
  `timezone` 为空字符串、`latitude`/`longitude` 为 `null`（前端未使用）；单次查询
  通常 1–5 秒，面板无外网或 zxinc 不可达时仍回退到 ipwho.is。

## D044 IP数据云作为可选第二数据源，独立设置页
- 日期：2026-11-04
- 背景：用户要求显示 tool.lu 页面上的“高精归属地定位（IP数据云）”字段。
  该数据来自 `api.ipdatacloud.com/v2/query`，必须携带用户自己的 API Key；
  直接调用返回 `key不能为空`。tool.lu 的查询接口返回 204，且页面明确禁止
  作为 API 使用，不能抓取。
- 决策：设置页新增独立的“IP数据云”页（`/settings#ipdata`），与“高级”页的
  出站连接检测分开；管理员填写 `ipDataCloudKey` 后，`ipLocation` 在查询纯真
  的同时并行查询 IP数据云，结果放在 `ipDataCloud`/`ipDataCloudError` 字段，
  前端位置结果显示“纯真”和“IP数据云”两行。Key 保存在
  `settings.x3ui_settings`，按字符串处理（纯数字 Key 不会被转成数字）。
- 后果：未配置 Key 时行为与 D043 完全一致；IP数据云失败不影响纯真结果；
  接口返回的区县/街道精度与额度取决于用户套餐；Key 对管理员可见（单管理员
  面板，与现有微信 Webhook 同等对待）。

## D045 每次代码修改都递增版本号
- 日期：2026-11-04
- 背景：用户明确要求“以后只要有修改版本号都增加”。此前规则只在正式打包时
  bump，源码在不同开发阶段可能共用同一个版本号，难以区分。
- 决策：每次代码/功能修改都递增 `Cargo.toml` 的 `version` 并同步
  `Cargo.lock`，即使不打包也 bump；纯文档/记忆同步不单独 bump。正式发布
  仍由用户确认后推送 `v*` tag，由 GitHub Actions 打包。
- 后果：源码版本号变成开发进度计数器，不再等于已发布版本；`CLAUDE.md`
  规则 3、会话结束清单和 `memory/NEXT-STEPS.md` 同步更新。本次将
  `0.2.21` → `0.2.22`。

## D046 Release 页面只显示本次更新内容
- 日期：2026-11-04
- 背景：用户要求 Release 页面“只显示更新内容就行了”。此前
  `.github/workflows/release.yml` 的 body 含一键安装、代理、在线更新和资产说明，
  并开启 `generate_release_notes`，页面较长。
- 决策：Release body 只保留“X.Y.Z 更新内容”列表；不再写入安装/更新说明，
  也关闭自动生成的 Full Changelog。安装与更新说明继续放在 README、`install.sh`
  和面板界面中。每次发布前更新 workflow body 的版本号和更新条目。
- 后果：Release 页面更简洁；发布前必须人工更新 body；v0.2.22 的历史说明
  保存在 `.github/release-notes/v0.2.22.md`。

## D047 Release 更新内容标题不带版本号
- 日期：2026-11-04
- 背景：用户要求 Release 页面的“更新内容”标题前面不要显示版本号。
- 决策：Release body 标题固定为 `## 更新内容`，版本由 Release 标题/标签
  （`v0.2.22`）体现；workflow body 与 `.github/release-notes/v0.2.22.md`
  同步修改。
- 后果：后续发布只更新标题下的条目，不再写 `## X.Y.Z 更新内容`。

## D048 IP数据云并入“高级”标签页
- 日期：2026-11-04
- 背景：用户要求把 IP数据云 放进“高级”菜单，与原有出站检测在同一页面
  切换，样式和“常规设置”页一致；D044 的独立侧栏页面不符合该交互。
- 决策：设置侧栏只保留“常规 / 安全设定 / 高级”；`/settings#advanced`
  使用 AntD Tabs，分为“出站检测”（原 `AdvancedTab`）和“IP数据云”
  （`IpDataCloudTab`）；移除 `/settings#ipdata` 独立入口。
- 后果：IP数据云不再单独出现在侧栏；旧的 `#ipdata` 链接会回落到“常规”页；
  版本 `0.2.22` → `0.2.23`（未打包/未打 tag）。

## D049 流量计量采用 3x-ui 模型：不重置 Xray 计数器，后端计算速率
- 日期：2026-11-05
- 背景：0.2.7 的实现会在手动/定时/出站重置时调用
  `QueryStats(reset=true)` 清零 Xray 计数器，并在 3x-ui 兼容层忽略重置失败后
  照样清零数据库。重置失败、超时或与采集并发时，下一轮 `current > 0 / last = 0`
  会把整段累计值再计一遍。速度则依赖前端对两次列表快照做差分：全部空闲时
  列表数据不变，effect 不执行，速度会永久停留在最后一次值。
- 决策：
  1. 重置只清数据库累计值（`up/down`），保留 `last_up/last_down` 基准，永不
     重置 Xray 计数器。删除了 `needs_reconcile` 的写入与读取；旧列保留兼容。
  2. 流量 job 在每次成功采样时，用本轮增量和单调时钟测得的真实间隔计算
     每个入站 tag、每个客户端邮箱的字节/秒，写入 `AppState.traffic_speeds`；
     空闲实体没有条目。入站 slim 列表返回 `upSpeed/downSpeed`，客户端列表在
     `traffic` 内返回同名字段，前端直接显示，不再自行差分。
  3. MASQUE 只接受裸邮箱，客户端同时关联 MASQUE 与普通入站时，额外按裸邮箱
     统计并归到 MASQUE attachment，避免被 per-attachment 路径吞掉。
- 后果：重置语义与 3x-ui 一致（边界误差 ≤ 一个采集周期）；速率是真实采样
  窗口的平均值，慢 tick/漏采不会产生倍率偏差，空闲时速度在一个周期内清空。
  入站与客户端计数器口径仍不同（代理层 vs 用户负载），不要求逐字节相等。
  版本 `0.2.23` → `0.2.24`（未打包/未打 tag）。

## D050 恢复 3x-ui 的实时流量 WebSocket 推送
- 日期：2026-11-05
- 背景：0.2.24 把速度改成后端快照 + REST 轮询下发。实测无关联客户端的入站
  速度虽能由入站计数器产生，但用户仍反馈实时速度不对：REST 轮询与后端采集
  周期存在相位差，一次轮询可能错过某个采集窗口；3x-ui 则用 WebSocket 每轮
  推送增量，浏览器收到后立即换算。
- 决策：
  1. 后端新增会话保护的 `/ws`（base path 由 guard 重写），流量 job 每轮广播
     3x-ui 同款 `{type:"traffic", payload:{traffics, clientTraffics,
     onlineClients, onlineByGuid, activeInbounds, lastOnlineMap,
     clientTrafficIntervalMs}, time}`。
  2. 前端重新启用 `useWebSocket`，用帧内的真实采集间隔换算字节/秒；缺失的
     入站/客户端按空闲清空速度。统计 RPC 失败时广播空增量，避免旧速度停留。
  3. 保留 0.2.24 的后端 `upSpeed/downSpeed` REST 字段作为 WS 不可用时的
     兜底；客户端速度合并时 WS 覆盖 REST 同邮箱值，避免重复相加。
  4. 慢客户端由 `broadcast` 的 `Lagged` 跳过旧帧；帧内 `lastOnlineMap` 只带
     本轮在线客户端，控制负载大小。
- 后果：实时速度与 3x-ui 一致，不再依赖 REST 轮询相位；无客户端入站的
  入站计数器速度也能实时显示。版本 `0.2.24` → `0.2.25`（发布 v0.2.25）。

## D051 实时 WS 流量帧按需构建并压缩内存占用
- 日期：2026-11-05
- 背景：0.2.25 恢复 `/ws` 后，流量 job 每轮都收集并序列化完整 traffic 帧，
  即使没有任何浏览器订阅；每帧用 `serde_json::Value` 为每个条目建树，广播
  缓冲保留 8 帧；WS 连接没有 ping/空闲/发送超时。用户反馈面板内存一直很高。
- 决策：
  1. tick 开头读取 `ws_events.receiver_count()`；为 0 时不收集
     `ws_traffics`/`ws_client_traffics`，`broadcast_traffic` 入口再次短路。
  2. 增量改为借用 `&str` 的 `InboundTraffic`/`ClientTraffic` 结构体，
     整个 `{type,payload,time}` 用 `serde_json::to_string` 一次写出，不建
     `Value` 树，避免每条的 Map/String 分配。
  3. 广播容量 8 → 2：慢客户端由 `Lagged` 跳到最新帧。
  4. `/ws` 每 30 秒 Ping，90 秒无入站帧断开，发送 10 秒超时；浏览器自动回
     Pong，正常页面不受影响。
- 后果：面板页未打开时不再产生帧内存；打开时 WS 路径的峰值/常驻内存随活跃
  客户端数明显下降（3000 客户端实测末值 28.8 → 27.8 MiB，峰值 34.7 → 33.7
  MiB），协议字段与 0.2.25 完全一致。版本 0.2.25 → 0.2.26。

## D052 WS 流量帧的除数改为真实采样间隔
- 日期：2026-11-05
- 背景：0.2.25/0.2.26 的 `traffic` 帧把 `clientTrafficIntervalMs` 固定设为
  配置的轮询秒数（`poll_secs × 1000`），但帧里的增量是相对上一轮**成功采样**
  的累计计数器差分。tick 耗时超过轮询周期、统计 RPC 超时后恢复、或设置刚
  改变时，增量实际跨越的窗口比配置值长；前端用配置值做除数会把实时速度按
  比例放大。0.2.24 的 REST `upSpeed/downSpeed` 用真实间隔计算，所以同一时刻
  WS 显示与 REST 显示不一致。另：面板/Xray 刚启动后的第一轮没有上一采样
  时间点，增量（自 Xray 启动以来的全部计数）仍被当成一个轮询窗口的速率广播，
  产生一次性尖峰。
- 决策：
  1. `tick` 把本轮 `elapsed_ms`（单调时钟，上一成功采样到本轮采样的真实
     间隔）写入帧的 `clientTrafficIntervalMs`；前端无需改动即可得到与 REST
     相同的字节/秒。
  2. 没有上一采样（`elapsed_ms <= 0`）时，累计流量照常计费，但该轮不向 WS
     帧放入入站/客户端增量（`rated_interval_ms` 返回 `None`），浏览器收到
     空增量后清空速度，下一轮起显示真实速率。
  3. Xray 在采样期间重启时也广播一帧空增量，立即清掉上一进程留下的速度。
  4. 帧内间隔字段在无速率可播时回退为配置的轮询值，保持 3x-ui 客户端对
     `clientTrafficIntervalMs` 的既有假设。
- 后果：WS 实时速度与 REST 快照、真实传输速率使用同一个分母，不再随 tick
  延迟按比例虚高。回归：慢 StatsService（人为延迟 3s、2s 轮询）下修复前
  1.0 MB/s 显示为 1.5 MB/s，修复后为 1.00 MB/s；fake/真实 Xray/无头
  Chrome 回归全部通过。版本 0.2.26 → 0.2.27。

## D053 入站 WS 速度增加与客户端页一致的过期保护
- 日期：2026-11-05
- 背景：0.2.25 起 WS 实时速度没有过期兜底的缺陷只修了一半：`useClients.ts`
  的客户端速度有「超过 2×采样间隔未收到帧即清空并回退 REST」的计时器，
  而 `useInbounds.ts` 的 `wsInboundSpeed` 一旦 WS 半途断开（公网抖动、反代
  掐长连接、睡眠恢复），就把断开瞬间的速率永久冻结在入站速度列里——空闲时
  表现为旧速度不归零，随后又与真实速率脱节。被合并的 REST `upSpeed` 兜底值
  被冻结的 WS 项永久覆盖（合并顺序 `{...server, ...ws}`）。
- 决策：
  1. 入站页 `applyTrafficEvent` 处理完每帧后，重置“2×帧间隔（上限 2 分钟）”
     的过期计时器（版本计数门闩，防止旧计时器误清新帧）。
  2. 计时器触发时清空整个 `wsInboundSpeed`：合并逻辑随即回退到每轮都在
     刷新的 REST `upSpeed/downSpeed`（与 WS 同一后端速率源，不会静止）。
  3. 组件卸载时清理计时器。语义与客户端页的 `clientSpeedExpiryTimers` 一致。
- 备选：在 WS 的 `disconnected` 事件里直接清空——更即时，但需要把连接事件
  接入页面 handler 链路且无法覆盖“边上边断”的半开连接；帧计时器对两种松动
  都成立，也被客户端页验证过，故先对齐既有模式。
- 后果：WS 断开时入站速度列最迟 2×采样间隔后回退 REST 速率并随轮询持续
  刷新；WS 恢复后下一帧立即接管。正常连接期间行为不变（每帧都重置计时）。
  版本 0.2.27 → 0.2.28（已实现，未在真实部署上验证；本地自测见
  `DEVELOPMENT-LOG.md`）。

## D054 按 3x-ui 模型重写实时流量取数（64 MiB 上限 / 分族查询 / 内存基线）
- 日期：2026-11-05
- 背景：v0.2.28 部署后用户仍报「大流量时面板没有任何速度显示」。逐行对照
  上游 3x-ui 的取数实现（`internal/xray/api.go` 的 `GetTraffic`）发现三项
  结构性差异：**(a)** 上游把 gRPC 接收上限从默认 4 MiB 提到 64 MiB，注释
  明说 ~45k 客户端的统计回复可单独达到 4 MiB，超限则**整轮失败**——本面板
  `StatsClient` 从未设置该上限，与大流量面板速度全空的症状吻合；**(b)**
  上游按 `user>>>` / `inbound>>>` / `outbound>>>` 分三次 QueryStats，单族
  失败只记 warning、其余族照常计数，本面板单次 `query("")` 任一失败则整轮
  无数据；**(c)** 上游把显示增量基线放在内存（`StatsLastValues`），每次
  成功查询即推进、与 DB 完全解耦，本面板显示增量复用 DB `last_up/last_down`
  （只在计费事务成功后推进），DB 写失败会连带清掉速度列与 WS 帧。
- 决策（对齐上游语义，Rust 重写）：
  1. `StatsClient::connect` 显式 `max_decoding_message_size(64 MiB)`
     （`MAX_STATS_REPLY_BYTES = 64 << 20`，对端是本机回环上的自家核心）。
  2. 新增 `query_families()`：三个族各一次 QueryStats（`regexp:false`、
     前缀过滤），返回 `Vec<FamilyStats>`（族 + stats + 可选 error）；仅在
     **全部**族失败时返回 Err 走既有 tick 失败路径，部分失败由 tick 记
     debug 后用其余族继续本轮。
  3. 新内存基线引擎 `display_deltas`（3x-ui `StatsLastValues` +
     `statsBaselined` 语义）：族首过只记录基线不发增量；未见过或回退的
     计数器从 0 起算（不掉流量）；族失败保留基线、下轮成功按跨度补算；
     仅在全部族成功且 map > 2×live 时剪枝（热路径零分配）。WS 帧
     `traffics`/`clientTraffics`、`AppState.traffic_speeds`、在线/活跃
     判定改用该引擎增量；**DB 计费路径（`apply_tick`、`add_traffic`、
     配额/到期/重置执行）保持原状不动**，计费正确性不受影响。
  4. 低内存约束（硬性）：基线 map 键用紧凑 `DeltaKey{kind, uplink, key}`
     而非完整 stat 名（省去 ~30 字节固定文本/条目）；只记录 traffic 计数
     器；`2×live` 阈值剪枝防膨胀；map 大小以**现存 traffic 计数器**为上界
     （≈ 每 (attachment, 方向) 一条，几十字节/条）。WS 帧/`traffic_speeds`
     的分配结构沿用 0.2.26 的优化（无订阅者不建帧）。
- 备选：仅修 (a) 不动结构——最小改动，但 (b)(c) 是上游已被生产验证的
  鲁棒性来源，且用户明确要求「把 3x-ui 的代码用 Rust 重写」，故按完整
  结构移植；显示与计费保持解耦（DB 基线仍为计费路径，与上游把 DB 写入
  放在 `submitTrafficWrite` 序列化事务的思路一致）。
- 后果：大客户端/大流量面板不再因单回复超 4 MiB 或单族失败而整体无速度；
  显示速度与 DB 写成败解耦。前端 WS 帧协议与 0.2.25 完全一致（字段、间隔、
  语义），无需前端改动。版本 0.2.28 → 0.2.29（已经 `cargo test` 119/119、
  clippy 零警告、fmt；**未在真实 Linux/大面板部署上验证**，见 G13）。

## D055 采样与计费彻底解耦：串行计费 writer（合并槽）
- 日期：2026-11-06
- 背景：v0.2.29 后用户仍报 tunnel 入站实时速度不准，对照确认「3x-ui 显示
  比本面板大很多」（累计量两边都正确）。逐行对照 3x-ui：其速度列 = 本轮
  增量 ÷ **硬编码 5 秒**（`TRAFFIC_POLL_INTERVAL_S`），且它的 DB 计费走独立
  的串行 writer（`submitTrafficWrite`，`internal/web/service/traffic_writer.go`），
  统计 job 节拍不被写库拖。本面板 0.2.29 的内存基线引擎虽然正确，但 tick
  仍是「采样 → 逐客户端/入站计费写 → 广播」串行：`traffic_poll_secs=2` 的
  大部署里一轮 DB 写可达十几秒，两次采样间隔被拖到 15–30 秒，速度列 =
  本轮字节 ÷ 整个积压时长 = 十几秒平均——远小于 3x-ui，也远小于网卡监控
  的瞬时值。广播还排在全部计费写之后（一处 `?` 即整帧丢失）。
- 决策：显示路径先跑完再碰数据库。tick 变为：分族采样 → `display_deltas` →
  写 `traffic_speeds`、推进 `last_stats_at`、刷新在线/活跃、广播 WS 帧；
  随后把本轮**原始计数器**打包成 `BillingSample` 推入 `BillingSink`
  （单槽合并队列，`Arc<Mutex<Option<_>>>` + `Notify`：写者落后时新样本直接
  取代旧样本——原始计数器天然包含历史，合并不重复计费也不掉字节，内存上界
  = 一个样本），由独立 tokio 任务 `run_billing_writer` 串行执行
  `apply_billing`：重读 clients/inbounds，attachment 计费与 DB 基准比较、
  配额/到期/limitIp 禁用、`reset_days` 重置、出站表、`last_online` 心跳、
  全局历史。`apply_billing` 失败只记 warn，显示不受牵连。epoch 防护：样本
  携带采样时的 `start_epoch`，核心已重启则整份丢弃（否则死进程的累计会对着
  清零后的基准重复计费）；心跳持久化移出广播路径（广播不再被 DB 写挡住）。
  在线/活跃的「本轮有流量」信号改用显示增量（计费写失败也认在线，与 0.2.29
  的 D054 语义一致）。
- 备选：照抄 3x-ui 除以固定周期——会在轮询拖长时虚高（0.2.26 的 D 案已验证
  是错的）；缩短采样窗口到固定 1–2 秒另开一条高频轻量查询——大客户端面板
  每秒一次全量统计回复代价太高（64 MiB 上限正是为此而设）。真实平均 +
  节拍不受积压影响是两者中唯一同时正确的。
- 低内存约束：新增常驻内存仅一个合并槽（约一份统计回复的 map 拷贝）；writer
  内 DB 读与旧路径同量级；无新增每轮分配。
- 后果：`traffic_poll_secs` 较小时显示节拍不再被计费尾工稀释；tunnel 与所有
  入站的实时速度回归到真实平均速率。计费最多落后一轮显示（3x-ui 同构）。
  版本 0.2.29 → 0.2.30（`cargo test` 121/121、clippy 零警告、fmt；本地
  真实 Xray tunnel 与 fake-stats 两条 WS 链路回归 PASS；**未在真实 Linux/
  大面板部署上验证**，见 G14）。

## D056 显示 1 秒采样、计费仍按轮询：实时速度对齐 1 秒监控
- 日期：2026-11-06
- 背景：0.2.30 部署后用户仍报「入站和客户端实时速度显示不准」。现场四方
  对照（Xray 原始计数器 / WS 帧 / REST / DB）显示：同一窗口的字节增量与
  `增量 ÷ 真实间隔` 完全一致，计费累计也对得上；但面板只有每
  `traffic_poll_secs`（该部署 5 秒）采样一次，速度列因此每 5 秒才换一次，
  且显示的是过去 5 秒的平均值。对比 1 秒刷新的网卡监控时，爬坡/结束/突发
  跨窗口会差几十个百分点（本地 4 MiB/s 限速下载实测窗口边界可读 3.22 /
  4.00 / 2.33 MB/s，而真实值稳定 4.00 MB/s）。这不是字节丢失，是显示窗口
  太长；此前 0.2.29/0.2.30 修的是取数与计费节拍，未改显示窗口。
- 决策：
  1. 显示 tick 与计费 tick 分离。有浏览器持有 `/ws`（`receiver_count > 0`）
     且客户端数 ≤ 2000 时，显示 tick 为 1 秒；否则回到
     `traffic_poll_secs`。大面板保持原轮询，避免每秒 3 次可能达几十 MB 的
     统计回复。
  2. 每个显示 tick 仍查三个族、推进内存基线、写 `traffic_speeds`、广播
     WS 帧；`clientTrafficIntervalMs` 是真实 1 秒间隔，浏览器除以它得到
     字节/秒。REST `upSpeed/downSpeed` 也按 1 秒窗口更新，WS 断开时兜底。
  3. 计费、在线 API、`last_online` 心跳、roster（clients/inbounds/身份映射）
     仍只在 `traffic_poll_secs` 的计费 tick 执行。显示 tick 复用 roster，
     并把 1 秒增量累计的移动邮箱/活跃入站 tag 在计费 tick 一次性交给
     writer，避免 DB 写入频率随显示频率上升。
  4. 显示 tick 的移动邮箱/活跃 tag 会累积到计费 tick；统计失败轮保留累积，
     下一次成功计费再落库。
- 被否方案：把 `traffic_poll_secs` 默认值改成 1–2 秒——只是让用户改设置，
  且计费/在线/心跳也会跟着变快；固定按 5 秒除数——慢 tick 时会虚高
  （D052 已否）；继续 5 秒窗口——与 1 秒监控的差距不会消失。
- 后果：速度列按 1 秒刷新，突发/爬坡窗口误差从「最多约一个轮询周期」降到
  约 1 秒；DB 写入频率不变。代价是 ≤2000 客户端的面板多约 3 次/秒
  `QueryStats`；>2000 客户端仍按配置轮询。WS 帧从每 5 秒一帧变为每秒一帧，
  前端过期计时器（2×帧间隔）仍成立。
- 版本 0.2.30 → 0.2.31（`cargo test` 121/121、clippy 零警告、fmt；
  fake-stats 计费 13/13、无头 Chrome UI+WS 10/10、真实 Xray tunnel 回归
  PASS；**未在真实 Linux/大面板部署上验证**）。

## D057 实时速度改为原生绝对速率快照；无人查看时不计算速度，计费始终运行
- 日期：2026-11-06
- 背景：用户明确要求「重新设计入站和客户端的速度显示，不要参考 3x-ui，
  用原生代码实现；流量统计要准确；实时速度只在打开面板时显示，没打开面板
  就不要统计速度，但流量要一直统计」。0.2.31 仍沿用 3x-ui 的
  `traffics`/`clientTraffics` 增量 + `clientTrafficIntervalMs` 除数协议：
  前端要自己差分/换算，且没有订阅者时后端仍会走完整显示路径。
- 决策：
  1. `/ws` 的 `traffic` 帧改为原生快照：
     `{inbounds: {tag: {up, down}}, clients: {email: {up, down}}, ...,
     intervalMs, ttlMs}`。`up/down` 是服务端已按真实采样间隔算好的
     **字节/秒**，浏览器直接渲染，不再差分、不再除以任何字段。
  2. 「打开面板」的判定就是 `/ws` 订阅者数（`ws_events.receiver_count() > 0`）。
     没有订阅者时不换算速率、不建帧、不序列化 JSON，`traffic_speeds` 置空；
     REST 的 `upSpeed/downSpeed` 因此同样为 0。有订阅者且客户端 ≤2000 时
     显示节拍为 1 秒，否则按 `traffic_poll_secs`。
  3. 流量计费完全不变：DB 基准、`BillingSink` 串行 writer、在线/配额/重置
     仍按 `traffic_poll_secs` 常驻运行，与是否有人查看面板无关。
     `display_deltas` 每个 tick 仍推进内存基线（防止 1 秒 tick 重复计同一段
     字节），只是速率换算受 `watching` 控制。
  4. `ttlMs = clamp(2×intervalMs + 1000, 3000, 120000)` 下发给浏览器；超过
     该时间没有新帧就清空速度列，替代 3x-ui 的固定过期假设。前端收到快照
     直接替换整张速度表，缺失的实体即空闲，不再跨帧累加。
- 被否方案：保留 3x-ui 字段以兼容旧前端——用户明确要求不参考 3x-ui，且
  增量+除数把换算责任放在浏览器，慢 tick/漏帧时容易再次失真；无人查看
  时仍采样速度——浪费 `QueryStats` 与内存，且与本需求冲突；用 REST 轮询
  是否有请求判断“面板打开”——轮询频率不稳定，WS 订阅者是更准确的信号。
- 后果：REST 速度字段只在有 WS 订阅者时有值（已写入 API 文档，e2e 显式
  验证“关闭面板时速度为 0、累计流量照常”）；inbound/client 速度口径统一
  为服务端字节/秒；3x-ui 的 `traffics`/`clientTraffics`/
  `clientTrafficIntervalMs` 从帧中移除，诊断脚本同步更新。
- 版本 0.2.31 → 0.2.32（`cargo test` 122/122、clippy 零警告、fmt；
  fake-stats 原生 e2e 18/18、零客户端入站 4/4、无头 Chrome UI+WS 12/12、
  `diag.py` 稳态均值 +0.10%；**未在真实 Linux/大面板部署上验证**）。

## D058 面板启动 Xray 时默认关闭 freedom splice，换取连续的下行计数
- 日期：2026-11-06
- 背景：用户现场反馈 tunnel 协议入站下载时实时下行一直 0 B/s，下载结束后
  突然出现一次很大的速率（如 17 MB/s）；VLESS 正常。读 Xray-core 源码确认：
  `tunnel` 就是 `dokodemo-door`（`infra/conf/xray.go`），`dokodemo.go` 设置
  `inbound.CanSpliceCopy = 1`，`freedom` 出站设置 `ob.CanSpliceCopy = 1`；
  Linux/Android 上 `proxy.CopyRawConnIfExist` 命中后用 `TCPConn.ReadFrom`
  做零拷贝，**整条响应的字节数只在连接结束时才 `writeCounter.Add(w)`**
  （`proxy/proxy.go`）。因此下载期间入站下行计数器不动，结束时一次性入账；
  若 Xray 在传输中途被杀，这部分字节永远不进统计。上传方向仍走
  `buf.Copy` 按块计数，所以用户只看到下行异常。
- 决策：面板在 `src/xray/supervisor.rs` 启动 Xray 子进程时设置
  `XRAY_BUF_SPLICE=disable`（Xray `platform.UseFreedomSplice`，值不是
  `auto`/`enable` 即关闭）。若运维已在环境变量 `XRAY_BUF_SPLICE` 或
  `xray.buf.splice` 中显式设置，则不覆盖。这样 tunnel/视线 Vision 的直连
  流量按每次读写计入，实时速度与计费都连续。
- 被否方案：
  1. 只改显示（把一次性增量摊到更长窗口）——传输期间仍然没有字节可分摊，
     实时速度依旧为 0；且进程被杀时计费丢字节的问题没解决。
  2. 只在存在 tunnel 入站时关闭 splice——Vision 直连同样走 splice，问题相同；
     行为随入站变化也难解释。
  3. 升级/修改 Xray 核心——面板不能改核心；`XRAY_BUF_SPLICE` 是官方开关。
- 后果：direct 转发（tunnel 与 Vision 的直连段）从内核零拷贝变为
  readv+write，CPU 与内存带宽略升；换取连续、可计费的统计。需要极限吞吐的
  部署可显式设置 `XRAY_BUF_SPLICE=enable` 恢复零拷贝（此时 tunnel 下行速度
  仍会一次性出现）。
- 版本 0.2.32 → 0.2.33（`cargo test` 123/123、clippy 零警告、fmt；
  **Windows 上 splice 不生效，实际效果待真实 Linux 部署验证**）。

## D059 保留 Xray splice 零拷贝，tunnel 实时速度改由内核 socket 计数提供
- 日期：2026-11-06
- 背景：D058 用 `XRAY_BUF_SPLICE=disable` 换连续计数，代价是 direct 转发
  失去内核零拷贝。用户要求**既保留零拷贝、又实时显示 tunnel 速度**，并明确
  “不改 Xray”（不打补丁、不换自建二进制）。
- 被否方案：修改 Xray 的 `CopyRawConnIfExist`，用 `io.LimitedReader` 驱动
  `TCPConn.ReadFrom` 分块返回（现代 Go 的 `net/spliceFrom` 会把
  `LimitedReader` 的剩余长度作为 splice 上限），从而零拷贝与逐块计数兼得。
  技术上可行，但需要维护自建 Xray，和面板“下载官方 Xray、随官方版本更新”
  的机制冲突，用户否决。
- 决策：
  1. `src/xray/supervisor.rs` 不再注入 `XRAY_BUF_SPLICE`，恢复 Xray 默认的
     freedom splice（零拷贝）；运维仍可显式设为 `disable`。
  2. 新增 `src/net_diag.rs`：Linux `NETLINK_SOCK_DIAG` / `INET_DIAG_INFO`
     只读 dump 所有 TCP socket 的 `tcpi_bytes_acked`（下行）与
     `tcpi_bytes_received`（上行）。splice 由内核搬字节，socket 计数在 ACK
     时持续增长，不依赖 Xray 何时记账。
  3. 只有存在 `/ws` 订阅者时采样；每 socket 以内核 cookie 为键做基线差分，
     首次见到只建基线（避免把连接历史一次性算进一秒）。
  4. 速率按“本地地址:端口”归属到 **tunnel（dokodemo）** 入站。tunnel 的
     每条连接都走 splice，且没有客户端，按端口归属足够。
     `http`/`mixed` 只在 CONNECT 上 splice，仍用 Xray 计数（见 G15）。
  5. 内核速率只驱动显示与入站活跃判定；**计费仍用 Xray 计数**。
- 已知代价/限制：
  - 两次采样之间建立并关闭的连接看不到，这类短连接不计入显示速率；长时间
    传输（用户报告的 17MB/s 场景）完整可见。
  - 只能按监听地址/端口归属，无法按用户归属（tunnel 无用户，不受影响）。
  - 仅 Linux、仅 TCP；dump 失败（权限/内核不支持）时退回 Xray 计数，并
    限频告警（每 5 分钟一次）。
  - 计费仍是 Xray 口径：splice 下入站累计在连接结束时入账，核心在传输中
    被杀会丢该段字节。
- 版本 0.2.33 → 0.2.34（`cargo test` 132/132、clippy 零警告、fmt、
  `cargo zigbuild --release --target x86_64-unknown-linux-musl` 通过；
  **Windows 没有 sock_diag，实际效果未验证**，见 TODO T32）。
