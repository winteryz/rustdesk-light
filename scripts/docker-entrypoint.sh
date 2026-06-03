#!/usr/bin/env bash
set -Eeuo pipefail

CONFIG_DIR="${RDL_CONFIG_DIR:-/etc/rust-desk-light}"
DATA_DIR="${RDL_DATA_DIR:-/var/lib/rust-desk-light}"
SERVER_CONF="$CONFIG_DIR/server.toml"

mkdir -p "$CONFIG_DIR" "$DATA_DIR"

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
  fi
elif [ -d "/geoip" ]; then
  MMDB=$(find /geoip -maxdepth 1 -name '*.mmdb' -print 2>/dev/null | head -1)
  if [ -n "$MMDB" ]; then
    ARGS+=("--geoip-db" "$MMDB")
    RDL_GEOIP_DB="$MMDB"
  fi
fi

if [ "${RDL_REQUIRE_CLIENT_AUTH:-}" = "1" ] || [ "${RDL_REQUIRE_CLIENT_AUTH:-}" = "true" ]; then
  ARGS+=("--require-client-auth")
fi

exec /usr/local/bin/rdl-server-cli "${ARGS[@]}"
