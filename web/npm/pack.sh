#!/usr/bin/env bash
# Stages the npm package in target/npm/pointblitz-web and lists what `npm pack` would include
# (P4.7). Does not publish: package.json is "private" until publishing is decided (P4.8).
# Run from the repository root.
set -euo pipefail
OUT=target/npm/pointblitz-web
bash web/build.sh > /dev/null
bash web/build.sh webgpu > /dev/null
rm -rf "${OUT:?}"
mkdir -p "$OUT"
cp web/npm/package.json web/npm/index.js web/npm/webgpu.js web/npm/README.md web/chunks.js LICENSE-MIT LICENSE-APACHE "$OUT/"
cp -r web/pkg web/pkg-webgpu "$OUT/"
(cd "$OUT" && npm pack --dry-run 2>&1 | grep -E "npm notice [0-9.]+[kMG]?B |total files|package size|unpacked size")
