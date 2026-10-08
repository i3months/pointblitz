#!/usr/bin/env bash
# Comparison suite (P1.5, P2.5): the three.js baseline and the PointBlitz targets, alternated run by run in one
# session (PR #10 review — sessions drift, so both implementations must share one).
#
# usage: bash bench/suite.sh <ply dir> <out dir>        (run from the repository root)
# env:   COLD=5 ORBIT=5 X60=3 X1=0 PORT=8783 IMPLS="three native web webs webgl video" B1_UPLOAD=a ORBIT_BATCHES="1"
#        ORBIT_BATCHES="30 1" = each orbit run once per batch size (decision 0049), batch 30 named <impl>-orbitb30-<i>
#        three-b1 / three-b1-webgpu / three-b2 = incremental three.js baselines (decision 0048),
#        three-b1a / three-b1b = B1 with upload (a) / (b), for the upload smoke
#        web = PointBlitz in Chrome on WebGPU, webs = same with the WebGPU-only module (decision 0033),
#        webgl = same page with WebGPU switched off (WebGL2), video = server video (serve + WebCodecs client)
#
# Starts a renamed copy of the replay server and stops it by PID. Writes <impl>-<scenario>-<i>.jsonl
# and the aggregated table (table.md).
#
# Validity (decisions 0038, 0040, 0041): every run goes through bench/gpu-watch.mjs (which starts
# bench/measure-env.ps1). A run that fails, or is invalid under C1/C3/C4 (bench/valid.mjs), is redone
# once with the same settings; the first attempt's files are kept as <run>.try1-failed.* /
# <run>.try1-invalid.* and every attempt is listed in runs.log. C2 is checked at the end
# (bench/contamination.mjs --apply).
set -euo pipefail

DATA=${1:?ply dir}
OUT=${2:?out dir}
COLD=${COLD:-5}
ORBIT=${ORBIT:-5}
X60=${X60:-3}
X1=${X1:-0}
PORT=${PORT:-8783}
IMPLS=${IMPLS:-three native web webs webgl video}
ORBIT_BATCHES=${ORBIT_BATCHES:-1}
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
# (`|| true`: under set -e a failing kill in the EXIT trap made the suite exit 1 after a complete run.)
stop_server() { local w; w=$(cat "/proc/$SERVER/winpid" 2>/dev/null || true); kill "$SERVER" 2>/dev/null || true; if [ -n "$w" ]; then taskkill //F //PID "$w" > /dev/null 2>&1 || true; fi; }
trap stop_server EXIT
sleep 3 # the server reads every snapshot once at startup (decision 0026)

URL="http://127.0.0.1:$PORT"
run() { # impl scenario speed index [batch]
  local impl=$1 scen=$2 speed=$3 i=$4 batch=${5:-1} name cmd
  name="$impl-$scen$( [ "$scen" = replay ] && echo "$speed" || true )$( [ "$batch" -gt 1 ] && echo "b$batch" || true )-$i"
  echo "$(date +%H:%M:%S) $name"
  case $impl in
    three) cmd=(node baseline/three/run.mjs --server "$URL" --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    # Incremental three.js baselines (decision 0048). B1_UPLOAD picks B1's upload (smoke: a vs b).
    three-b1) cmd=(node baseline/three/run.mjs --server "$URL" --label three-b1 --query "mode=b1&upload=${B1_UPLOAD:-a}" --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    three-b1a) cmd=(node baseline/three/run.mjs --server "$URL" --label three-b1a --query "mode=b1&upload=a" --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    three-b1b) cmd=(node baseline/three/run.mjs --server "$URL" --label three-b1b --query "mode=b1&upload=b" --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    three-b1-webgpu) cmd=(node baseline/three/run.mjs --server "$URL" --label three-b1-webgpu --query "mode=b1&upload=${B1_UPLOAD:-a}&renderer=webgpu" --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    three-b1-forcewebgl) cmd=(node baseline/three/run.mjs --server "$URL" --label three-b1-forcewebgl --query "mode=b1&upload=${B1_UPLOAD:-a}&renderer=webgpu&forcewebgl=1" --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;; # diagnosis (#72): instanced quads on WebGL2
    three-b2) cmd=(node baseline/three/run.mjs --server "$URL" --label three-b2 --query "mode=b2" --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    web) cmd=(node baseline/three/run.mjs --server "$URL" --target web --label web-webgpu --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    webs) cmd=(node baseline/three/run.mjs --server "$URL" --target web --label web-webgpu-only --query pkg=webgpu --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    webgl) cmd=(node baseline/three/run.mjs --server "$URL" --target web --label web-webgl2 --chrome-args disable-features=WebGPUService --scenario "$scen" --speed "$speed" --metrics "$ABS_OUT/$name.jsonl") ;;
    native) cmd=(node bench/native/run.mjs --server "$URL" --scenario "$scen" --speed "$speed" --exe "target/release/pointblitz-native$EXT" --metrics "$ABS_OUT/$name.jsonl") ;;
    video) cmd=(node bench/video/run.mjs --server "$URL" --port "$VIDEO_PORT" --scenario "$scen" --speed "$speed" --exe "target/release/pb-suite-serve$EXT" --metrics "$ABS_OUT/$name.jsonl" --log "$ABS_OUT/$name.server.log") ;;
  esac
  # orbit with batch > 1 (decision 0049): N frames per sync; browser pages take it as a page parameter.
  if [ "$batch" -gt 1 ]; then
    local k q=-1
    for k in "${!cmd[@]}"; do [ "${cmd[$k]}" = --query ] && q=$((k + 1)); done
    if [ "$impl" = native ]; then cmd+=(--batch "$batch")
    elif [ "$q" -ge 0 ]; then cmd[$q]="${cmd[$q]}&batch=$batch"
    else cmd+=(--query "batch=$batch"); fi
  fi
  # GPU and CPU state and display clock for every run; one redo of a failed or invalid run.
  local try r
  for try in 1 2; do
    if ! node bench/gpu-watch.mjs --out "$ABS_OUT/$name.gpu.csv" -- "${cmd[@]}" > /dev/null 2>> "$ABS_OUT/errors.log"; then
      r="failed"
    elif r=$(node bench/valid.mjs "$ABS_OUT" "$name"); then
      echo "$(date +%H:%M:%S) $name try $try: $r" >> "$ABS_OUT/runs.log"
      return 0
    else
      r="invalid ($r)"
    fi
    echo "$(date +%H:%M:%S) $name try $try: $r" >> "$ABS_OUT/runs.log"
    local f
    for f in "$ABS_OUT/$name".*; do
      case $f in *"/$name.try"[0-9]*) continue ;; esac # an earlier attempt, already kept
      [ -e "$f" ] && mv "$f" "${f/$name./$name.try$try-${r%% *}.}"
    done
  done
  return 0
}
for i in $(seq 1 "$COLD"); do for impl in $IMPLS; do run "$impl" cold 60 "$i"; done; done
for i in $(seq 1 "$ORBIT"); do for impl in $IMPLS; do for b in $ORBIT_BATCHES; do run "$impl" orbit 60 "$i" "$b"; done; done; done
for i in $(seq 1 "$X60"); do for impl in $IMPLS; do run "$impl" replay 60 "$i"; done; done
for i in $(seq 1 "$X1"); do for impl in $IMPLS; do run "$impl" replay 1 "$i"; done; done
node bench/contamination.mjs "$ABS_OUT" --apply
node baseline/three/aggregate.mjs "$ABS_OUT" --md "$ABS_OUT/table.md"
