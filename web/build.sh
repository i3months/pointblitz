#!/usr/bin/env bash
# Builds the browser target (decision 0028). Run from the repository root.
#   bash web/build.sh           → web/pkg          WebGPU + WebGL2 fallback (default)
#   bash web/build.sh webgpu    → web/pkg-webgpu   WebGPU only, without the GL backend (decision 0033)
# cargo build --release --target wasm32-unknown-unknown -p pointblitz-web, then
# wasm-bindgen --target web (CLI version must equal the wasm-bindgen crate: =0.2.129).
set -euo pipefail
WANT=0.2.129
HAVE=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')
if [ "$HAVE" != "$WANT" ]; then
  echo "wasm-bindgen CLI $WANT required (found ${HAVE:-none}): cargo install wasm-bindgen-cli --version $WANT --locked" >&2
  exit 1
fi
case "${1:-full}" in
  full) FLAGS=(); OUT=web/pkg; TARGET_DIR=target ;;
  webgpu) FLAGS=(--no-default-features); OUT=web/pkg-webgpu; TARGET_DIR=target/webgpu-only ;;
  *) echo "usage: bash web/build.sh [webgpu]" >&2; exit 2 ;;
esac
# A separate target dir keeps the two feature sets from overwriting each other's artefact.
cargo build --release --target wasm32-unknown-unknown -p pointblitz-web --target-dir "$TARGET_DIR" "${FLAGS[@]}"
wasm-bindgen --target web --no-typescript --out-dir "$OUT" "$TARGET_DIR/wasm32-unknown-unknown/release/pointblitz_web.wasm"
ls -l "$OUT"
