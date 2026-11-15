#!/usr/bin/env bash
set -euo pipefail
ROOT="${1:-$(dirname "$(dirname "$(cd "$(dirname "$0")" && pwd)")")}"
PORT="${2:-4280}"
REPO="$(cd "$(dirname "$0")/.." && pwd)"

echo "Orbitry API on http://127.0.0.1:${PORT}"
echo "Orbitry UI  on http://127.0.0.1:4281"
echo "Scanning    ${ROOT}"

(cd "$REPO" && cargo run -p orbitry -- --root "$ROOT" --port "$PORT" --no-open) &
API_PID=$!
trap 'kill "$API_PID" 2>/dev/null || true' EXIT
(cd "$REPO/ui" && npm run dev)