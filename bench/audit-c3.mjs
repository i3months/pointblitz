// Audits past suite directories against rule C3 (frame clock 59.5–60.5 Hz, decision 0040).
//
// usage: node bench/audit-c3.mjs <suite dir> [...]
// Per run, the best evidence available: the raw file (<run>.raw.json → display_hz, the worst of four
// time-ordered quarters, catches a clock that changes part-way), else display_hz in the metrics file,
// else 1000 / frame_interval_p50 (median only: misses a change late in the run). Runs with no frame
// clock record at all (native without --continuous) are listed as "no clock data".

import fs from 'node:fs';
import path from 'node:path';
import { summarize } from '../baseline/three/metrics.mjs';

const C3_HZ = [59.5, 60.5];
const read = (f) => fs.readFileSync(f, 'utf8').trim().split('\n').filter(Boolean).map((l) => JSON.parse(l));

for (const dir of process.argv.slice(2)) {
  const runs = fs.readdirSync(dir).filter((f) => f.endsWith('.jsonl')).map((f) => f.slice(0, -'.jsonl'.length)).sort();
  const rows = [];
  for (const run of runs) {
    const recs = read(path.join(dir, `${run}.jsonl`));
    const top = (m) => recs.find((x) => x.metric === m && x.seq == null && x.view == null);
    const target = recs[0]?.target ?? '?';
    let hz = NaN;
    let how = 'no clock data';
    const rawFile = path.join(dir, `${run}.raw.json`);
    if (fs.existsSync(rawFile)) {
      const r = summarize(JSON.parse(fs.readFileSync(rawFile, 'utf8')), { device: '', commit: '', target }).find((x) => x.metric === 'display_hz');
      if (r) [hz, how] = [r.value, 'raw quarters'];
    }
    if (!Number.isFinite(hz) && top('display_hz')) [hz, how] = [top('display_hz').value, 'display_hz'];
    if (!Number.isFinite(hz) && top('frame_interval_p50')) [hz, how] = [1000 / top('frame_interval_p50').value, 'median only'];
    const bad = Number.isFinite(hz) && (hz < C3_HZ[0] || hz > C3_HZ[1]);
    rows.push({ run, target, hz, how, bad });
  }
  const bad = rows.filter((r) => r.bad);
  const none = rows.filter((r) => !Number.isFinite(r.hz));
  const hz = rows.filter((r) => Number.isFinite(r.hz)).map((r) => r.hz);
  const range = hz.length ? `${Math.min(...hz).toFixed(2)}–${Math.max(...hz).toFixed(2)} Hz` : '—';
  const hows = [...new Set(rows.filter((r) => Number.isFinite(r.hz)).map((r) => r.how))].join(', ') || '—';
  console.log(`${dir}: ${rows.length} runs, outside C3 ${bad.length}, no clock data ${none.length} (${[...new Set(none.map((r) => r.target))].join(', ') || '—'}), clock ${range} [${hows}]`);
  for (const r of bad) console.log(`  OUT  ${r.run} (${r.target}) ${r.hz.toFixed(2)} Hz [${r.how}]`);
}
