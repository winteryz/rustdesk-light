#!/usr/bin/env bash
set -Eeuo pipefail

CONFIG_DIR="${RDL_CONFIG_DIR:-/etc/rust-desk-light}"
DATA_DIR="${RDL_DATA_DIR:-/var/lib/rust-desk-light}"
SERVER_CONF="$CONFIG_DIR/server.toml"

mkdir -p "$CONFIG_DIR" "$DATA_DIR"

prompt_with_default() {
  local var_name="$1"
  local prompt="$2"
  local default="$3"
  local current="${!var_name:-}"
  local display="${current:-$default}"
  read -r -p "$prompt [$display]: " input
  if [ -n "$input" ]; then
    printf -v "$var_name" "%s" "$input"
  elif [ -z "$current" ]; then
    printf -v "$var_name" "%s" "$default"
  fi
}

guided_setup() {
  echo "========================================"
  echo " rust-desk-light server setup"
  echo "========================================"
  echo
  echo "--- Network ---"
  prompt_with_default RDL_IP     "Listen IP"     "0.0.0.0"
  prompt_with_default RDL_PORT   "Listen port"   "5169"
  echo
  echo "--- Auth ---"
  prompt_with_default RDL_AUTH_TOKEN "Auth token (leave empty to auto-generate)" ""
  read -r -p "Require client auth? [y/N]: " auth_choice
  case "$auth_choice" in
    y|Y|yes|YES) RDL_REQUIRE_CLIENT_AUTH="true" ;;
    *)           RDL_REQUIRE_CLIENT_AUTH="false" ;;
  esac
  echo
  echo "--- GeoIP ---"
  mmdb_auto=""
  if [ -d "/geoip" ]; then
    mmdb_auto=$(find /geoip -maxdepth 1 -name '*.mmdb' -print 2>/dev/null | head -1)
  fi
  if [ -n "$mmdb_auto" ]; then
    echo "  Auto-detected: $mmdb_auto"
    RDL_GEOIP_DB="$mmdb_auto"
  else
    read -r -p "GeoIP db path (empty to skip): " geoip_input
    if [ -n "$geoip_input" ] && [ -f "$geoip_input" ]; then
      RDL_GEOIP_DB="$geoip_input"
    fi
  fi
  echo
  echo "========================================"
  echo
}

[ -t 0 ] && [ -t 1 ] && guided_setup

build_args() {
  ARGS=()
  ARGS+=("--ip" "${RDL_IP:-0.0.0.0}")
  ARGS+=("--port" "${RDL_PORT:-5169}")

  if [ -n "${RDL_AUTH_TOKEN:-}" ]; then
    ARGS+=("--auth-token" "$RDL_AUTH_TOKEN")
  fi

  if [ -f "$SERVER_CONF" ]; then
    ARGS+=("--config" "$SERVER_CONF")
  fi

  if [ -n "${RDL_GEOIP_DB:-}" ]; then
    if [ -f "$RDL_GEOIP_DB" ]; then
      ARGS+=("--geoip-db" "$RDL_GEOIP_DB")
    else
      echo "WARNING: RDL_GEOIP_DB set but not found: $RDL_GEOIP_DB"
    fi
  elif [ -d "/geoip" ]; then
    mmdb=$(find /geoip -maxdepth 1 -name '*.mmdb' -print 2>/dev/null | head -1)
    if [ -n "$mmdb" ]; then
      ARGS+=("--geoip-db" "$mmdb")
      RDL_GEOIP_DB="$mmdb"
    fi
  fi

  if [ "${RDL_REQUIRE_CLIENT_AUTH:-}" = "1" ] || [ "${RDL_REQUIRE_CLIENT_AUTH:-}" = "true" ]; then
    ARGS+=("--require-client-auth")
  fi
}

build_args

echo "========================================"
echo " rust-desk-light server"
echo "========================================"
echo "  listen: ${RDL_IP:-0.0.0.0}:${RDL_PORT:-5169}"
echo "  config: ${SERVER_CONF}"
echo "  geoip:  ${RDL_GEOIP_DB:-disabled}"
echo "  auth:   ${RDL_AUTH_TOKEN:+set}${RDL_AUTH_TOKEN:-auto-generate}"
echo "  restart: automatic on crash"
echo "========================================"
echo

STOP_REQUESTED=0
trap 'echo "Shutdown requested..."; STOP_REQUESTED=1' TERM INT

if [ "${RDL_DISABLE_RESTART:-}" = "1" ]; then
  exec /usr/local/bin/rdl-server-cli "${ARGS[@]}"
fi

MAX_RESTART_DELAY=30
RESTART_DELAY=1

while [ "$STOP_REQUESTED" -eq 0 ]; do
  if /usr/local/bin/rdl-server-cli "${ARGS[@]}"; then
    exit_code=0
  else
    exit_code=$?
  fi
  [ "$STOP_REQUESTED" -eq 1 ] && break
  echo
  echo "========================================"
  echo " Server exited (code: $exit_code)"
  echo " Restarting in ${RESTART_DELAY}s ..."
  echo "========================================"
  echo
  sleep "$RESTART_DELAY" 2>/dev/null || true
  [ "$STOP_REQUESTED" -eq 1 ] && break
  RESTART_DELAY=$((RESTART_DELAY * 2))
  [ "$RESTART_DELAY" -le "$MAX_RESTART_DELAY" ] || RESTART_DELAY="$MAX_RESTART_DELAY"
done