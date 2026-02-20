#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="${ROOT}/data"
mkdir -p "${DATA_DIR}"

export FUNDVAL_DATA_DIR="${DATA_DIR}"
export SECRET_KEY="${SECRET_KEY:-django-insecure-dev-only}"

BACKEND_PORT="${BACKEND_PORT:-8001}"
FRONTEND_PORT="${FRONTEND_PORT:-3000}"

export PORT="${BACKEND_PORT}"
"${ROOT}/fundval-backend" &
BACKEND_PID=$!

cleanup() {
  kill "${BACKEND_PID}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

export PORT="${FRONTEND_PORT}"
export API_PROXY_TARGET="http://localhost:${BACKEND_PORT}"
"${ROOT}/node/bin/node" "${ROOT}/frontend/server.js"

