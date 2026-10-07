// Aggregates repeated runs into one table (P0.6): median and min–max per metric and scenario.
//
// usage: node aggregate.mjs <dir with *.jsonl> [--md <out.md>]
// Per-event records (event_latency with seq, frame_time_total_p50_view with view, swap_frame_gpu_wait)
// are grouped by seq/view as well.

import fs from 'node:fs';
import path from 'node:path';
import { parseArgs } from './args.mjs';
import { percentile } from './metrics.mjs';

const dir = process.argv[2];
const args = parseArgs(process.argv.slice(3));
const records = fs
  .readdirSync(dir)
  .filter((f) => f.endsWith('.jsonl'))
  .flatMap((f) =>
    fs
      .readFileSync(path.join(dir, f), 'utf8')
      .trim()
      .split('\n')
      .filter(Boolean)
      .map((l) => ({ ...JSON.parse(l), file: f })),
  );

const groups = new Map();
for (const r of records) {
  const scen = r.speed != null ? `${r.scenario}×${r.speed}` : r.scenario;
  const key = [scen, r.metric, r.seq ?? '', r.view ?? '', r.kind ?? ''].join('|');
  if (!groups.has(key)) groups.set(key, { ...r, scenario: scen, values: [], runs: new Set() });
  const g = groups.get(key);
  g.values.push(r.value);
  g.runs.add(r.file);
}

const rows = [...groups.values()].map((g) => {
  const s = [...g.values].sort((a, b) => a - b);
  return {
    scenario: g.scenario,
    metric: g.metric,
    detail: [g.seq != null ? `#${g.seq}` : '', g.kind ?? '', g.view ?? ''].filter(Boolean).join(' '),
    unit: g.unit,
    runs: g.runs.size,
    median: percentile(s, 0.5),
    min: s[0],
    max: s[s.length - 1],
  };
});
rows.sort((a, b) => a.scenario.localeCompare(b.scenario) || 0);

const fmt = (v) => (v == null ? '' : Number.isInteger(v) ? v.toLocaleString('en-US') : v.toFixed(2));
const lines = [
  '| 시나리오 | 지표 | 세부 | 실행 수 | 중앙값 | 최소 | 최대 | 단위 |',
  '|---|---|---|---:|---:|---:|---:|---|',
  ...rows.map((r) => `| ${r.scenario} | ${r.metric} | ${r.detail} | ${r.runs} | ${fmt(r.median)} | ${fmt(r.min)} | ${fmt(r.max)} | ${r.unit} |`),
];
const device = records[0]?.device;
const header = device
  ? `장비: ${device.renderer} · ${device.browser} · ${device.cpu} · ${device.os} · ${device.impl} · commit ${records[0].commit}\n\n`
  : '';
const out = header + lines.join('\n') + '\n';
if (args.md) fs.writeFileSync(args.md, out);
process.stdout.write(out);
