// P4.6 judgment table (docs/bench/comparison-plan.md): SPEC §7.1 targets applied to a C1–C4 suite.
//
// usage: node bench/comparison.mjs <suite dir> [--validity] [--previous <suite dir> --previous-label <text>]
// Prints the Markdown table: metric, target, goal, value (median, min–max, valid runs), verdict,
// previous value/verdict under the same goal. A cell with fewer than 3 valid runs is "유효 실행 부족".
// Runs the suite redid (<run>.tryN-*) and runs moved to contaminated/ are not read.

import fs from 'node:fs';
import path from 'node:path';

const argv = process.argv.slice(2);
const dir = argv[0];
const opt = (k) => (argv.includes(k) ? argv[argv.indexOf(k) + 1] : undefined);
const prevDir = opt('--previous');
const prevLabel = opt('--previous-label') ?? 'previous';

const CLIENT = ['native', 'web-webgpu', 'web-webgpu-only', 'web-webgl2'];
const BROWSER = ['web-webgpu', 'web-webgpu-only', 'web-webgl2'];
const VIDEO = ['video'];
const BASE = ['three.js'];

// [label, scenario, metrics (first found wins: screen-time first, then the older draw-time name),
//  targets, goal, unit]. goal: number (≤), 'record' (recorded only), or 'baseline' (three.js, no verdict).
const ROWS = [
  ['event_latency preview p50', 'replay×60', ['event_latency_preview_p50'], CLIENT, 50, 'ms'],
  ['event_latency refined p50', 'replay×60', ['event_latency_refined_p50'], CLIENT, 200, 'ms'],
  ['cold 마지막 스냅샷 event_latency', 'cold', ['event_latency_refined_p50'], CLIENT, 300, 'ms'],
  ['main_thread_block 횟수', 'replay×60', ['main_thread_block_count'], BROWSER, 0, ''],
  ['main_thread_block 횟수', 'cold', ['main_thread_block_count'], BROWSER, 0, ''],
  ['frame_time_total p50', 'orbit', ['frame_time_total_p50'], CLIENT, 1.7, 'ms'],
  ['frame_time_total p95', 'orbit', ['frame_time_total_p95'], CLIENT, 1.9, 'ms'],
  ['frame_time_total p99', 'orbit', ['frame_time_total_p99'], CLIENT, 2.0, 'ms'],
  ['mem_cpu 최대', 'replay×60', ['mem_cpu'], CLIENT, 'record', 'MB'],
  ['event_latency preview p50 (screen)', 'replay×60', ['event_screen_latency_preview_p50', 'event_latency_preview_p50'], VIDEO, 80, 'ms'],
  ['event_latency refined p50 (screen)', 'replay×60', ['event_screen_latency_refined_p50', 'event_latency_refined_p50'], VIDEO, 250, 'ms'],
  ['cold (screen)', 'cold', ['event_screen_latency_refined_p50', 'event_latency_refined_p50'], VIDEO, 350, 'ms'],
  ['입력 → 표시 p50 (screen)', 'orbit', ['input_screen_latency_p50', 'input_latency_p50'], VIDEO, 50, 'ms'],
  ['입력 → 표시 p95 (screen)', 'orbit', ['input_screen_latency_p95', 'input_latency_p95'], VIDEO, 80, 'ms'],
  ['입력 → 표시 p50 (screen)', 'replay×60', ['input_screen_latency_p50', 'input_latency_p50'], VIDEO, 50, 'ms'],
  ['입력 → 표시 p95 (screen)', 'replay×60', ['input_screen_latency_p95', 'input_latency_p95'], VIDEO, 80, 'ms'],
  ['영상 프레임 빠짐', 'orbit', ['video_frame_missed_pct'], VIDEO, 1, '%'],
  ['클라이언트 main_thread_block 횟수', 'replay×60', ['main_thread_block_count'], VIDEO, 0, ''],
  ['클라이언트 main_thread_block 횟수', 'cold', ['main_thread_block_count'], VIDEO, 0, ''],
  ['event_latency preview p50', 'replay×60', ['event_latency_preview_p50'], BASE, 'baseline', 'ms'],
  ['event_latency refined p50', 'replay×60', ['event_latency_refined_p50'], BASE, 'baseline', 'ms'],
  ['cold 마지막 스냅샷 event_latency', 'cold', ['event_latency_refined_p50'], BASE, 'baseline', 'ms'],
  ['frame_time_total p50 / p95 / p99', 'orbit', ['frame_time_total_p50'], BASE, 'baseline', 'ms'],
  ['main_thread_block 횟수', 'replay×60', ['main_thread_block_count'], BASE, 'baseline', ''],
  ['mem_cpu 최대', 'replay×60', ['mem_cpu'], BASE, 'baseline', 'MB'],
];

// Run-level values: { target → { scenario → { metric → [value per run] } } }.
function load(d) {
  const out = {};
  for (const f of fs.readdirSync(d).filter((f) => f.endsWith('.jsonl') && !/\.try\d+-/.test(f))) {
    for (const line of fs.readFileSync(path.join(d, f), 'utf8').trim().split('\n').filter(Boolean)) {
      const r = JSON.parse(line);
      if (r.seq != null || r.view != null || r.metric == null) continue;
      const scen = r.scenario === 'replay' ? `replay×${r.speed}` : r.scenario;
      ((out[r.target] ??= {})[scen] ??= {})[r.metric] ??= [];
      out[r.target][scen][r.metric].push(r.value);
    }
  }
  return out;
}
const median = (v) => {
  const s = [...v].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};
const fmt = (x, unit) => (unit === 'MB' ? (x / 1e6).toFixed(1) : Number.isInteger(x) ? String(x) : x.toFixed(x < 10 ? 2 : 1));

function cell(data, target, scen, metrics, goal, unit) {
  const m = metrics.find((k) => data[target]?.[scen]?.[k]?.length);
  const v = m ? data[target][scen][m] : [];
  if (!v.length) return { text: '—', verdict: '—' };
  const med = median(v);
  const note = m !== metrics[0] ? ` (${m}, 그린 시각)` : '';
  let text = `${fmt(med, unit)} (${fmt(Math.min(...v), unit)}–${fmt(Math.max(...v), unit)}, ${v.length} 회)${note}`;
  if (metrics[0] === 'video_frame_missed_pct') text += `; 1 % 초과 ${v.filter((x) => x > 1).length}/${v.length} 회`;
  if (metrics[0] === 'frame_time_total_p50' && goal === 'baseline') {
    const t = ['p50', 'p95', 'p99'].map((q) => median(data[target][scen][`frame_time_total_${q}`] ?? [NaN]));
    text = `${t.map((x) => fmt(x, unit)).join(' / ')} (${v.length} 회)`;
  }
  let verdict;
  if (goal === 'record') verdict = '기록만';
  else if (goal === 'baseline') verdict = '기준 방식';
  else if (v.length < 3) verdict = '유효 실행 부족';
  else verdict = med <= goal ? '통과' : '**미달**';
  return { text, verdict };
}

// Validity per target (decisions 0038, 0040, 0041): valid runs, redone runs, frame clock, CPU
// calibration, processes with power throttling off (from <run>.cpu.log).
function validity(d) {
  const files = fs.readdirSync(d);
  const rows = {};
  for (const f of files.filter((f) => f.endsWith('.jsonl') && !/\.try\d+-/.test(f))) {
    const run = f.slice(0, -'.jsonl'.length);
    const recs = fs.readFileSync(path.join(d, f), 'utf8').trim().split('\n').filter(Boolean).map((l) => JSON.parse(l));
    const t = recs[0]?.target ?? '?';
    const r = (rows[t] ??= { runs: 0, redone: 0, hz: [], cpu: [], unthrottled: [], failed: 0 });
    r.runs++;
    if (files.some((g) => g.startsWith(`${run}.try`) && g.endsWith('.jsonl'))) r.redone++;
    const hz = recs.find((x) => x.metric === 'display_hz' && x.seq == null)?.value;
    if (hz != null) r.hz.push(hz);
    const cpu = path.join(d, `${run}.cpu.log`);
    if (fs.existsSync(cpu)) {
      const lines = fs.readFileSync(cpu, 'utf8').replace(/^﻿/, '').split('\n').filter((l) => l.trim()).map((l) => JSON.parse(l));
      const cal = lines.find((x) => x.type === 'cpu_calibration');
      if (cal) r.cpu.push(cal.iters_per_s);
      const perf = lines.filter((x) => x.type === 'cpu_perf');
      if (perf.length) {
        r.unthrottled.push(Math.max(...perf.map((x) => x.unthrottled)));
        r.failed += Math.max(...perf.map((x) => x.failed));
      }
    }
  }
  const span = (v, k = 2) => (v.length ? `${Math.min(...v).toFixed(k)}–${Math.max(...v).toFixed(k)}` : '—');
  const contaminated = fs.existsSync(path.join(d, 'contaminated')) ? fs.readdirSync(path.join(d, 'contaminated')).filter((f) => f.endsWith('.jsonl')).length : 0;
  console.log('| 대상 | 유효 실행 | 다시 돌린 실행 | 화면 시계(C3) | CPU 보정(C4, 회/s) | 전원 조절 끈 프로세스(실행마다) |');
  console.log('|---|---:|---:|---|---|---|');
  for (const [t, r] of Object.entries(rows).sort()) {
    console.log(`| ${t} | ${r.runs} | ${r.redone} | ${span(r.hz)} Hz | ${span(r.cpu, 0)} | ${span(r.unthrottled, 0)}${r.failed ? ` (실패 ${r.failed})` : ''} |`);
  }
  console.log(`\nC2 로 뺀 실행(contaminated/): ${contaminated} 회.\n`);
}

const now = load(dir);
if (argv.includes('--validity')) validity(dir);
const prev = prevDir ? load(prevDir) : null;
const head = ['지표', '시나리오', '대상', '목표', '값(중앙값, 최소–최대, 유효 실행)', '판정'];
if (prev) head.push(`${prevLabel}: 값 / 판정(같은 목표)`);
console.log(`| ${head.join(' | ')} |`);
console.log(`|${head.map(() => '---').join('|')}|`);
for (const [label, scen, metrics, targets, goal, unit] of ROWS) {
  for (const target of targets) {
    const c = cell(now, target, scen, metrics, goal, unit);
    const g = goal === 0 ? '0 회' : typeof goal === 'number' ? `≤ ${goal}${unit ? ` ${unit}` : ''}` : goal === 'record' ? '기록만' : '—';
    const row = [label, scen, target, g, c.text, c.verdict];
    if (prev) {
      const p = cell(prev, target, scen, metrics, goal, unit);
      row.push(p.text === '—' ? '—' : `${p.text} / ${p.verdict}`);
    }
    console.log(`| ${row.join(' | ')} |`);
  }
}
