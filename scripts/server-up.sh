#!/usr/bin/env bash
set -Eeuo pipefail

cd "$(dirname "$0")/.."

AUTH_TOKEN="${RDL_AUTH_TOKEN:-mekiller}"
IP="${RDL_IP:-0.0.0.0}"
PORT="${RDL_PORT:-5169}"

echo "========================================"
echo " rust-desk-light server (Docker)"
echo "========================================"
echo "  listen: $IP:$PORT"
echo "  token:  $AUTH_TOKEN"
echo "========================================"
echo

export RDL_AUTH_TOKEN="$AUTH_TOKEN"
export RDL_IP="$IP"
export RDL_PORT="$PORT"

docker compose up -d --build

echo
echo "Logs: docker compose logs -f"
echo "Stop: docker compose down"
