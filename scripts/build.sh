#!/usr/bin/env bash
# Builds the Orbitry release binary: UI first (embedded into the binary), then Rust.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root/ui"
[ -d node_modules ] || npm install
npm run build
cd "$root"
cargo build --release -p orbitry
mkdir -p dist
cp target/release/orbitry dist/orbitry
echo "Built dist/orbitry"