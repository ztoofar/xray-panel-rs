# Lumen Panel (xray-panel-rs)

Lumen 是一款面向 Linux amd64 的轻量级 Xray 控制面板。后端基于 Rust、Axum 与
SQLite，前端基于 React 与 Ant Design（3x-ui 前端裁剪）；面板以子进程方式托管
本机 Xray，并负责生成、校验和应用 Xray 配置。发布产物为静态 musl 单二进制，
前端资源在编译期嵌入，运行时不依赖 Node.js。

- 仓库：<https://github.com/ztoofar/xray-panel-rs>
- 发布：<https://github.com/ztoofar/xray-panel-rs/releases>
- 许可证：MIT

## 主要功能

- 七个页面：系统状态、入站、客户端、出站、路由、Xray、设置；表单覆盖传输、
  TLS/REALITY、sockopt、fallbacks、嗅探、balancer 等字段，无需手写 JSON。
- 入站支持 11 种 Xray 原生协议（VLESS、VMess、Trojan、Shadowsocks、HTTP、
  Mixed、WireGuard、Hysteria、Tunnel、TUN、MASQUE），出站支持 13 种。
- 客户端凭据生成、多入站关联、分组与备注、批量操作、流量/到期限制；在线状态
  与最后在线时间基于 `GetUsersStats` 与访问日志。
- 配置生成 → `xray run -test` → 备份 → 原子替换 → 重启，校验失败不覆盖线上配置。
- gRPC StatsService 流量统计，支持小时/天/周/月重置与客户端独立重置。
- 出站流量统计、DNS/路由/日志配置、负载均衡实时状态与手动切换。
- 出站订阅：拉取分享链接作为只读出站，支持定时刷新与 HTTP/SOCKS5 代理。
- 面板在线更新、Xray 版本切换、Geofiles 更新与更新代理。
- REALITY x25519、ML-DSA-65、ML-KEM-768、TLS ECH 生成与证书 SHA-256 计算。
- 节点地址统一用于分享链接与二维码；SQLite 备份/恢复、日志查看、周期性内存回收。

## 环境要求

- Linux x86_64
- systemd（推荐；一键安装脚本使用，也可手动部署）
- Xray 核心；安装脚本在缺失时自动安装官方版本
- 源码构建：Rust stable、C 编译器（SQLite 使用 bundled）、Node.js 26+ 与 npm

## 安装

### 一键安装（需 root）

```sh
curl -fsSL https://raw.githubusercontent.com/ztoofar/xray-panel-rs/main/install.sh \
  | sudo bash
```

脚本行为：

- 下载最新 Release，校验包内 `SHA256SUMS` 后安装面板。
- 检测到已有安装时进入更新模式，保留数据库与 `/etc/xray-panel.env`。
- 已是最新版本时跳过重装，且不会停止面板服务；`PANEL_FORCE=1` 强制重装。
- `/usr/local/bin/xray` 不存在时安装官方 Xray 核心；`SKIP_XRAY=1` 跳过。

可选参数：

| 变量 | 说明 |
| --- | --- |
| `PANEL_VERSION` | 安装指定版本，例如 `0.2.22`；默认最新版本 |
| `PANEL_REPO` | 发布仓库，默认 `ztoofar/xray-panel-rs` |
| `PANEL_PROXY` | 下载代理，支持 `http://` 与 `socks5h://`；未设置时回退 `HTTPS_PROXY` / `ALL_PROXY` |
| `PANEL_FORCE` | 设为 `1` 时即使已是最新版本也重新安装 |
| `SKIP_XRAY` | 设为 `1` 时跳过 Xray 核心安装 |
| `XRAY_VERSION` | 指定 Xray 版本，默认最新版本 |

示例：

```sh
curl -fsSL https://raw.githubusercontent.com/ztoofar/xray-panel-rs/main/install.sh \
  | sudo PANEL_VERSION=0.2.22 PANEL_PROXY=http://127.0.0.1:7890 bash
```

`PANEL_PROXY` 仅作用于安装脚本内部的下载；获取 `install.sh` 本身的 `curl`
需要单独配置代理。代理不可达时，脚本与面板会返回包含代理地址的错误。

### 手动安装

```sh
sudo install -d -m 0750 /etc/xray-panel
sudo install -d -m 0750 /usr/local/etc/xray
sudo install -d -m 0750 /usr/local/share/xray
sudo install -m 0755 target/release/xray-panel-rs /usr/local/bin/xray-panel-rs
sudo cp deploy/xray-panel.env.example /etc/xray-panel.env
sudo cp deploy/xray-panel.service /etc/systemd/system/xray-panel.service
sudo chmod 600 /etc/xray-panel.env
sudo systemctl daemon-reload
sudo systemctl enable --now xray-panel
```

首次启动创建默认账号 **admin / admin**。登录后应立即修改密码，并按需调整
`/etc/xray-panel.env`。`/api/health` 可用于健康检查。

## 更新

### 面板在线更新

登录面板，进入「设置 → 面板更新」，使用「检查更新」与「立即更新」。面板从
GitHub Releases 下载 tar.gz，使用 GitHub 为资产计算的 SHA-256 digest 校验，
解出二进制后原子替换；systemd 部署会自动重启，非 systemd 部署返回
`restartRequired` 并提示手动重启。

在线更新使用「设置 → 运行环境 → 更新代理」（`updateProxy`）。`127.0.0.1`
指向面板服务器自身，而非操作浏览器所在的电脑。

### 通过安装脚本更新

重复执行一键安装脚本即可。脚本会保留数据库与配置文件；若已是最新版本则直接
跳过并给出提示，不停止面板服务。需要强制重装或修复 systemd 单元时使用：

```sh
... | sudo PANEL_FORCE=1 bash
```

## 配置

### 环境变量

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `PANEL_LISTEN` | `0.0.0.0:2053` | 首次启动的监听地址；之后以面板设置保存值为准 |
| `PANEL_DB` | `xray-panel.db` | SQLite 数据库路径 |
| `XRAY_BIN` | `/usr/local/bin/xray` | Xray 可执行文件 |
| `XRAY_CONFIG` | `/usr/local/etc/xray/config.json` | 面板生成的 Xray 配置路径 |
| `XRAY_API_ADDR` | `127.0.0.1:10085` | Xray StatsService gRPC 地址 |
| `PANEL_XRAY_LOG` | 与 `PANEL_DB` 同目录的 `xray.log` | Xray stdout/stderr 日志 |
| `PANEL_LOG_DIR` | 与 `PANEL_DB` 同目录的 `panel-logs/` | 面板日志按日文件目录（保留 14 天） |
| `PANEL_USERNAME` | `admin` | 首次启动创建的管理员用户名 |
| `PANEL_PASSWORD` | `admin` | 首次启动密码；之后以数据库为准 |
| `PANEL_WORKERS` | `2` | Tokio 工作线程数 |
| `PANEL_LOG` | `info` | 日志级别 |
| `PANEL_SECURE_COOKIE` | `false` | 仅 HTTPS 反向代理后设为 `true` |
| `XRAY_BUF_SPLICE` | 不设置（Xray 默认开启 splice） | Xray 的 freedom splice 开关。面板不注入此变量，默认保留零拷贝；因此 Linux 上 tunnel 下载的实时下行会在连接结束时一次性入账。显式设为 `disable`（或 `xray.buf.splice=disable`）可关闭零拷贝、改为按每次读写连续计数（0.2.36 已回到 0.2.32 行为） |

`PANEL_DB`、`PANEL_LOG`、`PANEL_WORKERS` 仅从环境变量读取。监听地址、面板
路径、Xray 路径、日志路径与安全 Cookie 可在面板设置中覆盖，重启后生效；面板
路径（`webBasePath`）保存后立即生效。

### 面板设置

- 常规：监听地址/端口、URI 路径、节点地址、会话有效期、分页大小、微信
  Webhook（默认通知方式，用于登录失败提醒和出站检测告警）、面板在线更新。
- 高级：出站真实连接检测（单个/多个出站、1–1440 分钟频率、不可用通知、
  最近结果）。
- 运行环境：Xray 二进制/配置/API/日志路径、更新代理。
- 安全：管理员凭据。

保存设置不会重启面板或 Xray；需要重启时使用设置页顶部的“重启面板”按钮。
面板日志固定使用中国时间（UTC+8），可在仪表盘日志弹窗中查看实时日志或最近
14 天的历史文件。

节点地址用于分享链接与客户端二维码，避免写入浏览器访问面板时使用的局域网
地址。

## 部署与安全

- 面板仅提供 HTTP，不管理 TLS 证书。公网部署应置于可信的 HTTPS 反向代理之后，
  并设置 `PANEL_SECURE_COOKIE=true`；或使用防火墙限制来源、通过 SSH 隧道访问：

  ```sh
  ssh -L 2053:127.0.0.1:2053 user@server
  ```

- 首次登录后立即修改默认密码，并限制面板端口访问来源。
- Xray 由面板作为子进程启动和监管。不要再通过 systemd 启动同一个 Xray，
  否则 API 端口与入站端口会冲突。
- Xray 配置由面板生成并托管，请通过页面修改，不要直接编辑生成文件。

## 资源占用

默认 2 个 Tokio 工作线程；SQLite 连接池上限 2、页缓存约 2 MiB，WAL +
`synchronous=NORMAL`，不使用 ORM；流量统计复用常驻 gRPC 连接。流量轮询默认
5 秒（2–300 秒可调），每 15 分钟清理过期会话并收缩内存。Release 构建启用
`opt-level="z"`、LTO 与 strip。

## 构建

前端必须在 Rust 编译前完成构建，`rust-embed` 会在编译期把 `web/dist/`
嵌入二进制。

```sh
cd frontend-3xui
npm install
npm run build          # 输出到 ../web/dist

cd ..
cargo build --release  # 产物 target/release/xray-panel-rs
```

Linux musl 交叉编译：

```sh
cargo zigbuild --release --target x86_64-unknown-linux-musl
```

发布默认由 GitHub Actions 完成。同步 `Cargo.toml` 版本后推送 `v*` 标签，
`.github/workflows/release.yml` 会自动构建并上传 tar.gz：

```sh
git tag v0.2.22
git push origin v0.2.22
```

标签版本必须与 `Cargo.toml` 一致。每个 Release 仅提供一个资产：
`xray-panel-rs-linux-amd64-lumen.tar.gz`，包含静态二进制、systemd 单元、
安装脚本、README 与包内 `SHA256SUMS`。

## 已知限制

- Shadowsocks 传统加密方式仅支持单客户端（Xray 限制），推荐使用
  2022-blake3；同一客户端关联多个 Shadowsocks 入站时应使用相同加密方式与密码。
- HTTP/Mixed 协议没有标准分享链接。
- MTProto、AmneziaWG、TUIC 依赖外部 sidecar，本面板不内置，入站表单不提供；
  WARP/Nord/PIA 等集成同样不提供。
- Hysteria 与 MASQUE 必须配置有效的 TLS 证书，否则 Xray 配置校验失败。
- 3x-ui 兼容 API 尚未覆盖 2FA、邮件、订阅服务器、节点、HWID 日志等高级接口；
  未实现的操作返回明确错误，不伪造成功。路由测试、REALITY 扫描、系统历史、
  客户端 IP 日志等入口已移除。
- 前端不使用 WebSocket，状态通过轮询与操作后刷新获取。
- `limitIp` 通过在线 IP 数近似执行；`limitHwid` 仅保存，不强制。
- 在线状态依赖 Xray 26+ 的 `statsUserOnline` 策略与 `GetUsersStats` RPC。
- 客户端变更通过重新生成配置并重启 Xray 生效，会短暂中断连接。
- Xray 版本与 Geofiles 更新会停止并重启 Xray；下载按官方 SHA-256 校验，
  失败不替换。
- `.db` 恢复要求备份与当前 schema 兼容，不兼容的备份会回滚并报错。
- 仅支持单管理员账号。
- 面板异常崩溃（非正常停止）时可能残留 Xray 子进程；建议使用 systemd 部署，
  其默认 KillMode 会随服务停止子进程。

## 许可证

MIT，见 [LICENSE](LICENSE)。
