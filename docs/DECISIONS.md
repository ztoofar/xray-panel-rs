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
