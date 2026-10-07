#!/usr/bin/env bash
# Builds the browser target into web/pkg (decision 0028). Run from the repository root.
#   cargo build --release --target wasm32-unknown-unknown -p pointblitz-web
#   wasm-bindgen --target web  (CLI version must equal the wasm-bindgen crate: =0.2.129)
set -euo pipefail
WANT=0.2.129
HAVE=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')
if [ "$HAVE" != "$WANT" ]; then
  echo "wasm-bindgen CLI $WANT required (found ${HAVE:-none}): cargo install wasm-bindgen-cli --version $WANT --locked" >&2
  exit 1
fi
cargo build --release --target wasm32-unknown-unknown -p pointblitz-web
wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/pointblitz_web.wasm
ls -l web/pkg
