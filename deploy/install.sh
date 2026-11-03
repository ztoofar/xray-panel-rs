#!/usr/bin/env bash
# Installs Lumen (xray-panel-rs) as a systemd service.
# Run as root from the unpacked release directory:  sudo ./install.sh
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  echo "run this script as root" >&2
  exit 1
fi

SRC_DIR=$(cd "$(dirname "$0")" && pwd)
BIN="$SRC_DIR/xray-panel-rs"
SERVICE_SRC="$SRC_DIR/xray-panel.service"
for file in "$BIN" "$SERVICE_SRC"; do
  [ -f "$file" ] || { echo "missing $file" >&2; exit 1; }
done

DATA_DIR=${PANEL_DATA_DIR:-/etc/xray-panel}
ENV_FILE=${PANEL_ENV_FILE:-/etc/xray-panel.env}
XRAY_DIR=${XRAY_DIR:-/usr/local/etc/xray}
XRAY_CONFIG=${XRAY_CONFIG:-$XRAY_DIR/config.json}

# The panel owns Xray's config from here on; keep a timestamped copy first.
if [ -f "$XRAY_CONFIG" ]; then
  BAK="$XRAY_CONFIG.bak-$(date +%Y%m%d%H%M%S)"
  cp -a "$XRAY_CONFIG" "$BAK"
  echo "backed up existing Xray config to $BAK"
fi

# A running panel keeps its binary busy; stop it before replacing the file.
if command -v systemctl >/dev/null 2>&1 && systemctl is-active --quiet xray-panel; then
  systemctl stop xray-panel
fi
install -m 0755 "$BIN" /usr/local/bin/xray-panel-rs
install -d -m 0750 "$DATA_DIR"

if [ ! -f "$ENV_FILE" ]; then
  umask 077
  cat > "$ENV_FILE" <<EOF
PANEL_LISTEN=0.0.0.0:2053
PANEL_DB=$DATA_DIR/panel.db
PANEL_USERNAME=admin
PANEL_PASSWORD=admin
XRAY_BIN=/usr/local/bin/xray
XRAY_CONFIG=$XRAY_CONFIG
XRAY_API_ADDR=127.0.0.1:10085
PANEL_XRAY_LOG=$DATA_DIR/xray.log
PANEL_WORKERS=2
PANEL_LOG=info
EOF
  echo "wrote $ENV_FILE"
else
  echo "kept existing $ENV_FILE"
fi
chmod 600 "$ENV_FILE" || true

install -m 0644 "$SERVICE_SRC" /etc/systemd/system/xray-panel.service
systemctl daemon-reload
systemctl enable xray-panel
systemctl restart xray-panel
sleep 1
systemctl --no-pager --full status xray-panel | head -20 || true

cat <<'EOF'

Lumen is starting.
  URL:      http://<server-ip>:2053
  Login:    admin / admin  (change immediately)
  Data:     /etc/xray-panel/panel.db
  Logs:     /etc/xray-panel/xray.log and journalctl -u xray-panel -f

Stop any distro Xray service first if it is running (it would fight for the
inbound ports):
  systemctl disable --now xray
EOF
