#!/usr/bin/env bash
# Lumen (xray-panel-rs) one-click installer.
#
#   curl -fsSL https://raw.githubusercontent.com/ztoofar/xray-panel-rs/main/install.sh | sudo bash
#
# Environment:
#   PANEL_REPO       GitHub repo (default: ztoofar/xray-panel-rs)
#   PANEL_VERSION    Release tag or version, e.g. v0.2.18 (default: latest)
#   PANEL_PROXY      HTTP/SOCKS proxy for downloads, e.g. http://127.0.0.1:7890
#                    or socks5h://127.0.0.1:1080 (falls back to HTTPS_PROXY)
#   PANEL_FORCE=1    Reinstall even when the installed version is already current
#   SKIP_XRAY=1      Do not install the Xray core when /usr/local/bin/xray is missing
#   XRAY_VERSION     Xray release tag (default: latest)
#   PANEL_DATA_DIR, PANEL_ENV_FILE, XRAY_CONFIG   forwarded to deploy/install.sh
set -euo pipefail

REPO=${PANEL_REPO:-ztoofar/xray-panel-rs}
VERSION=${PANEL_VERSION:-}
PROXY=${PANEL_PROXY:-${HTTPS_PROXY:-${https_proxy:-${ALL_PROXY:-${all_proxy:-}}}}}
FORCE=${PANEL_FORCE:-0}
SKIP_XRAY=${SKIP_XRAY:-0}
XRAY_VERSION=${XRAY_VERSION:-}

PANEL_BIN=/usr/local/bin/xray-panel-rs

log() { printf '\033[1;36m[lumen]\033[0m %s\n' "$*"; }
die() {
  printf '\033[1;31m[lumen]\033[0m %s\n' "$*" >&2
  exit 1
}

PROXY_ARGS=()
if [ -n "$PROXY" ]; then
  PROXY_ARGS=(-x "$PROXY")
  log "using download proxy: $PROXY"
fi

# download URL OUTFILE: fails loudly and mentions the proxy when one is set.
download() {
  local url=$1 out=$2
  if ! curl -fsSL --retry 3 --connect-timeout 20 --max-time 600 \
    "${PROXY_ARGS[@]}" -o "$out" "$url"; then
    if [ -n "$PROXY" ]; then
      die "download failed: $url (proxy $PROXY is unreachable or invalid)"
    fi
    die "download failed: $url (check network access to GitHub)"
  fi
}

# Latest release version without the leading v; empty when it cannot be resolved.
latest_release_version() {
  local headers
  headers=$(curl -fsSI --connect-timeout 10 "${PROXY_ARGS[@]}" \
    "https://github.com/${REPO}/releases/latest" 2>/dev/null) || return 1
  printf '%s\n' "$headers" | tr -d '\r' \
    | sed -n 's#^[Ll]ocation: .*/releases/tag/v\{0,1\}\([^/[:space:]]*\)$#\1#p' \
    | tail -n1
}

# Version reported by the installed binary. Only builds that carry the
# LUMEN_VERSION marker support --version; probing an older binary would start
# the panel, so the marker check comes first.
installed_version() {
  local out
  [ -x "$PANEL_BIN" ] || return 1
  grep -aq 'LUMEN_VERSION=' "$PANEL_BIN" || return 1
  out=$("$PANEL_BIN" --version 2>/dev/null) || return 1
  printf '%s\n' "$out" \
    | sed -n 's/.*LUMEN_VERSION=\([0-9][^ )]*\).*/\1/p' | head -n1
}

if [ "$(id -u)" -ne 0 ]; then
  die "run this installer as root (use sudo)"
fi
[ "$(uname -s)" = "Linux" ] || die "this installer currently supports Linux only"
[ "$(uname -m)" = "x86_64" ] || die "this installer currently supports x86_64 Linux only"

for cmd in curl tar sha256sum; do
  command -v "$cmd" >/dev/null 2>&1 || die "missing required command: $cmd"
done

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

if [ -n "$VERSION" ]; then
  VERSION=${VERSION#v}
  BASE="https://github.com/${REPO}/releases/download/v${VERSION}"
  TARGET=$VERSION
else
  BASE="https://github.com/${REPO}/releases/latest/download"
  # Best effort: when the lookup fails we still download and compare by hash.
  TARGET=$(latest_release_version || true)
fi

MODE=install
if [ -x "$PANEL_BIN" ]; then
  MODE=update
  log "existing Lumen installation detected; updating (database and config are kept)"
fi

install_xray() {
  local asset digest_url digest got zipdir
  case "$(uname -m)" in
    x86_64) asset="Xray-linux-64.zip" ;;
    *) die "unsupported Xray architecture: $(uname -m)" ;;
  esac
  if [ -n "$XRAY_VERSION" ]; then
    digest_url="https://github.com/XTLS/Xray-core/releases/download/v${XRAY_VERSION#v}/${asset}"
  else
    digest_url="https://github.com/XTLS/Xray-core/releases/latest/download/${asset}"
  fi
  log "installing Xray core from ${digest_url}"
  download "$digest_url" "$TMP/xray.zip"
  download "${digest_url}.dgst" "$TMP/xray.dgst"
  digest=$(sed -n 's/^SHA2-256=//p' "$TMP/xray.dgst" | head -n1)
  [ -n "$digest" ] || die "Xray digest file has no SHA2-256 entry"
  got=$(sha256sum "$TMP/xray.zip" | awk '{print $1}')
  [ "$got" = "$digest" ] || die "Xray archive checksum mismatch"
  zipdir="$TMP/xray"
  mkdir -p "$zipdir"
  if ! command -v unzip >/dev/null 2>&1; then
    install_unzip
  fi
  unzip -oq "$TMP/xray.zip" xray -d "$zipdir"
  install -m 0755 "$zipdir/xray" /usr/local/bin/xray
}

install_unzip() {
  if command -v apt-get >/dev/null 2>&1; then
    apt-get update -qq && apt-get install -y -qq unzip
  elif command -v dnf >/dev/null 2>&1; then
    dnf install -y unzip
  elif command -v yum >/dev/null 2>&1; then
    yum install -y unzip
  elif command -v apk >/dev/null 2>&1; then
    apk add --no-cache unzip
  elif command -v zypper >/dev/null 2>&1; then
    zypper --non-interactive install unzip
  else
    die "unzip is required to install Xray; install it and re-run"
  fi
}

# Keep the Xray core present even when the panel itself is up to date.
ensure_xray() {
  if [ "$SKIP_XRAY" != "1" ] && [ ! -x /usr/local/bin/xray ]; then
    install_xray
  fi
  if command -v systemctl >/dev/null 2>&1; then
    if systemctl is-active --quiet xray 2>/dev/null; then
      log "warning: the distribution 'xray' service is running and will fight for inbound ports"
      log "         disable it with: systemctl disable --now xray"
    fi
  fi
}

if [ "$FORCE" != "1" ] && [ -n "$TARGET" ]; then
  CURRENT=$(installed_version || true)
  if [ -n "$CURRENT" ] && [ "$CURRENT" = "$TARGET" ]; then
    log "Lumen ${TARGET} is already installed; skipping reinstall (PANEL_FORCE=1 forces it)"
    ensure_xray
    exit 0
  fi
fi

log "downloading Lumen release from ${REPO} (${MODE})"
download "${BASE}/xray-panel-rs-linux-amd64-lumen.tar.gz" "$TMP/package.tar.gz"
tar -xzf "$TMP/package.tar.gz" -C "$TMP"
PKG="$TMP/xray-panel-rs-linux-amd64-lumen"
[ -f "$PKG/install.sh" ] || die "release package is missing install.sh"
[ -f "$PKG/SHA256SUMS" ] || die "release package is missing SHA256SUMS"

log "verifying package checksum"
(cd "$PKG" && sha256sum -c SHA256SUMS)

# Older binaries do not support --version, but an identical hash still proves
# the installed release is current. Skipping here avoids stopping the service.
if [ "$FORCE" != "1" ] && [ -x "$PANEL_BIN" ]; then
  PKG_SUM=$(sha256sum "$PKG/xray-panel-rs" | awk '{print $1}')
  CUR_SUM=$(sha256sum "$PANEL_BIN" 2>/dev/null | awk '{print $1}') || true
  if [ -n "$CUR_SUM" ] && [ "$PKG_SUM" = "$CUR_SUM" ]; then
    log "installed binary is identical to Lumen ${TARGET:-latest}; skipping reinstall (PANEL_FORCE=1 forces it)"
    ensure_xray
    exit 0
  fi
fi

ensure_xray

log "installing Lumen"
bash "$PKG/install.sh"
