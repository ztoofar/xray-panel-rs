# Lumen Panel (xray-panel-rs)

面向 Linux amd64 的**低内存** Xray 单机控制面板。Rust + Axum 后端，React 前端，
SQLite 存储，面板直接管理本机 Xray 子进程。

## 功能

- 管理员登录（Argon2id）、会话 Cookie、CSRF 保护、登录限流
- **入站/出站/客户端/路由/Xray 配置页面沿用成熟面板的页面与表单**，
  接到本面板的 Rust 后端；另含系统状态与面板设置（常规/安全）
- 出站连接测试：TCP 直连探测，以及通过临时 Xray 实例 + 本地 SOCKS
  入站的 HTTP / 真实延迟隧道测试
- 入站/出站/客户端/路由表单的协议子项、传输、TLS/REALITY、sockopt、
  fallbacks、嗅探、balancer 等字段与 3x-ui 页面一致，不需要手写 JSON
- 客户端凭据（UUID / 密码 / 账号）在表单内生成；支持一个客户端关联多个入站、
  备注、分组、IP/HWID 字段、批量操作
- 在线客户端：通过 Xray `GetUsersStats` 获取真实在线状态
- Xray 配置生成、`xray run -test` 校验、子进程监管、异常自动重启
- 通过 Xray gRPC StatsService 统计流量，周期重置（每小时/天/周/月），
  并支持客户端独立按天重置
- 出站流量统计、DNS / 路由 / 日志配置；负载均衡实时状态与手动切换（RoutingService）
- 出站订阅：从订阅 URL 拉取分享链接并作为只读出站注入运行时配置；支持定时更新、
  标签稳定、HTTP/SOCKS5 代理拉取
- 入站密钥工具：REALITY x25519、ML-DSA-65、ML-KEM-768、TLS ECH，以及从证书
  计算 SHA-256 并填入 `pinnedPeerCertSha256`
- 面板设置可配置**节点地址**（域名或 IP）；分享链接和客户端二维码始终使用该
  地址，不会把浏览器打开的局域网地址写进链接
- Xray 版本切换、Geofiles 更新、GEOdata 源配置；支持在设置页配置 HTTP/SOCKS5
  更新代理（用于 GitHub/Geofiles 下载）
- 面板在线更新：设置页从 GitHub Releases 下载压缩包，用 GitHub 资产 digest
  校验后解出二进制并原子替换，systemd 部署自动重启；GitHub Actions 在 `v*`
  标签自动构建发布
- SQLite `.db` 文件备份与恢复（自定义 `/settings` 页保留；面板设置页也可备份/恢复）
- Lumen 品牌与青蓝主题（#0EA5E9），默认浅色，支持深色/极暗主题
- 静态前端嵌入二进制，无 Node 运行时依赖

明确不做：

- 多节点管理、Telegram/Discord Bot、邮件通知
- 订阅服务器（raw / JSON / Clash）
- MTProto、AmneziaWG、TUIC（依赖外部 sidecar）、WARP/Nord/PIA
- 广告/赞助、API 文档
- **SSL 证书安装/管理**（面板只跑 HTTP）
- **IP 连接数限制**（在线状态与在线 IP 展示已支持）

## 下载与安装

GitHub Releases：<https://github.com/ztoofar/xray-panel-rs/releases>

### 一键安装（Linux x86_64，需 root）

```sh
curl -fsSL https://raw.githubusercontent.com/ztoofar/xray-panel-rs/main/install.sh | sudo bash
```

脚本会下载最新 Release、校验 SHA256、安装面板；`/usr/local/bin/xray` 不存在时
自动安装官方 Xray 核心（`SKIP_XRAY=1` 可跳过）。指定版本或仓库：

```sh
curl -fsSL https://raw.githubusercontent.com/ztoofar/xray-panel-rs/main/install.sh \
  | sudo PANEL_VERSION=0.2.16 PANEL_REPO=owner/repo bash
```

### 在线更新

登录面板 →「设置」→「面板更新」→「检查更新」/「立即更新」。面板从 GitHub
Releases 下载 tar.gz、用资产 digest 校验并解出二进制，原子替换后由 systemd
自动重启；非 systemd 部署会
提示手动重启。

## 环境要求

- Linux amd64
- 可用的 Xray 二进制（自行安装/替换；面板不会下载或更新它）
- 构建需要：Rust stable、C 编译器（SQLite 使用 bundled 编译）、Node 22+ / npm
  （构建自研前端时需要）

## 构建

```sh
cd frontend-3xui
npm install
npm run build          # 输出到 ../web/dist

cd ..
cargo build --release  # 产物 target/release/xray-panel-rs
```

前端必须先构建，`rust-embed` 会在编译时嵌入 `web/dist/`。当前生效前端是
`frontend-3xui/`（裁剪为系统状态/入站/客户端/出站/路由/Xray/面板设置
七个路由，并换用 Lumen 品牌），由本面板 Rust 后端提供 API。`frontend/` 是之前
自研表单的源码，已不参与构建，保留备用；`web/dist-custom-v2/` 是其构建产物。

## 预编译二进制

每个 GitHub Release 只附带一个资产：

- `xray-panel-rs-linux-amd64-lumen.tar.gz`：完整安装包（Linux amd64 静态二进制、
  systemd 单元、安装脚本、README 和包内 `SHA256SUMS`）

面板在线更新下载这个压缩包，使用 GitHub 为 Release 资产自动计算的 SHA-256
digest 校验后解出二进制；不依赖额外校验文件。也可在源码目录执行：

```sh
cargo zigbuild --release --target x86_64-unknown-linux-musl
```

GitHub Actions 在推送 `v*` 标签时自动构建并发布以上文件
（`.github/workflows/release.yml`）。发布前先同步 `Cargo.toml` 版本，然后：

```sh
git tag v0.2.17
git push origin v0.2.17
```

标签必须与 `Cargo.toml` 的版本一致，否则 release workflow 会失败。

## 手动安装（无安装脚本）

```sh
sudo install -d -m 0750 /etc/xray-panel
sudo install -d -m 0750 /usr/local/etc/xray
sudo install -m 0755 target/release/xray-panel-rs /usr/local/bin/xray-panel-rs
```

创建 `/etc/xray-panel.env`（权限 0600）：

```ini
PANEL_LISTEN=0.0.0.0:2053
PANEL_DB=/etc/xray-panel/panel.db
PANEL_USERNAME=admin
PANEL_PASSWORD=admin
XRAY_BIN=/usr/local/bin/xray
XRAY_CONFIG=/usr/local/etc/xray/config.json
XRAY_API_ADDR=127.0.0.1:10085
PANEL_XRAY_LOG=/etc/xray-panel/xray.log
PANEL_WORKERS=2
PANEL_LOG=info
```

首次启动默认账号是 **admin / admin**（可用 `PANEL_USERNAME` / `PANEL_PASSWORD`
覆盖）。默认监听 `0.0.0.0:2053` 且没有 TLS，**登录后请立即通过侧边栏设置图标
（或直接访问 `/settings`）修改密码**，并用防火墙限制 2053 端口的访问来源。
`/settings` 还提供 `.db` 备份/恢复入口。

参考 `deploy/xray-panel.service` 和 `deploy/xray-panel.env.example`；或直接复制到
`/etc/systemd/system/xray-panel.service`：

```ini
[Unit]
Description=xray-panel-rs
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
EnvironmentFile=/etc/xray-panel.env
# 防止相对路径的数据库/日志落到根目录
WorkingDirectory=/etc/xray-panel
ExecStart=/usr/local/bin/xray-panel-rs
Restart=on-failure
RestartSec=3
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=full
# Xray 二进制替换与 Geofiles 默认写入 XRAY_BIN 目录
ReadWritePaths=/etc/xray-panel /usr/local/etc/xray /usr/local/bin
UMask=0077

[Install]
WantedBy=multi-user.target
```

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now xray-panel
journalctl -u xray-panel -f
```

面板默认监听 `0.0.0.0:2053`（所有网卡），监听地址可用 `PANEL_LISTEN` 手动修改
（例如改为 `127.0.0.1:2053` 仅本机访问）。本项目不提供 SSL 证书/强制 HTTPS
功能；若直接暴露公网，请自行用防火墙限制来源，或放在 HTTPS 反向代理之后。SSH
隧道方式：

```sh
ssh -L 2053:127.0.0.1:2053 user@server
# 浏览器打开 http://127.0.0.1:2053
```

不要把面板直接暴露到公网；本项目不提供 TLS/证书功能。若确有需要，请自行放在
可信的 HTTPS 反向代理之后，并设置 `PANEL_SECURE_COOKIE=true`。

## 环境变量

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `PANEL_LISTEN` | `0.0.0.0:2053` | 面板首次启动的监听地址；之后以面板设置页保存的 IP/端口为准 |
| `PANEL_DB` | `xray-panel.db` | SQLite 路径 |
| `XRAY_BIN` | `/usr/local/bin/xray` | Xray 可执行文件（可随时手动替换） |
| `XRAY_CONFIG` | `/usr/local/etc/xray/config.json` | 面板生成的配置路径（会覆盖） |
| `XRAY_API_ADDR` | `127.0.0.1:10085` | Xray StatsService gRPC 地址 |
| `PANEL_XRAY_LOG` | 与 `PANEL_DB` 同目录的 `xray.log` | Xray stdout/stderr 的落盘日志；面板“日志”弹窗优先读取该文件 |
| `PANEL_USERNAME` | `admin` | 首次启动创建的用户名 |
| `PANEL_PASSWORD` | `admin` | 首次启动密码（仅首次，之后以数据库为准） |
| `PANEL_WORKERS` | `2` | Tokio 工作线程数 |
| `PANEL_LOG` | `info` | 日志级别 |
| `PANEL_SECURE_COOKIE` | `false` | 仅 HTTPS 反代后设为 `true` |

## Xray 配置与核心替换

- 面板每次应用变更都会重新生成 `XRAY_CONFIG`，旧文件备份为 `config.json.bak`。
- 生成前执行 `xray run -test -c <临时文件>`，失败不会覆盖线上配置。
- `stats`、`api`、`inbounds` 由面板强制管理；`log`、`dns`、`outbounds`、
  `routing`、`metrics`、`policy` 可在 Xray 页面（基础/连接限制/日志等标签页或
  高级 JSON）编辑。`policy` 只强制 `statsUserOnline`，连接限制和统计开关按用户配置生效。
- Xray 由面板作为子进程启动和监管，**不要再同时用 systemd 启动同一个 Xray**，
  否则 API 端口/入站端口会冲突。
- 替换 Xray 核心：停止面板 → 替换 `XRAY_BIN` 指向的二进制 → 启动面板。
  面板启动时会重新探测 `xray version`。

## 低内存设计

- Tokio 默认 2 个 worker（`PANEL_WORKERS` 可调）
- SQLite 连接池上限 2，页缓存约 2 MiB，WAL + `synchronous=NORMAL`
- 不使用 ORM；所有查询走 sqlx 轻量映射
- 流量统计使用常驻 gRPC 连接，不按周期拉起 `xray api` 子进程
- 流量轮询默认 10 秒，可在面板设置中调整为 5–300 秒
- Rust release 使用 `opt-level="z"`、LTO、strip；运行时内存主要来自 Xray 本身
- 开发机 release 实测（Windows，空闲、未加载真实 Xray 流量）：约 9 MB 工作集 /
  2.5 MB 私有内存；Linux 分配器数字会略有差异

## 已知限制

- Shadowsocks 传统加密方式只支持单客户端（Xray 限制）；推荐使用 2022-blake3。
- 同一客户端关联多个 Shadowsocks 入站时，应使用相同的加密方式和密码。
- HTTP/Mixed 协议没有标准分享链接。
- 入站协议支持 Xray 原生的 `vmess / vless / trojan / shadowsocks / http / mixed /
  wireguard / hysteria / tunnel / tun / masque`；出站支持 13 种 Xray 原生协议。
  MTProto / AmneziaWG / TUIC 需要 mtg-multi / tuic-server / amneziawg-go 等
  sidecar，本面板不内置，入站表单不提供；WARP/Nord/PIA 等集成同样不做。
- Hysteria / MASQUE 必须配置有效的 TLS 证书；否则 Xray 配置校验会失败。
- 3x-ui 前端页面已复制，但 Rust 兼容 API 尚未覆盖全部高级接口（如 2FA、邮件、
  订阅服务器、节点、HWID 日志）；未实现的操作会返回明确错误，不会假装成功。
  路由测试、REALITY 扫描、系统历史、面板自更新、客户端 IP 日志、Happ/订阅
  链接等入口已移除。
- Xray 的 WebSocket 实时推送未实现，前端已禁用 WS，改用操作后重新拉取。
- Xray 配置文件由面板生成并托管，请通过页面修改而不是直接编辑。
- `limitIp` 通过 Xray 在线 IP 数近似执行（超限会停用客户端并重启 Xray）。
- Xray 版本/Geofiles 更新会停止并重启 Xray；下载按官方 SHA-256（Xray `.dgst`、
  Geofiles `.sha256sum`）校验，失败不替换。更新代理在设置页「运行环境」配置。
- GEOdata 使用 Xray 原生 `geodata` 配置块，下载出站由该块里的 `outbound`
  选择；面板设置中的更新代理用于 Xray 版本与 Geofiles。
- 在线状态依赖 Xray 的 `statsUserOnline` 策略与 `GetUsersStats` RPC（Xray 26+
  支持）；旧核心下在线列表为空。
- 客户端变更当前通过重新生成配置并重启 Xray 生效（会短暂中断连接）。
- `.db` 恢复要求备份与当前面板 schema 一致；不兼容的旧备份会回滚并报错。
- 仅支持单管理员账号。
- 面板异常崩溃（不是正常停止）时，已启动的 Xray 子进程可能残留；下次启动面板前
  请先结束残留的 Xray，或使用 systemd（默认 KillMode 会随服务停止子进程）。
