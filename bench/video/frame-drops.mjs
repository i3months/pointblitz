// P4.3: where server-video frame drops come from (raw files from run.mjs --target video --raw).
//
// usage: node bench/video/frame-drops.mjs <raw.json> [...]
// For each run: display cycles by frames drawn (0 / 1 / 2+), how many empty cycles are followed by
// a double (same frames, uneven timing — a beat between the server's tick clock and the display's
// vsync) versus not made up (frames lost), and the jitter of server ticks, arrivals and draws.

import fs from 'node:fs';

const pct = (a, q) => {
  const s = [...a].sort((x, y) => x - y);
  return s.length ? s[Math.round((s.length - 1) * q)] : NaN;
};
const diffs = (a) => a.slice(1).map((x, i) => x - a[i]);
const f = (x) => x.toFixed(2);

for (const file of process.argv.slice(2)) {
  const r = JSON.parse(fs.readFileSync(file, 'utf8'));
  const draws = r.cycleDraws ?? [];
  const n = draws.length;
  const empty = draws.filter((d) => d === 0).length;
  const doubles = draws.filter((d) => d >= 2).length;
  let madeUp = 0;
  for (let i = 0; i + 1 < n; i++) if (draws[i] === 0 && draws[i + 1] >= 2) madeUp++;
  const frames = r.frameTimes ?? [];
  const tick = diffs(frames.map((x) => x[0]));
  const rx = diffs(frames.map((x) => x[1]));
  const drawn = diffs(frames.map((x) => x[2]));
  const totalDrawn = draws.reduce((a, b) => a + b, 0);
  console.log(`== ${file}`);
  console.log(`cycles ${n}: empty ${empty} (${f((100 * empty) / n)} %), double ${doubles}, frames drawn ${totalDrawn} (${f(totalDrawn / n)} per cycle)`);
  console.log(`empty cycles followed by a double: ${madeUp} of ${empty} (${f((100 * madeUp) / Math.max(empty, 1))} %)`);
  for (const [name, d] of [['server tick', tick], ['arrival', rx], ['draw', drawn]]) {
    console.log(`${name.padEnd(12)} interval p50 ${f(pct(d, 0.5))} p5 ${f(pct(d, 0.05))} p95 ${f(pct(d, 0.95))} max ${f(Math.max(...d))} ms`);
  }
}
