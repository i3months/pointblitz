#!/usr/bin/env bash
# Before/after comparison in one session (P4.11, decision 0050): two builds side by side, each with
# its own replay server, every target run alternately on both. The order flips every round so drift
# within the session does not favour one side.
#
# usage: bash bench/ab.sh <before repo> <ply dir> <out dir>        (run from the after repository root)
# env:   COLD=10 X60=10 IMPLS="web webgl native video" PORT=8871 VIDEO_DATA=after|both
#        <before repo> is a worktree of the earlier commit, built here (release binaries, web module).
#
# Names: <impl>-<before|after>-<scenario>-<i>. web and webgl runs also keep their raw marks (<name>.raw.json)
# for the stage split. The video server of the after build reads the snapshots locally (--data);
# the before build fetches /chunks as it did. Validity and redo are the same as bench/suite.sh
# (gpu-watch, C1/C3/C4 per run, one redo, C2 at the end). The browser receive floor (a 40 MB static
# file over the same kind of server) is measured at the start and the end of the session.
set -euo pipefail

BEFORE=$(cd "${1:?before repo}" && pwd)
DATA=${2:?ply dir}
OUT=${3:?out dir}
COLD=${COLD:-10}
X60=${X60:-10}
IMPLS=${IMPLS:-web webgl native video}
PORT=${PORT:-8871}
AFTER=$(pwd)
EXT=$( [ -f target/release/pointblitz-bench.exe ] && echo .exe || true )

build() { # repo
  (cd "$1" && cargo build --release -q -p pointblitz-bench -p pointblitz-native -p pointblitz-server && bash web/build.sh > /dev/null)
  cp "$1/target/release/pointblitz-bench$EXT" "$1/target/release/pb-ab-replay$EXT"
  cp "$1/target/release/pointblitz-server$EXT" "$1/target/release/pb-ab-serve$EXT"
}
build "$BEFORE"
build "$AFTER"
mkdir -p "$OUT"
ABS_OUT=$(cd "$OUT" && pwd)
# 40 MB file under the after tree's web root for the receive floor.
head -c 40033040 /dev/zero > target/pb-ab-floor.bin

declare -A URL PIDS
start() { # side repo port
  "$2/target/release/pb-ab-replay$EXT" replay --data "$DATA" --web "$2" --port "$3" > "$ABS_OUT/server-$1.log" 2> "$ABS_OUT/server-$1.err" &
  PIDS[$1]=$!
  URL[$1]="http://127.0.0.1:$3"
}
stop_servers() {
  local s w
  for s in "${!PIDS[@]}"; do
    w=$(cat "/proc/${PIDS[$s]}/winpid" 2>/dev/null || true)
    kill "${PIDS[$s]}" 2>/dev/null || true
    if [ -n "$w" ]; then taskkill //F //PID "$w" > /dev/null 2>&1 || true; fi
  done
}
trap stop_servers EXIT
start before "$BEFORE" "$PORT"
start after "$AFTER" $((PORT + 1))
sleep 4

floor() { # label
  node bench/fetch-floor.mjs --server "${URL[after]}" --path static/target/pb-ab-floor.bin --times 5 > "$ABS_OUT/floor-$1.json" 2>> "$ABS_OUT/errors.log" || true
}
floor start

run() { # impl side scenario index
  local impl=$1 side=$2 scen=$3 i=$4 repo name cmd speed=60
  repo=$( [ "$side" = before ] && echo "$BEFORE" || echo "$AFTER" )
  name="$impl-$side-$scen$( [ "$scen" = replay ] && echo 60 || true )-$i"
  echo "$(date +%H:%M:%S) $name"
  case $impl in
    web) cmd=(node baseline/three/run.mjs --server "${URL[$side]}" --target web --label "web-$side" --scenario "$scen" --speed $speed --metrics "$ABS_OUT/$name.jsonl" --raw "$ABS_OUT/$name.raw.json") ;;
    webgl) cmd=(node baseline/three/run.mjs --server "${URL[$side]}" --target web --label "webgl-$side" --chrome-args disable-features=WebGPUService --scenario "$scen" --speed $speed --metrics "$ABS_OUT/$name.jsonl" --raw "$ABS_OUT/$name.raw.json") ;;
    native) cmd=(node bench/native/run.mjs --server "${URL[$side]}" --scenario "$scen" --speed $speed --exe "$repo/target/release/pointblitz-native$EXT" --metrics "$ABS_OUT/$name.jsonl") ;;
    video)
      cmd=(node bench/video/run.mjs --server "${URL[$side]}" --port $((PORT + 100)) --scenario "$scen" --speed $speed --exe "$repo/target/release/pb-ab-serve$EXT" --metrics "$ABS_OUT/$name.jsonl" --log "$ABS_OUT/$name.server.log")
      # VIDEO_DATA=both: the before build reads locally as well (P4.14, both builds have decision 0050).
      if [ "$side" = after ] || [ "${VIDEO_DATA:-after}" = both ]; then cmd+=(--data "$DATA"); fi
      ;;
  esac
  local try r f
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
    for f in "$ABS_OUT/$name".*; do
      case $f in *"/$name.try"[0-9]*) continue ;; esac
      [ -e "$f" ] && mv "$f" "${f/$name./$name.try$try-${r%% *}.}"
    done
  done
  return 0
}
pair() { # impl scenario index
  if [ $(($3 % 2)) -eq 1 ]; then run "$1" before "$2" "$3"; run "$1" after "$2" "$3"
  else run "$1" after "$2" "$3"; run "$1" before "$2" "$3"; fi
}
for i in $(seq 1 "$COLD"); do for impl in $IMPLS; do pair "$impl" cold "$i"; done; done
for i in $(seq 1 "$X60"); do for impl in $IMPLS; do pair "$impl" replay "$i"; done; done
floor end
node bench/contamination.mjs "$ABS_OUT" --apply
