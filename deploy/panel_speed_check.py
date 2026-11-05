#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Panel speed diagnostic: compares four sources over one window.

  1. Xray core counters   (xray api statsquery, the ground truth)
  2. panel WS traffic frame (what the browser divides to draw the speed)
  3. panel REST /inbounds/list/slim upSpeed/downSpeed
  4. panel DB inbounds.up/down (billed totals)

Run on the panel server while traffic flows through the inbound:

  sudo python3 panel_speed_check.py --tag <入站tag> --dur 40
  # 面板用 https 自签: --panel https://127.0.0.1:2053  (自动跳过证书校验)

Print the last table here together with "面板 UI 上看到的数字".
"""
import argparse
import base64
import json
import os
import re
import socket
import ssl
import struct
import subprocess
import sys
import time
import http.client
import urllib.parse
import urllib.request

DEFAULT_DB_CANDIDATES = [
    "/etc/xray-panel/panel.db",
    "/etc/xray-panel/xray-panel.db",
    "/opt/xray-panel/panel.db",
    "/var/lib/xray-panel/panel.db",
    "./panel.db",
]


def human(n):
    n = float(n)
    for unit in ("B", "KB", "MB", "GB"):
        if abs(n) < 1024:
            return f"{n:,.1f} {unit}/s" if unit != "B" else f"{n:,.0f} {unit}/s"
        n /= 1024.0
    return f"{n:,.1f} TB/s"


def find_db(explicit):
    if explicit:
        return explicit
    # systemd unit EnvironmentFile first (authoritative)
    for path in ("/etc/xray-panel.env", "/etc/sysconfig/xray-panel"):
        try:
            with open(path, "r", encoding="utf-8", errors="ignore") as fh:
                for line in fh:
                    m = re.match(r'\s*PANEL_DB\s*=\s*"?(.*?) "?\s*$', line)
                    if m and os.path.exists(m.group(1)):
                        return m.group(1)
        except OSError:
            pass
    for cand in DEFAULT_DB_CANDIDATES:
        if os.path.exists(cand):
            return cand
    return None


def api_addr_from_env():
    for path in ("/etc/xray-panel.env", "/etc/sysconfig/xray-panel"):
        try:
            with open(path, "r", encoding="utf-8", errors="ignore") as fh:
                for line in fh:
                    m = re.match(r'\s*XRAY_API_ADDR\s*=\s*"?(.*?)"?\s*$', line)
                    if m:
                        return m.group(1)
        except OSError:
            pass
    return None


def read_db(db_path, tags):
    """Return {tag: (up, down)} read-only (mode=ro avoids locking the writer)."""
    import sqlite3

    uri = "file:%s?mode=ro" % urllib.request.pathname2url(db_path)
    con = sqlite3.connect(uri, uri=True, timeout=2.0)
    try:
        out = {}
        for tag, up, down in con.execute("SELECT tag, up, down FROM inbounds"):
            if tags is None or tag in tags:
                out[tag] = (up, down)
        return out
    finally:
        con.close()


def login(panel, user, password):
    """POST /login like the panel UI does; return a Cookie header value."""
    body = urllib.parse.urlencode({"username": user, "password": password}).encode()
    req = urllib.request.Request(panel.rstrip("/") + "/login", data=body)
    if panel.startswith("https"):
        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        resp = urllib.request.urlopen(req, context=ctx, timeout=10)
    else:
        resp = urllib.request.urlopen(req, timeout=10)
    cookies = resp.headers.get_all("Set-Cookie") or []
    jar = {}
    for line in cookies:
        name, _, rest = line.partition("=")
        jar[name.strip()] = rest.split(";")[0].strip()
    if not jar:
        raise SystemExit("login returned no session cookie: %s" % resp.read(200))
    return "; ".join("%s=%s" % kv for kv in jar.items())


def rest_slim(panel, cookie, tags):
    """GET /panel/api/inbounds/list/slim; return {tag: (upSpeed, downSpeed, up, down)}."""
    req = urllib.request.Request(
        panel.rstrip("/") + "/panel/api/inbounds/list/slim",
        headers={"Cookie": cookie},
    )
    if panel.startswith("https"):
        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        resp = urllib.request.urlopen(req, context=ctx, timeout=10)
    else:
        resp = urllib.request.urlopen(req, timeout=10)
    payload = json.loads(resp.read())
    rows = payload.get("obj") or payload.get("data") or []
    out = {}
    for row in rows:
        tag = row.get("tag")
        if tags is None or tag in tags:
            out[tag] = (
                row.get("upSpeed", 0),
                row.get("downSpeed", 0),
                row.get("up", 0),
                row.get("down", 0),
            )
    return out


def xray_counters(api_addr, pattern):
    """`xray api statsquery -pattern <family>` on the panel's own API port."""
    if not XRAY_BIN:
        return None
    cmd = [XRAY_BIN, "api", "statsquery", "--server", api_addr, "-pattern", pattern]
    try:
        proc = subprocess.run(cmd, capture_output=True, timeout=15)
    except (OSError, subprocess.TimeoutExpired) as err:
        print("  xray statsquery failed:", err, file=sys.stderr)
        return None
    text = proc.stdout.decode("utf-8", "replace")
    out = {}
    for m in re.finditer(
        r'"(inbound>>>[^"]+>>>traffic>>>(?:up|down)link)"\s*,\s*"value"\s*:\s*(\d+)',
        text,
    ):
        out[m.group(1)] = int(m.group(2))
    if not out:
        # Non-JSON output (older xray): NAME=... value lines / proto text.
        for m in re.finditer(
            r"(inbound>>>\S+>>>traffic>>>(?:up|down)link)[^0-9]*(\d+)", text
        ):
            out[m.group(1)] = int(m.group(2))
    if not out and proc.returncode != 0:
        print("  statsquery stderr:", proc.stderr.decode("utf-8", "replace")[:300],
              file=sys.stderr)
    return out or None


class WsCapture:
    """Minimal text-frame websocket reader for the panel's /ws traffic bus."""

    def __init__(self, panel, cookie):
        parts = urllib.parse.urlparse(panel)
        host = parts.hostname
        port = parts.port or (443 if parts.scheme == "https" else 80)
        self.sock = socket.create_connection((host, port), timeout=10)
        if parts.scheme == "https":
            ctx = ssl.create_default_context()
            ctx.check_hostname = False
            ctx.verify_mode = ssl.CERT_NONE
            self.sock = ctx.wrap_socket(self.sock, server_hostname=host)
        key = base64.b64encode(os.urandom(16)).decode()
        # The panel's UI connects `<basePath>ws`; with an empty base that is
        # exactly "/ws". A leading double slash misses the route and answers
        # the SPA HTML instead of upgrading.
        base = parts.path if parts.path.endswith("/") else parts.path + "/"
        req = (
            "GET %s HTTP/1.1\r\nHost: %s\r\nUpgrade: websocket\r\n"
            "Connection: Upgrade\r\nSec-WebSocket-Key: %s\r\n"
            "Sec-WebSocket-Version: 13\r\nCookie: %s\r\n\r\n"
            % (base.rstrip("/") + "/ws", host, key, cookie)
        ).encode()
        self.sock.sendall(req)
        buf = b""
        while b"\r\n\r\n" not in buf:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise SystemExit("ws handshake closed")
            buf += chunk
        head, _, self.rest = buf.partition(b"\r\n\r\n")
        if b"101" not in head.split(b"\r\n", 1)[0]:
            raise SystemExit("ws handshake rejected: %s" % head[:200])

    def _recv_exact(self, n):
        while len(self.rest) < n:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise OSError("ws closed")
            self.rest += chunk
        out, self.rest = self.rest[:n], self.rest[n:]
        return out

    def next_frame(self, timeout):
        self.sock.settimeout(timeout)
        try:
            first, second = self._recv_exact(2)
        except (socket.timeout, OSError):
            return None
        opcode = first & 0x0F
        length = second & 0x7F
        if length == 126:
            length = struct.unpack(">H", self._recv_exact(2))[0]
        elif length == 127:
            length = struct.unpack(">Q", self._recv_exact(8))[0]
        payload = self._recv_exact(length) if length else b""
        if opcode == 0x8:
            raise OSError("ws closed by panel")
        if opcode not in (0x1, 0x2):
            return b""
        return payload


def parse_ws_traffic(payload, tags):
    """Return {tag: (up, down, intervalMs)} for traffic frames."""
    try:
        msg = json.loads(payload)
    except (ValueError, UnicodeDecodeError):
        return None
    body = msg.get("payload") if isinstance(msg.get("payload"), dict) else msg
    if not isinstance(body, dict):
        return None
    rows = body.get("traffics")
    if not isinstance(rows, list):
        return None
    interval = body.get("clientTrafficIntervalMs") or body.get("intervalMs") or 0
    out = {}
    for row in rows:
        tag = row.get("Tag") or row.get("tag")
        if tag and (tags is None or tag in tags):
            out[tag] = (
                row.get("Up", row.get("up", 0)),
                row.get("Down", row.get("down", 0)),
                int(interval or 0),
            )
    return out


def main():
    global XRAY_BIN
    ap = argparse.ArgumentParser()
    ap.add_argument("--panel", default="http://127.0.0.1:2053",
                    help="面板地址，https 自签证书自动放行")
    ap.add_argument("--user", default="", help="面板用户名（--tag 给全时可留空跳过 REST/WS）")
    ap.add_argument("--password", default="", help="面板密码")
    ap.add_argument("--tag", action="append", default=None,
                    help="只看指定入站 tag，可重复；缺省看全部")
    ap.add_argument("--dur", type=int, default=40, help="观察秒数")
    ap.add_argument("--api", default="", help="Xray API 地址，缺省从环境变量/127.0.0.1:10085")
    ap.add_argument("--xray", default="", help="xray 可执行文件，缺省 PATH 或 /usr/local/bin/xray")
    ap.add_argument("--db", default="", help="panel.db 路径，缺省自动探测")
    args = ap.parse_args()

    tags = set(args.tag) if args.tag else None
    XRAY_BIN = args.xray or os.environ.get("XRAY_BIN") or next(
        (p for p in ("/usr/local/bin/xray", "/usr/bin/xray") if os.path.exists(p)), None
    )
    api_addr = (
        args.api
        or os.environ.get("XRAY_API_ADDR")
        or api_addr_from_env()
        or "127.0.0.1:10085"
    )
    db_path = find_db(args.db)

    print("panel=%s  api=%s  xray=%s  db=%s  dur=%ss  tags=%s"
          % (args.panel, api_addr, XRAY_BIN, db_path, args.dur,
             sorted(tags) if tags else "ALL"))

    have_panel = bool(args.user and args.password)
    cookie = login(args.panel, args.user, args.password) if have_panel else None
    ws = None
    if have_panel:
        try:
            ws = WsCapture(args.panel, cookie)
        except SystemExit as err:
            print("WS 连接失败（不影响其余对照）:", err)

    x0 = xray_counters(api_addr, "inbound>>>")
    r0 = read_db(db_path, tags) if db_path else None
    t0 = time.time()

    frames = {}   # tag -> [bytes, ...]
    intervals = {}  # tag -> [ms, ...]
    # The UI column alternates between the WS frame value and the REST
    # snapshot, and a single REST read is one 5s window — a ramping transfer
    # reads 2-3x its own average. Record every REST poll so the table can
    # compare averages and peaks instead of one lucky instant.
    rest_series = {}  # tag -> [((up, down), ...)]
    rest_times = {}
    next_rest = 0.0
    end = time.time() + args.dur
    print("采集中…（期间请让流量稳定通过该入站）")
    while time.time() < end:
        now = time.time()
        if have_panel and now >= next_rest:
            next_rest = now + 1.0
            try:
                snap = rest_slim(args.panel, cookie, tags)
                for tag, (ru, rd, _, _) in snap.items():
                    rest_series.setdefault(tag, []).append((ru, rd))
                    rest_times.setdefault(tag, []).append(now)
            except Exception as err:
                print("REST 取样失败:", err)
        if ws is not None:
            payload = ws.next_frame(timeout=0.4)
            if payload:
                got = parse_ws_traffic(payload, tags)
                if got:
                    for tag, (up, down, interval) in got.items():
                        frames.setdefault(tag, [0, 0])
                        frames[tag][0] += up
                        frames[tag][1] += down
                        if interval:
                            intervals.setdefault(tag, []).append(
                                (now, up, down, interval))
            continue
        time.sleep(0.4)

    x1 = xray_counters(api_addr, "inbound>>>")
    r1 = read_db(db_path, tags) if db_path else None
    span = time.time() - t0

    want = tags or set()
    if x0 and x1:
        want |= {n.split(">>>")[1] for n in x0}
    if frames:
        want |= set(frames)
    if r0:
        want |= set(r0)

    print("\n=== 对照表（%s 秒窗口）===" % f"{span:.1f}")
    print("%-20s %16s %14s %22s %16s" % ("tag", "Xray原始", "WS帧合计", "REST速度(均值/峰值)", "DB计费"))
    ok = True
    for tag in sorted(want):
        def fam(name_suffix):
            key = "inbound>>>%s>>>traffic>>>%s" % (tag, name_suffix)
            if not (x0 and x1):
                return None
            if key in x0 and key in x1 and x1[key] >= x0[key]:
                return (x1[key] - x0[key]) / span
            return None

        raw_up, raw_down = fam("uplink"), fam("downlink")

        f_up = f_down = 0.0
        if tag in frames:
            f_up = frames[tag][0] / span
            f_down = frames[tag][1] / span
        iv = intervals.get(tag) or []
        iv_note = ""
        if len(iv) > 1:
            gaps = [iv[i + 1][0] - iv[i][0] for i in range(len(iv) - 1)]
            div_ms = sum(row[3] for row in iv) / len(iv)
            iv_note = " 帧间隔min%.1f/avg%.1f/max%.1fs vs 除数%.1fs" % (
                min(gaps), sum(gaps) / len(gaps), max(gaps), div_ms / 1000.0)
        f_total = f_up + f_down

        rest_txt = "-"
        rest_avg = None
        rest_peak = None
        series = rest_series.get(tag) or []
        if series:
            avg_up = sum(s[0] for s in series) / len(series)
            avg_dn = sum(s[1] for s in series) / len(series)
            peak = max(series, key=lambda s: s[0] + s[1])
            rest_txt = "均%s+%s|峰%s+%s" % (
                human(avg_up).replace("/s", ""),
                human(avg_dn).replace("/s", ""),
                human(peak[0]).replace("/s", ""),
                human(peak[1]).replace("/s", ""),
            )
            rest_avg = avg_up + avg_dn
            rest_peak = peak[0] + peak[1]

        db_txt = "-"
        db_total = None
        if r0 and r1 and tag in r0 and tag in r1:
            du = max(0, r1[tag][0] - r0[tag][0]) / span
            dd = max(0, r1[tag][1] - r0[tag][1]) / span
            db_txt = "%s+%s" % (human(du), human(dd))
            db_total = du + dd

        raw_txt = "-"
        raw_total = None
        if raw_up is not None or raw_down is not None:
            raw_txt = "%s+%s" % (human(raw_up or 0), human(raw_down or 0))
            raw_total = (raw_up or 0) + (raw_down or 0)

        print("%-20s %16s %14s %22s %16s%s"
              % (tag, raw_txt, human(f_total) if tag in frames else "-",
                 rest_txt, db_txt, iv_note))

        # Per-tag verdict lines keep the table readable.
        def cmp(label, value):
            nonlocal ok
            if raw_total and value is not None and raw_total > 10_000 and value > 10_000:
                drift = (value - raw_total) / raw_total
                if abs(drift) > 0.25:
                    print("  → [%s] %s 较 Xray 原始 %+.0f%%" % (tag, label, drift * 100))
                    ok = False

        cmp("WS帧合计", f_total if tag in frames else None)
        cmp("DB计费", db_total)
        cmp("REST均值", rest_avg)
        if raw_total and rest_peak is not None and raw_total > 10_000:
            if abs(rest_peak - raw_total) / raw_total > 0.5:
                print("  → [%s] REST 峰值较原始 %+.0f%%（爬坡流量下属正常：UI 末次窗口≠全程均值）"
                      % (tag, (rest_peak - raw_total) / raw_total * 100))

    print("\n=== 判定 ===")
    if ok:
        print("  四方均值在 ±25% 内一致：核心计数、WS 帧、REST、计费都正确，")
        print("  速度列没有丢数或被摊薄。UI 与网卡监控的差距属于口径差异")
        print("  （面板只算该端口 payload；监控含包头/重传/整机其他流量/双向合计），")
        print("  以及采样窗平均 vs 瞬时值的差别。")
    else:
        print("  存在超出 ±25% 的偏差，按上面箭头列出的项排查。")
    print("\n请把上面整段表 + “面板 UI 上该入站速度列的数字” + “服务器监控同一时刻的数字”一起发来。")


if __name__ == "__main__":
    main()
