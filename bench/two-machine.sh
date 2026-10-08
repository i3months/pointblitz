#!/usr/bin/env bash
# Same headless measurements on any machine, for the cost model's second machine (decision 0043).
#
# usage: bash bench/two-machine.sh <ply dir> <out dir>        (run from the repository root)
# env:   RUNS=5 PORT=8790
#   ① chunks: replay server + GET /chunks/<last> RUNS times → server log "read / total ms" per delivery
#   ② serve:  `serve --orbit` + `probe` (headless client, no decoding) RUNS times → per-tick render
#             (draw + read back) and encode ms in <out>/serve-orbit-<i>.log
#   ③ encode: `encode-test` on the last snapshot (fixed viewpoints + orbit), QP 18
# Before each run it waits until the GPU is idle (nvidia-smi utilisation ≤ 15 % for 3 s, rule C1) and
# never touches other processes. Writes env.txt (GPU, driver, CPU, load) and runs.log.
set -euo pipefail

DATA=${1:?ply dir}
OUT=${2:?out dir}
RUNS=${RUNS:-5}
PORT=${PORT:-8790}
VPORT=$((PORT + 1))
EXT=$( [ -f target/release/pointblitz-bench.exe ] && echo .exe || true )
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
LAST=$(ls "$DATA"/*.ply | sort | tail -1)

cargo build --release -q -p pointblitz-bench -p pointblitz-server

{
  date -u +%FT%TZ
  nvidia-smi --query-gpu=name,driver_version,memory.total,clocks.max.graphics --format=csv,noheader
  # Environment record only: tools missing on a platform (uptime on Git Bash) must not stop the run.
  if [ -r /proc/cpuinfo ]; then grep -m1 "model name" /proc/cpuinfo || true; nproc || true; uptime 2>/dev/null || true; cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || true; fi
  if [ -n "${PROCESSOR_IDENTIFIER:-}" ]; then echo "$PROCESSOR_IDENTIFIER"; fi
  uname -a
} > "$OUT/env.txt" 2>&1

idle_gpu() { # wait until utilisation ≤ 15 % for 3 consecutive seconds
  local ok=0 u
  while [ "$ok" -lt 3 ]; do
    u=$(nvidia-smi --query-gpu=utilization.gpu --format=csv,noheader,nounits | head -1 | tr -d ' ')
    if [ "$u" -le 15 ]; then ok=$((ok + 1)); else ok=0; echo "$(date +%T) GPU busy ($u %), waiting" >> "$OUT/runs.log"; fi
    sleep 1
  done
}

target/release/pointblitz-bench$EXT replay --data "$DATA" --port "$PORT" > "$OUT/replay.log" 2> "$OUT/replay.err" &
REPLAY=$!
stop() { local w; w=$(cat "/proc/$REPLAY/winpid" 2>/dev/null || true); kill "$REPLAY" 2>/dev/null || true; if [ -n "$w" ]; then taskkill //F //PID "$w" > /dev/null 2>&1 || true; fi; }
trap stop EXIT
sleep 3
SEQ=$(ls "$DATA"/*.ply | wc -l | tr -d ' ')

# ① chunk conversion + loopback delivery of the last snapshot, as one full delivery.
for i in $(seq 1 "$RUNS"); do
  idle_gpu
  curl -s -o /dev/null "http://127.0.0.1:$PORT/chunks/$SEQ"
  echo "$(date +%T) chunks-$i done" >> "$OUT/runs.log"
done

# ② serve (cold load, then 720 orbit frames at 1.5°) with the headless client.
for i in $(seq 1 "$RUNS"); do
  idle_gpu
  target/release/pointblitz-server$EXT serve --replay "http://127.0.0.1:$PORT" --port "$VPORT" --orbit \
    --wait-for-client --log "$OUT/serve-orbit-$i.log" 2> "$OUT/serve-orbit-$i.err" &
  S=$!
  sleep 2
  target/release/pointblitz-server$EXT probe --url "ws://127.0.0.1:$VPORT" --expect-snapshots 1 --max-seconds 60 \
    > "$OUT/probe-orbit-$i.json" 2> "$OUT/probe-orbit-$i.err" || true
  wait "$S" || true
  echo "$(date +%T) serve-orbit-$i done" >> "$OUT/runs.log"
done

# ③ encoding alone.
idle_gpu
target/release/pointblitz-server$EXT encode-test --ply "$LAST" --viewpoints bench/viewpoints/flight-01.json \
  --out "$OUT/encode" --qp 18 > "$OUT/encode.out" 2> "$OUT/encode.err"
echo "$(date +%T) encode done" >> "$OUT/runs.log"
