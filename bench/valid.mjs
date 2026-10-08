// Single-run validity check (decisions 0038, 0040, 0041), for redoing an invalid run at once.
//
// usage: node bench/valid.mjs <dir> <run>      prints "ok ..." or the reasons, exit 0 / 1
// C1, C3 and C4 need only the run's own files (<run>.jsonl, .gpu.csv, .cpu.log). C2 compares a run
// with the session's other runs, so it stays with bench/contamination.mjs at the end of a suite.

import fs from 'node:fs';
import path from 'node:path';

const [dir, run] = process.argv.slice(2);
const ref = JSON.parse(fs.readFileSync(new URL('./cpu-reference.json', import.meta.url), 'utf8')).iters_per_s;
const recs = fs.readFileSync(path.join(dir, `${run}.jsonl`), 'utf8').trim().split('\n').filter(Boolean).map((l) => JSON.parse(l));
const top = (m) => recs.find((x) => x.metric === m && x.seq == null && x.view == null)?.value;
const why = [];

const gpu = path.join(dir, `${run}.gpu.csv`);
const before = fs.existsSync(gpu)
  ? fs.readFileSync(gpu, 'utf8').split('\n').filter((l) => l.startsWith('before,')).map((l) => Number(l.split(',')[2]))
  : [];
const util = before.length ? before.reduce((a, b) => a + b, 0) / before.length : NaN;
if (util > 15) why.push(`C1 before ${util.toFixed(1)} %`);

const hz = top('display_hz');
if (!(hz >= 59.5 && hz <= 60.5)) why.push(`C3 ${hz ?? 'no display_hz'}`);

const cpuFile = path.join(dir, `${run}.cpu.log`);
const cal = fs.existsSync(cpuFile)
  ? fs.readFileSync(cpuFile, 'utf8').replace(/^﻿/, '').split('\n').filter((l) => l.trim()).map((l) => JSON.parse(l)).find((x) => x.type === 'cpu_calibration')?.iters_per_s
  : undefined;
if (!(cal >= 0.9 * ref)) why.push(`C4 cpu ${cal ?? 'no calibration'}`);
const late = top('server_late_ticks_pct');
if (late > 1) why.push(`C4 late ticks ${late.toFixed(2)} %`);

console.log(why.length ? why.join('; ') : `ok ${hz.toFixed(2)} Hz, cpu ${cal}`);
process.exit(why.length ? 1 : 0);
