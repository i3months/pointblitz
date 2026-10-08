// Flags contaminated runs in a suite directory (P3.4 validity rules, decision 0038).
//
// usage: node bench/contamination.mjs <suite dir> [--apply]
//   C1  mean GPU utilisation in the 2 s before the run > 15 %
//   C2  server video: render p50 or encode p50 > 2 × the median of the same scenario's video runs
// --apply moves each flagged run's files (<run>.jsonl, .gpu.csv, .server.log) into <dir>/contaminated/
// so the aggregate leaves them out; they stay on disk and are listed in the report.

import fs from 'node:fs';
import path from 'node:path';

const dir = process.argv[2];
const apply = process.argv.includes('--apply');
const C1_UTIL = 15;
const C2_FACTOR = 2;

const runs = fs.readdirSync(dir).filter((f) => f.endsWith('.jsonl')).map((f) => f.slice(0, -'.jsonl'.length));
const median = (v) => {
  const s = [...v].sort((a, b) => a - b);
  return s.length ? s[Math.floor((s.length - 1) / 2)] : NaN;
};

const info = runs.map((run) => {
  const r = { run, before: NaN, render: NaN, encode: NaN, scenario: run.replace(/^video-/, '').replace(/-\d+$/, '') };
  const gpu = path.join(dir, `${run}.gpu.csv`);
  if (fs.existsSync(gpu)) {
    const rows = fs.readFileSync(gpu, 'utf8').split('\n').filter((l) => l.startsWith('before,')).map((l) => Number(l.split(',')[2]));
    if (rows.length) r.before = rows.reduce((a, b) => a + b, 0) / rows.length;
  }
  const log = path.join(dir, `${run}.server.log`);
  if (fs.existsSync(log)) {
    const s = fs.readFileSync(log, 'utf8').trim().split('\n').map((l) => JSON.parse(l)).find((x) => x.type === 'summary');
    if (s) [r.render, r.encode] = [s.render_ms_p50, s.encode_ms_p50];
  }
  return r;
});

const byScenario = {};
for (const r of info.filter((x) => Number.isFinite(x.render))) (byScenario[r.scenario] ??= []).push(r);
const flagged = [];
for (const r of info) {
  const why = [];
  if (r.before > C1_UTIL) why.push(`C1 before ${r.before.toFixed(1)} % > ${C1_UTIL} %`);
  const peers = byScenario[r.scenario];
  if (peers && Number.isFinite(r.render)) {
    const mr = median(peers.map((p) => p.render));
    const me = median(peers.map((p) => p.encode));
    if (r.render > C2_FACTOR * mr) why.push(`C2 render p50 ${r.render.toFixed(2)} > 2 × ${mr.toFixed(2)} ms`);
    if (r.encode > C2_FACTOR * me) why.push(`C2 encode p50 ${r.encode.toFixed(2)} > 2 × ${me.toFixed(2)} ms`);
  }
  if (why.length) flagged.push({ run: r.run, why });
}

console.log(`${runs.length} runs, ${flagged.length} contaminated`);
for (const f of flagged) console.log(`  ${f.run}: ${f.why.join('; ')}`);
fs.writeFileSync(path.join(dir, 'contamination.json'), JSON.stringify({ rules: { C1_UTIL, C2_FACTOR }, runs: info, flagged }, null, 2));
if (apply && flagged.length) {
  const dest = path.join(dir, 'contaminated');
  fs.mkdirSync(dest, { recursive: true });
  for (const f of flagged) {
    for (const ext of ['.jsonl', '.gpu.csv', '.server.log']) {
      const src = path.join(dir, f.run + ext);
      if (fs.existsSync(src)) fs.renameSync(src, path.join(dest, f.run + ext));
    }
  }
  console.log(`moved ${flagged.length} runs to ${dest}`);
}
