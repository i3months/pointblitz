#!/usr/bin/env bash
# P0.6 baseline measurement suite (docs/bench/baseline-three.md).
#
# usage: bash baseline/three/suite.sh <ply dir> <out dir>        (run from the repository root)
# env:   COLD=5 ORBIT=5 X60=3 SKIP_X1=0 PORT=8782
#
# Starts a renamed copy of the replay server (so other sessions' servers are not affected), stops
# it by PID, runs every scenario, and prints the aggregated table.
set -euo pipefail

DATA=${1:?ply dir}
OUT=${2:?out dir}
COLD=${COLD:-5}
ORBIT=${ORBIT:-5}
X60=${X60:-3}
SKIP_X1=${SKIP_X1:-0}
PORT=${PORT:-8782}

cargo build --release -q -p pointblitz-bench
BIN=target/release/pb-suite-replay$( [ -f target/release/pointblitz-bench.exe ] && echo .exe )
cp target/release/pointblitz-bench$( [ -f target/release/pointblitz-bench.exe ] && echo .exe ) "$BIN"
mkdir -p "$OUT"
"$BIN" replay --data "$DATA" --web . --port "$PORT" > "$OUT/server.log" 2>&1 &
SERVER=$!
# Git Bash: $! is an MSYS pid; stop the Windows process by its own pid as well.
stop_server() { local w; w=$(cat "/proc/$SERVER/winpid" 2>/dev/null); kill "$SERVER" 2>/dev/null; [ -n "$w" ] && taskkill //F //PID "$w" > /dev/null 2>&1; true; }
trap stop_server EXIT
sleep 1

ABS_OUT=$(cd "$OUT" && pwd)
URL="http://127.0.0.1:$PORT"
cd baseline/three
for i in $(seq 1 "$COLD"); do node run.mjs --server "$URL" --scenario cold --metrics "$ABS_OUT/cold-$i.jsonl" > /dev/null; done
for i in $(seq 1 "$ORBIT"); do node run.mjs --server "$URL" --scenario orbit --metrics "$ABS_OUT/orbit-$i.jsonl" > /dev/null; done
for i in $(seq 1 "$X60"); do node run.mjs --server "$URL" --scenario replay --speed 60 --metrics "$ABS_OUT/replay60-$i.jsonl" > /dev/null; done
if [ "$SKIP_X1" != "1" ]; then
  node run.mjs --server "$URL" --scenario replay --speed 1 --metrics "$ABS_OUT/replay1-1.jsonl" > /dev/null
fi
node aggregate.mjs "$ABS_OUT" --md "$ABS_OUT/table.md"
