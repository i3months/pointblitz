// Server-video frame sizes around snapshot swaps (cost model, PR #36 review ①), from raw files of
// run.mjs --target video --raw (frameTimes carry the message size since decision 0040).
//
// usage: node bench/video/frame-bytes.mjs <raw.json> [...]
// Swap frame = the first frame that shows a snapshot (its 'presented' mark). Still = drawn frames
// with no swap within ±5 frames. Size = the WebSocket message (metadata + Annex B). The frame number
// of a drawn frame is its server tick time × 60 / 1000 (serve ticks at a fixed 60 fps).

import fs from 'node:fs';

const pct = (a, q) => {
  const s = [...a].sort((x, y) => x - y);
  return s.length ? s[Math.round((s.length - 1) * q)] : NaN;
};
const kib = (b) => `${(b / 1024).toFixed(1)} KiB`;

const swaps = [];
const still = [];
for (const file of process.argv.slice(2)) {
  const r = JSON.parse(fs.readFileSync(file, 'utf8'));
  const drawn = new Map();
  for (const [tMs, , , size] of r.frameTimes ?? []) if (size != null) drawn.set(Math.round((tMs * 60) / 1000), size);
  if (!drawn.size) {
    console.log(`${file}: no frame sizes (raw from before decision 0040)`);
    continue;
  }
  const swapFrames = [...new Set(r.marks.filter((m) => m.name === 'presented').map((m) => m.frame))];
  for (const n of swapFrames) if (drawn.has(n)) swaps.push(drawn.get(n));
  for (const [n, size] of drawn) if (!swapFrames.some((s) => Math.abs(s - n) <= 5)) still.push(size);
}
if (swaps.length) {
  console.log(`swap frames: ${swaps.length}, p50 ${kib(pct(swaps, 0.5))}, max ${kib(Math.max(...swaps))}`);
  console.log(`still frames: ${still.length}, p50 ${kib(pct(still, 0.5))}, p95 ${kib(pct(still, 0.95))}`);
}
