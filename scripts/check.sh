#!/usr/bin/env bash
# Everything CI checks, runnable before opening a PR (PROJECT §2). Run from the repository root.
#
# Also checks the other OS's code paths from this machine, so a cfg(windows) / cfg(not(windows))
# split cannot break the other CI job unseen (PR #24 broke the Ubuntu build that way):
#   rustup target add x86_64-unknown-linux-gnu   (on Windows)   or   x86_64-pc-windows-msvc (on Linux)
set -euo pipefail

step() { printf '\n== %s\n' "$*"; }

step fmt
cargo fmt --all --check

step "clippy (host)"
cargo clippy --workspace --all-targets -- -D warnings

host=$(rustc -vV | sed -n 's/^host: //p')
case "$host" in
  *windows*) other=x86_64-unknown-linux-gnu ;;
  *) other=x86_64-pc-windows-msvc ;;
esac
if rustup target list --installed | grep -qx "$other"; then
  step "clippy ($other — the other CI OS, check only)"
  cargo clippy --workspace --all-targets --target "$other" -- -D warnings
else
  echo "warning: target $other not installed; the other OS's cfg paths are not checked (rustup target add $other)" >&2
fi

step "clippy (wasm32: web default, web WebGPU-only)"
cargo clippy -p pointblitz-web --target wasm32-unknown-unknown -- -D warnings
cargo clippy -p pointblitz-web --target wasm32-unknown-unknown --no-default-features -- -D warnings

step "crate boundaries (SPEC §3.1): the core has no network or JSON dependency"
if cargo tree -q -p pointblitz-core -e normal --prefix none | grep -E "^(serde_json|tungstenite|winit|wasm-bindgen) "; then
  echo "error: pointblitz-core depends on the above (SPEC §3.1, decision 0003)" >&2
  exit 1
fi

step test
cargo test --workspace

step "crates.io packages (decision 0044): license copies match the root, every published crate packages and builds"
for d in crates/*/; do
  if grep -q "^publish = false" "$d/Cargo.toml"; then continue; fi
  for f in LICENSE-MIT LICENSE-APACHE; do
    cmp -s "$f" "$d$f" || { echo "error: $d$f differs from ./$f (copy the root file)" >&2; exit 1; }
  done
done
cargo publish --workspace --exclude pointblitz-bench --dry-run --allow-dirty --quiet

step "browser client unit tests"
node --test web/chunks.test.mjs baseline/three/ply-parse.test.mjs

echo
echo "all checks passed"
