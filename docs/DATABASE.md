# 数据库设计（DATABASE）

> 引擎：SQLite（sqlx，无 ORM）。迁移目录：`migrations/`，启动时由
> `sqlx::migrate!` 自动执行。最后核对：2026-11-01。

## 1. 连接与运行参数

代码：`src/db/mod.rs`

| 参数 | 值 | 原因 |
| --- | --- | --- |
| journal_mode | WAL | 读写并发、崩溃恢复 |
| synchronous | NORMAL | 低内存 VPS 上兼顾性能与安全 |
| foreign_keys | ON | 级联删除客户端/会话 |
| cache_size | `-2000`（约 2 MiB） | 低内存目标 |
| temp_store | MEMORY | 加速排序/临时表（内存由查询规模决定） |
| 连接池 | max 2 / min 1，acquire timeout 5s | SQLite 写串行，小池省内存 |
| busy_timeout | 5s | 短暂写锁竞争 |

默认数据库路径由 `PANEL_DB` 指定（默认 `xray-panel.db`）。WAL 模式会同时产生
`.db-wal`、`.db-shm` 文件；备份使用 `VACUUM INTO` 生成单文件快照。

时间单位：

- 表内 `created_at`、`updated_at`、`last_online`、`last_reset_time`、
  `last_traffic_reset_time` 等均为 **Unix 秒**。
- 入站/客户端的 `expiry_time` 由 3x-ui 前端以**毫秒**写入；历史数据可能是秒。
  代码在比较/排序时用“值 < 100000000000 视为秒”统一处理（见 `jobs.rs`
  `expired_at`、`compat.rs` `expiry_ms`）。
- 兼容 API 返回客户端 `lastOnline`/`createdAt`/`updatedAt` 时乘 1000 转毫秒。

## 2. 表结构

### users
| 列 | 类型 | 说明 |
| --- | --- | --- |
| id | INTEGER PK AUTOINCREMENT | |
| username | TEXT NOT NULL UNIQUE | 当前只允许单管理员 |
| password_hash | TEXT NOT NULL | Argon2id PHC 字符串 |
| created_at | INTEGER NOT NULL | 秒 |

### sessions
| 列 | 类型 | 说明 |
| --- | --- | --- |
| token_hash | TEXT PK | `SHA256(token)`，不存明文 token |
| user_id | INTEGER FK users(id) ON DELETE CASCADE | |
| csrf | TEXT NOT NULL | 会话 CSRF token |
| created_at | INTEGER NOT NULL | 秒 |
| expires_at | INTEGER NOT NULL | 秒；索引 `idx_sessions_expires` |

维护任务每 15 分钟删除过期会话。

### settings（键值表）
| 列 | 类型 | 说明 |
| --- | --- | --- |
| key | TEXT PK | |
| value | TEXT NOT NULL | |

已知键：

| key | 形态 | 用途 |
| --- | --- | --- |
| `x3ui_settings` | JSON 对象字符串 | 3x-ui 设置页保存的全部字段（含 `webBasePath`、`webListen`、`webPort`、`sessionMaxAge`、`pageSize`、`secureCookie`、Xray 路径等） |
| `xray_extra` | JSON 对象字符串 | Xray 模板：log/dns/outbounds/routing 等，面板生成配置时合并并强制覆盖关键块 |
| `traffic_poll_secs` | 数字字符串 | 流量/在线轮询周期（2–300，默认 5），job 直接读取 |
| `public_address` | 字符串 | 分享链接使用的外部地址（可为空） |
| `panel_name` | 字符串 | 种子值，当前 UI 使用 Lumen 品牌 |

> 运行时设置读取优先级：数据库中的 `x3ui_settings` 覆盖启动环境变量；但
> `PANEL_DB`、`PANEL_LOG`、`PANEL_WORKERS` 只在环境变量中生效。

### inbounds
| 列 | 类型 | 说明 |
| --- | --- | --- |
| id | INTEGER PK AUTOINCREMENT | |
| user_id | INTEGER NOT NULL DEFAULT 1 | 预留多用户字段，当前单管理员 |
| up / down | INTEGER NOT NULL DEFAULT 0 | 累计字节 |
| last_up / last_down | INTEGER NOT NULL DEFAULT 0 | 上次采集的 Xray 累计值，用于算增量；不对外序列化 |
| total | INTEGER NOT NULL DEFAULT 0 | 总流量配额，0=不限 |
| remark | TEXT NOT NULL DEFAULT '' | 备注 |
| sub_sort_index | INTEGER NOT NULL DEFAULT 1 | 0008；订阅输出排序（0 归一化为 1） |
| exclude_from_sub | INTEGER NOT NULL DEFAULT 0 | 0008；布尔，订阅输出时排除 |
| enable | INTEGER NOT NULL DEFAULT 1 | 布尔 |
| expiry_time | INTEGER NOT NULL DEFAULT 0 | Unix 时间；UI 写毫秒、旧数据可能是秒（代码统一归一化），0=不过期 |
| traffic_reset | TEXT NOT NULL DEFAULT 'never' | `never/hourly/daily/weekly/monthly` |
| traffic_reset_day | INTEGER NOT NULL DEFAULT 1 | 月重置日 |
| last_traffic_reset_time | INTEGER NOT NULL DEFAULT 0 | 秒 |
| listen | TEXT NOT NULL DEFAULT '' | 监听地址 |
| port | INTEGER NOT NULL | |
| protocol | TEXT NOT NULL | vless/vmess/trojan/shadowsocks/http/mixed/wireguard 等 |
| settings | TEXT NOT NULL DEFAULT '{}' | 协议 JSON（客户端注入前） |
| stream_settings | TEXT NOT NULL DEFAULT '{}' | 传输/TLS/REALITY JSON |
| sniffing | TEXT NOT NULL DEFAULT '{}' | 嗅探 JSON |
| tag | TEXT NOT NULL UNIQUE | Xray 入站 tag，也是统计标识的一部分 |
| share_addr_strategy | TEXT NOT NULL DEFAULT 'node' | 0008；分享链接地址策略 `node/listen/custom` |
| share_addr | TEXT NOT NULL DEFAULT '' | 0008；`custom` 策略使用的主机名 |
| disable_flow | INTEGER NOT NULL DEFAULT 0 | 0008；布尔，生成配置和分享链接时去掉 client flow |
| created_at / updated_at | INTEGER NOT NULL | 秒 |

### clients
| 列 | 类型 | 说明 |
| --- | --- | --- |
| id | INTEGER PK AUTOINCREMENT | |
| inbound_id | INTEGER FK inbounds(id) ON DELETE CASCADE | 主入站（兼容 3x-ui 的单一归属） |
| email | TEXT NOT NULL UNIQUE | 客户端唯一标识，也是统计邮箱 |
| enable | INTEGER NOT NULL DEFAULT 1 | |
| up / down | INTEGER NOT NULL DEFAULT 0 | 累计字节（跨全部 attachment 汇总） |
| last_up / last_down | INTEGER NOT NULL DEFAULT 0 | 旧版聚合基准；0.2.7 起仅用于裸邮箱外部配置与 MASQUE 裸邮箱计量，面板生成的常规统计按 `client_inbound_traffic` 逐 attachment 计算 |
| total | INTEGER NOT NULL DEFAULT 0 | 配额，0=不限 |
| expiry_time | INTEGER NOT NULL DEFAULT 0 | Unix 时间；UI 以毫秒写，旧数据可能是秒，代码统一归一化 |
| settings | TEXT NOT NULL DEFAULT '{}' | 协议凭据 JSON + `subId` |
| comment | TEXT NOT NULL DEFAULT '' | 0004 |
| group_name | TEXT NOT NULL DEFAULT '' | 0004 |
| limit_ip | INTEGER NOT NULL DEFAULT 0 | 0=不限；通过在线 IP 数近似执行 |
| limit_hwid | INTEGER NOT NULL DEFAULT 0 | 只保存/展示，未强制 |
| reset_days | INTEGER NOT NULL DEFAULT 0 | 按天自动重置流量 |
| last_reset_time | INTEGER NOT NULL DEFAULT 0 | 秒 |
| last_online | INTEGER NOT NULL DEFAULT 0 | 0006；秒，API 返回毫秒 |
| created_at / updated_at | INTEGER NOT NULL | 秒 |

索引：`idx_clients_inbound(inbound_id)`。

### client_inbounds（多对多）
| 列 | 说明 |
| --- | --- |
| client_id | FK clients(id) ON DELETE CASCADE |
| inbound_id | FK inbounds(id) ON DELETE CASCADE |

主键 `(client_id, inbound_id)`，索引 `idx_client_inbounds_inbound(inbound_id)`。
0003 迁移把每个客户端原有的 `inbound_id` 关系回填到此表。一个客户端可关联多个
入站；Xray 中每个关联生成独立统计身份 `<inbound_id>:<email>`。

### client_inbound_traffic（per-attachment 统计基准）
0007 新增。每个 `(client_id, inbound_id)` 保存独立的上次累计值，修复“多入站
共用一个聚合基准，单个 attachment 重置会污染其他 attachment 增量”的问题。

| 列 | 类型 | 说明 |
| --- | --- | --- |
| client_id | FK clients(id) ON DELETE CASCADE | 主键之一 |
| inbound_id | FK inbounds(id) ON DELETE CASCADE | 主键之一 |
| last_up / last_down | INTEGER NOT NULL DEFAULT 0 | 该 attachment 上次采集值；**重置流量时不清零** |
| needs_reconcile | INTEGER NOT NULL DEFAULT 0 | 0.2.24 起不再写入（保留列兼容旧库）；重置不再触碰 Xray 计数器，因此无需 reconcile |
| updated_at | INTEGER NOT NULL DEFAULT 0 | 秒 |

主键 `(client_id, inbound_id)`，索引 `idx_client_inbound_traffic_inbound(inbound_id)`。
迁移回填已有 `client_inbounds`，不修改任何累计流量。新增关联不需要触发器：
统计任务遇到缺失的行时按基准 0 处理并 upsert。关联删除后遗留的行不会被读取
（统计只遍历当前关联），客户端/入站删除时由外键级联清理。

### outbound_traffics
| 列 | 类型 | 说明 |
| --- | --- | --- |
| tag | TEXT PK | 出站 tag |
| up / down | INTEGER NOT NULL DEFAULT 0 | 累计字节 |
| last_up / last_down | INTEGER NOT NULL DEFAULT 0 | 上次累计；重置流量时不清零 |
| updated_at | INTEGER NOT NULL DEFAULT 0 | 秒 |

### outbound_subscriptions
| 列 | 类型 | 说明 |
| --- | --- | --- |
| id | INTEGER PK | 自增 |
| remark / url | TEXT | 备注与订阅 URL |
| enabled | INTEGER | 0/1 |
| allow_private / allow_insecure | INTEGER | SSRF 白名单与 TLS 校验开关 |
| user_agent / proxy_url | TEXT | 拉取请求头与可选代理（http/socks） |
| tag_prefix | TEXT | 标签前缀；空值回退 `sub<id>-` |
| update_interval | INTEGER | 秒，最小 60 |
| priority | INTEGER | 注入顺序（越小越前） |
| prepend | INTEGER | 0=手动出站之后，1=之前 |
| last_updated | INTEGER | 上次成功拉取（秒） |
| last_error | TEXT | 最近一次失败原因 |
| last_fetched_outbounds | TEXT | JSON 数组，运行时注入用；列表接口不返回 |
| link_identities | TEXT | JSON 对象 identity→tag，标签稳定用；不返回 |
| created_at / updated_at | INTEGER | 秒 |

订阅出站永远不写入 `xray_extra`，每次生成运行时配置时合并到 `outbounds`。

### api_tokens
0001 迁移创建（id、name、token_hash、created_at），当前 Rust 面板**没有 API token
功能**，仅在备份/恢复的表清单中出现。不要基于它假设存在对外 API 鉴权。

## 3. 迁移历史（append-only）

| 文件 | 内容 |
| --- | --- |
| `0001_init.sql` | users、sessions、settings、inbounds、clients、api_tokens |
| `0002_outbound_traffic.sql` | outbound_traffics |
| `0003_client_inbounds.sql` | 客户端-入站多对多表并回填 |
| `0004_client_extras.sql` | comment、group_name、limit_ip、limit_hwid、reset_days、last_reset_time |
| `0005_client_sub_id.sql` | 为旧客户端补 `settings.subId`（随机 16 hex） |
| `0006_client_last_online.sql` | clients.last_online |
| `0007_client_inbound_traffic.sql` | per-attachment 统计基准表并回填现有关联 |
| `0008_inbound_meta.sql` | inbounds 增加 sub_sort_index、exclude_from_sub、share_addr_strategy、share_addr、disable_flow |
| `0009_outbound_subscriptions.sql` | 出站订阅表（URL、代理、间隔、排序、上次结果/错误） |

新增迁移时：编号递增、只追加、不要修改历史迁移；同步更新本文件与
`docs/ARCHITECTURE.md` 的相关描述。恢复旧备份时按共有列复制，新增列使用
schema 默认值（2026-11-01 起），因此 0007 及更早的备份仍可导入。

## 4. 数据不变量

1. `clients.email` 全局唯一；`inbounds.tag` 全局唯一。
2. 每个客户端至少有一条 `client_inbounds` 关系；读取时若无关联则回退到
   `clients.inbound_id`。
3. 重置流量只清零 `clients.up/down`、`inbounds.up/down`、
   `outbound_traffics.up/down`，**绝不重置 Xray 计数器，也不清零
   `last_up/last_down` 基准**（与 3x-ui 一致）。后续采样只计基准之后的新字节，
   因此重置 RPC 失败、超时或与采集并发都不会把整段累计值再计一次。
4. 面板生成的配置按 attachment 计量，增量基准以 `client_inbound_traffic` 为准；
   `clients.last_up/last_down` 只服务裸邮箱外部配置（以及 MASQUE 的裸邮箱计量），
   不再代表聚合基准。Xray 新进程启动时两者一并清零，累计值不动。
5. `last_up/last_down`、`needs_reconcile` 是内部字段，不通过 API 序列化；
   `needs_reconcile` 自 0.2.24 起仅为旧库兼容列。
6. 删除入站会级联删除客户端（外键）及 `client_inbounds` / `client_inbound_traffic`
   关系。

## 5. 备份与恢复

- 下载：`VACUUM INTO` 临时文件，返回 `application/octet-stream`。
- 恢复：上传必须是 SQLite 文件头；在事务内按固定顺序清空/插入
  `client_inbound_traffic, outbound_subscriptions, outbound_traffics, sessions,
  clients, inbounds, api_tokens, users, settings`；
  要求备份至少包含 `users/settings/inbounds/clients` 四张表。恢复后重启 Xray。
  旧备份缺少 `outbound_subscriptions` 时会清空当前订阅，避免混入备份外的数据。
- 恢复不会自动迁移 schema；备份与当前版本不兼容时会失败并回滚。
