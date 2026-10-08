#!/usr/bin/env bash
# Comparison suite (P1.5, P2.5): the three.js baseline and the PointBlitz targets, alternated run by run in one
# session (PR #10 review — sessions drift, so both implementations must share one).
#
# usage: bash bench/suite.sh <ply dir> <out dir>        (run from the repository root)
# env:   COLD=5 ORBIT=5 X60=3 X1=0 PORT=8783 IMPLS="three native web webs webgl video"
#        web = PointBlitz in Chrome on WebGPU, webs = same with the WebGPU-only module (decision 0033),
#        webgl = same page with WebGPU switched off (WebGL2), video = server video (serve + WebCodecs client)
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
IMPLS=${IMPLS:-three native web webs webgl video}
VIDEO_PORT=$((PORT + 100))

cargo build --release -q -p pointblitz-bench -p pointblitz-native -p pointblitz-server
cp "target/release/pointblitz-server$( [ -f target/release/pointblitz-server.exe ] && echo .exe || true )" "target/release/pb-suite-serve$( [ -f target/release/pointblitz-server.exe ] && echo .exe || true )"
bash web/build.sh > /dev/null
bash web/build.sh webgpu > /dev/null
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
  local impl=$1 scen=$2 speed=$3 i=$4 name cmd
  name="$impl-$scen$( [ "$scen" = replay ] && echo "$speed" || true )-$i"
  echo "$(date +%H:%M:%S) $name"
  case $impl in
    three) cmd=(node baseline/three/run.mjs --server "$URL" --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    web) cmd=(node baseline/three/run.mjs --server "$URL" --target web --label web-webgpu --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    webs) cmd=(node baseline/three/run.mjs --server "$URL" --target web --label web-webgpu-only --query pkg=webgpu --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    webgl) cmd=(node baseline/three/run.mjs --server "$URL" --target web --label web-webgl2 --chrome-args disable-features=WebGPUService --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    native) cmd=(node bench/native/run.mjs --server "$URL" --scenario "$scen" --speed "$speed" --exe "target/release/pointblitz-native$EXT" --metrics "$ABS_OUT/$name.jsonl") ;;
    video) cmd=(node bench/video/run.mjs --server "$URL" --port "$VIDEO_PORT" --scenario "$scen" --speed "$speed" --exe "target/release/pb-suite-serve$EXT" --metrics "$ABS_OUT/$name.jsonl" --log "$ABS_OUT/$name.server.log") ;;
  esac
  # GPU state before and during every run (decision 0038).
  node bench/gpu-watch.mjs --out "$ABS_OUT/$name.gpu.csv" -- "${cmd[@]}" > /dev/null
}
for i in $(seq 1 "$COLD"); do for impl in $IMPLS; do run "$impl" cold 60 "$i"; done; done
for i in $(seq 1 "$ORBIT"); do for impl in $IMPLS; do run "$impl" orbit 60 "$i"; done; done
for i in $(seq 1 "$X60"); do for impl in $IMPLS; do run "$impl" replay 60 "$i"; done; done
for i in $(seq 1 "$X1"); do for impl in $IMPLS; do run "$impl" replay 1 "$i"; done; done
node baseline/three/aggregate.mjs "$ABS_OUT" --md "$ABS_OUT/table.md"
