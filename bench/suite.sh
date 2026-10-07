#!/usr/bin/env bash
# P1.5 comparison suite: three.js baseline and PointBlitz native, alternated run by run in one
# session (PR #10 review — sessions drift, so both implementations must share one).
#
# usage: bash bench/suite.sh <ply dir> <out dir>        (run from the repository root)
# env:   COLD=5 ORBIT=5 X60=3 X1=0 PORT=8783 IMPLS="three native"
#
# Starts a renamed copy of the replay server and stops it by PID. Writes <impl>-<scenario>-<i>.jsonl
# and the aggregated table (table.md).
set -euo pipefail

DATA=${1:?ply dir}
OUT=${2:?out dir}
COLD=${COLD:-5}
ORBIT=${ORBIT:-5}
X60=${X60:-3}
X1=${X1:-0}
PORT=${PORT:-8783}
IMPLS=${IMPLS:-three native}

cargo build --release -q -p pointblitz-bench -p pointblitz-native
EXT=$( [ -f target/release/pointblitz-bench.exe ] && echo .exe || true )
BIN=target/release/pb-suite-replay$EXT
cp "target/release/pointblitz-bench$EXT" "$BIN"
mkdir -p "$OUT"
ABS_OUT=$(cd "$OUT" && pwd)
"$BIN" replay --data "$DATA" --web . --port "$PORT" > "$ABS_OUT/server.log" 2> "$ABS_OUT/server.err" &
SERVER=$!
# Git Bash: $! is an MSYS pid; stop the Windows process by its own pid as well.
stop_server() { local w; w=$(cat "/proc/$SERVER/winpid" 2>/dev/null); kill "$SERVER" 2>/dev/null; [ -n "$w" ] && taskkill //F //PID "$w" > /dev/null 2>&1; true; }
trap stop_server EXIT
sleep 3 # the server reads every snapshot once at startup (decision 0026)

URL="http://127.0.0.1:$PORT"
run() { # impl scenario speed index
  local impl=$1 scen=$2 speed=$3 i=$4 name
  name="$impl-$scen$( [ "$scen" = replay ] && echo "$speed" || true )-$i"
  echo "$(date +%H:%M:%S) $name"
  case $impl in
    three) (cd baseline/three && node run.mjs --server "$URL" --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl" > /dev/null) ;;
    native) node bench/native/run.mjs --server "$URL" --scenario "$scen" --speed "$speed" --exe "target/release/pointblitz-native$EXT" --metrics "$ABS_OUT/$name.jsonl" > /dev/null ;;
  esac
}
for i in $(seq 1 "$COLD"); do for impl in $IMPLS; do run "$impl" cold 60 "$i"; done; done
for i in $(seq 1 "$ORBIT"); do for impl in $IMPLS; do run "$impl" orbit 60 "$i"; done; done
for i in $(seq 1 "$X60"); do for impl in $IMPLS; do run "$impl" replay 60 "$i"; done; done
for i in $(seq 1 "$X1"); do for impl in $IMPLS; do run "$impl" replay 1 "$i"; done; done
node baseline/three/aggregate.mjs "$ABS_OUT" --md "$ABS_OUT/table.md"
