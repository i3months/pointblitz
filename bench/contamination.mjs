// Flags contaminated runs in a suite directory (P3.4 validity rules, decision 0038).
//
// usage: node bench/contamination.mjs <suite dir> [--apply]
//   C1  mean GPU utilisation in the 2 s before the run > 15 %
//   C2  server video: render p50 or encode p50 > 2 × the median of the same scenario's video runs
//   C3  frame clock outside 59.5–60.5 Hz (display_hz; runs from before it existed: 1000 / frame_interval_p50)
//       — a display turned off by the idle timeout slows it to ~56.6 Hz (decision 0040)
//   C4  CPU state (decision 0041): the calibration before the run (<run>.cpu.log, bench/measure-env.ps1)
//       below 90 % of bench/cpu-reference.json, or no calibration record; server video also: late ticks > 1 %
// --apply moves each flagged run's files (<run>.jsonl, .gpu.csv, .server.log) into <dir>/contaminated/
// so the aggregate leaves them out; they stay on disk and are listed in the report.

import fs from 'node:fs';
import path from 'node:path';

const dir = process.argv[2];
const apply = process.argv.includes('--apply');
const C1_UTIL = 15;
const C2_FACTOR = 2;
const C3_HZ = [59.5, 60.5];
const C4_CPU = 0.9;
const C4_LATE_PCT = 1;
const cpuRef = JSON.parse(fs.readFileSync(new URL('./cpu-reference.json', import.meta.url), 'utf8')).iters_per_s;

// First attempts the suite redid (<run>.tryN-*.jsonl) are kept for the record, not checked.
const runs = fs.readdirSync(dir).filter((f) => f.endsWith('.jsonl') && !/\.try\d+-/.test(f)).map((f) => f.slice(0, -'.jsonl'.length));
const median = (v) => {
  const s = [...v].sort((a, b) => a - b);
  return s.length ? s[Math.floor((s.length - 1) / 2)] : NaN;
};

const info = runs.map((run) => {
  const r = { run, before: NaN, render: NaN, encode: NaN, hz: NaN, cpu: NaN, late: NaN, scenario: run.replace(/^video-/, '').replace(/-\d+$/, '') };
  const recs = fs.readFileSync(path.join(dir, `${run}.jsonl`), 'utf8').trim().split('\n').filter(Boolean).map((l) => JSON.parse(l));
  const hz = recs.find((x) => x.metric === 'display_hz' && x.seq == null)?.value;
  const p50 = recs.find((x) => x.metric === 'frame_interval_p50' && x.seq == null)?.value;
  r.hz = hz ?? (p50 ? 1000 / p50 : NaN);
  r.late = recs.find((x) => x.metric === 'server_late_ticks_pct')?.value ?? NaN;
  const cpuFile = path.join(dir, `${run}.cpu.log`);
  if (fs.existsSync(cpuFile)) {
    // PowerShell 5.1 writes a byte order mark.
    const lines = fs.readFileSync(cpuFile, 'utf8').replace(/^﻿/, '').split('\n').filter((l) => l.trim());
    const cal = lines.map((l) => JSON.parse(l)).find((x) => x.type === 'cpu_calibration');
    if (cal) r.cpu = cal.iters_per_s;
  }
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
  if (r.hz < C3_HZ[0] || r.hz > C3_HZ[1]) why.push(`C3 frame clock ${r.hz.toFixed(2)} Hz outside ${C3_HZ.join('–')}`);
  if (!(r.cpu >= C4_CPU * cpuRef)) why.push(Number.isFinite(r.cpu) ? `C4 CPU calibration ${r.cpu} < ${C4_CPU} × ${cpuRef}` : 'C4 no CPU calibration record');
  if (r.late > C4_LATE_PCT) why.push(`C4 server late ticks ${r.late.toFixed(2)} % > ${C4_LATE_PCT} %`);
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
fs.writeFileSync(path.join(dir, 'contamination.json'), JSON.stringify({ rules: { C1_UTIL, C2_FACTOR, C3_HZ, C4_CPU, C4_LATE_PCT, cpuRef }, runs: info, flagged }, null, 2));
if (apply && flagged.length) {
  const dest = path.join(dir, 'contaminated');
  fs.mkdirSync(dest, { recursive: true });
  for (const f of flagged) {
    for (const ext of ['.jsonl', '.gpu.csv', '.cpu.log', '.server.log']) {
      const src = path.join(dir, f.run + ext);
      if (fs.existsSync(src)) fs.renameSync(src, path.join(dest, f.run + ext));
    }
  }
  console.log(`moved ${flagged.length} runs to ${dest}`);
}
