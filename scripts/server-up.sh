#!/usr/bin/env bash
set -Eeuo pipefail

cd "$(dirname "$0")/.."

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

echo "========================================"
echo " rust-desk-light server (Docker)"
echo "========================================"
echo

echo "--- Network ---"
prompt_with_default RDL_IP   "Listen IP"   "${RDL_IP:-0.0.0.0}"
prompt_with_default RDL_PORT "Listen port" "${RDL_PORT:-5169}"
echo

echo "--- Auth ---"
prompt_with_default RDL_AUTH_TOKEN "Auth token (leave empty to auto-generate)" "${RDL_AUTH_TOKEN:-mekiller}"
read -r -p "Require client auth? [y/N]: " auth_choice
case "$auth_choice" in
  y|Y|yes|YES) RDL_REQUIRE_CLIENT_AUTH="true" ;;
  *)           RDL_REQUIRE_CLIENT_AUTH="false" ;;
esac
echo

echo "--- GeoIP ---"
mmdb_file=""
if [ -d "third_party/geoip" ]; then
  mmdb_file=$(find third_party/geoip -maxdepth 1 -name '*.mmdb' -print 2>/dev/null | head -1)
fi
if [ -n "$mmdb_file" ]; then
  echo "  Auto-detected: $mmdb_file"
fi
echo

echo "========================================"
echo
echo "Starting with:"
echo "  listen: ${RDL_IP}:${RDL_PORT}"
echo "  token:  ${RDL_AUTH_TOKEN}"
echo "  geoip:  ${mmdb_file:-disabled}"
echo "  client auth: ${RDL_REQUIRE_CLIENT_AUTH}"
echo

export RDL_AUTH_TOKEN="${RDL_AUTH_TOKEN}"
export RDL_IP="${RDL_IP}"
export RDL_PORT="${RDL_PORT}"
export RDL_REQUIRE_CLIENT_AUTH="${RDL_REQUIRE_CLIENT_AUTH}"

docker compose up -d --build

echo
echo "Logs: docker compose logs -f"
echo "Stop: docker compose down"
