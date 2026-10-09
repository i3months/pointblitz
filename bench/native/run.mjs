// Runs one PointBlitz native scenario and writes SPEC §6.2 metrics (P1.5).
//
// usage: node bench/native/run.mjs --server http://127.0.0.1:8700 --scenario replay|cold|orbit
//                                  [--speed 60] [--metrics <file.jsonl>] [--raw <file.json>]
//                                  [--exe target/release/pointblitz-native.exe] [--no-vsync]
//
// The native viewer writes marks (crates/native, decision 0026); this script turns them into the
// same raw shape the baseline page produces and runs the same summarize() (baseline/three/metrics.mjs),
// so both implementations are measured by one definition. mem_cpu = native process private bytes
// (SPEC §7.1), sampled by the same PowerShell sampler as Chrome's renderer process.

import { spawn, execSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { parseArgs } from '../../baseline/three/args.mjs';
import { percentile, summarize } from '../../baseline/three/metrics.mjs';
import { peakCommit, sampleProcesses } from '../../baseline/three/procmem.mjs';

const args = parseArgs();
const server = args.server ?? 'http://127.0.0.1:8700';
const scenario = args.scenario ?? 'replay';
const speed = Number(args.speed ?? 60);
const exe = args.exe ?? 'target/release/pointblitz-native.exe';
const marksPath = path.join(os.tmpdir(), `pb-native-marks-${process.pid}-${Date.now()}.jsonl`);

const cli = ['--server', server, '--scenario', scenario, '--speed', String(speed), '--marks', marksPath, '--exit-on-end', '--wait-before-exit'];
if (args['no-vsync']) cli.push('--no-vsync');
if (args.continuous) cli.push('--continuous');
if (args.batch) cli.push('--batch', String(args.batch));
if (args['memory-hints']) cli.push('--memory-hints', args['memory-hints']);

const t0 = Date.now();
const spawnAt = Date.now();
const child = spawn(exe, cli, { stdio: ['pipe', 'ignore', 'pipe'] });
const stopProc = sampleProcesses([child.pid]);
let stderr = '';
// The viewer prints `finished` and waits: read its OS peak commit while it exists (decision 0032).
let peak = 0;
child.stderr.on('data', (d) => {
  stderr += d;
  if (!peak && /^finished\r?$/m.test(stderr)) {
    peak = peakCommit([child.pid])[child.pid] ?? 0;
    child.stdin.end('\n');
  }
});
const code = await new Promise((resolve) => child.on('exit', resolve));
const samples = stopProc();
if (code !== 0) {
  process.stderr.write(stderr);
  throw new Error(`native exited with ${code}`);
}

const lines = fs.readFileSync(marksPath, 'utf8').trim().split('\n').map((l) => JSON.parse(l));
fs.unlinkSync(marksPath);
const by = (name) => lines.filter((m) => m.name === name);

// Same mark names as the baseline where they mean the same thing; fetch_end carries the bytes
// actually delivered (chunks, not PLY).
const marks = [
  ...by('snapshot_received'),
  ...by('submitted'),
  ...by('presented'),
  ...by('first_presented'), // first reflection of a coarse-first delivery (decision 0051)
  ...by('scene_swap'),
  ...by('sync_start'),
  ...by('sync_end'),
  ...by('delivered').map((d) => ({ name: 'fetch_end', t: d.t, seq: d.seq, bytes: d.bytes, delivery: d.delivery })),
].sort((a, b) => a.t - b.t);
const raw = {
  scenario,
  speed,
  marks,
  frames: by('frames')[0]?.t ?? [],
  presentClock: by('present_clock')[0]?.t ?? [],
  longtasks: [],
  syncFrames: by('sync_frame').map((f) => ({ view: f.view, cpu: f.cpu, ms: f.ms, gpu: f.gpu })),
  // Decision 0049: the native client writes pass timestamps when the adapter has TIMESTAMP_QUERY.
  gpuTimer: by('sync_frame').some((f) => f.gpu != null) ? 'wgpu TIMESTAMP_QUERY, render pass begin/end' : by('sync_frame').length ? 'none: adapter without TIMESTAMP_QUERY' : undefined,
  proc: { renderer: samples[child.pid] ?? [], gpu: [] },
  peak: { renderer: peak, method: 'OS peak commit of the native process (PeakPagefileUsage)' },
  native: { uploaded: by('uploaded'), delivered: by('delivered'), display: by('display')[0] },
  lines,
  wall_ms: Date.now() - t0,
};

const sh = (cmd) => {
  try {
    return execSync(cmd, { stdio: ['ignore', 'pipe', 'ignore'] }).toString().trim();
  } catch {
    return 'unknown';
  }
};
const commit = sh('git describe --always --dirty --abbrev=7');
const adapter = stderr.match(/^adapter: (.*)$/m)?.[1] ?? 'unknown';
const surface = stderr.match(/^surface: (.*)$/m)?.[1] ?? 'unknown';
const device = {
  impl: 'PointBlitz native (wgpu 30.0.1)',
  browser: '—',
  renderer: adapter,
  surface,
  gpu_driver: sh('nvidia-smi --query-gpu=driver_version --format=csv,noheader'),
  power_plan: sh('powercfg /getactivescheme').match(/[0-9a-f]{8}-[0-9a-f-]{27}/i)?.[0] ?? 'unknown',
  refresh_hz: raw.native.display?.refresh_hz ?? 'unknown',
  cpu: os.cpus()[0].model.trim(),
  os: `${os.type()} ${os.release()}`,
};

// Where the time goes, per snapshot (PR #12 review): the client stamps every boundary.
//   queue          received → request sent (previous delivery still running)
//   server_read    request → response headers (server reads the snapshot file)
//   server_stream  headers → last chunk received (server converts and sends; earlier chunks are
//                  uploaded meanwhile)
//   upload_tail    last chunk received → in the scene (validation + GPU buffer of the last chunk)
//   present        in the scene → presented (next frame + GPU finish)
const at = (name, seq) => lines.find((m) => m.name === name && m.seq === seq)?.t;
const stageNames = [
  ['queue', 'snapshot_received', 'fetch_start'],
  ['server_read', 'fetch_start', 'headers'],
  ['server_stream', 'headers', 'last_chunk_received'],
  ['upload_tail', 'last_chunk_received', 'uploaded'],
  ['present', 'uploaded', 'presented'],
];
const stageRecords = [];
for (const r of by('snapshot_received')) {
  for (const [name, a, b] of stageNames) {
    const ta = at(a, r.seq), tb = at(b, r.seq);
    if (ta != null && tb != null) stageRecords.push({ name, seq: r.seq, kind: r.kind, ms: tb - ta });
  }
  const up = by('uploaded').find((m) => m.seq === r.seq);
  if (up) stageRecords.push({ name: 'upload_total', seq: r.seq, kind: r.kind, ms: up.upload_ms });
}

const records = summarize(raw, { device, commit, target: 'native' })
  // Browser-only metric (SPEC §7.1); the JS-specific stage split has no native counterpart.
  .filter((r) => !r.metric.startsWith('main_thread_block') && !r.metric.startsWith('stage_'))
  // Frames are drawn on demand unless --continuous, so intervals only describe the continuous loop.
  .filter((r) => args.continuous || !r.metric.startsWith('frame_interval'))
  // The GPU figure is exact here: bytes of vertex buffers on the GPU after the last snapshot.
  .filter((r) => r.metric !== 'mem_gpu_estimate');
const up = raw.native.uploaded.at(-1);
if (up) {
  records.push({ ...records[0], metric: 'mem_gpu', value: up.gpu_bytes, unit: 'B', samples: 1, seq: undefined, kind: undefined, view: undefined, method: 'vertex buffer bytes after the last snapshot (16 B/point)' });
}
const base = { ...records[0], seq: undefined, kind: undefined, view: undefined, samples: 1 };
for (const st of stageRecords) records.push({ ...base, metric: `stage_${st.name}`, value: Math.round(st.ms * 100) / 100, unit: 'ms', seq: st.seq, kind: st.kind });
for (const kind of ['preview', 'refined']) {
  for (const [name] of [...stageNames, ['upload_total']]) {
    const v = stageRecords.filter((x) => x.kind === kind && x.name === name).map((x) => x.ms).sort((a, b) => a - b);
    if (v.length) records.push({ ...base, metric: `stage_${name}_${kind}_p50`, value: Math.round(percentile(v, 0.5) * 100) / 100, unit: 'ms', samples: v.length });
  }
}
// Memory before any snapshot arrived: window, device, swapchain and driver — the floor of mem_cpu.
const firstChunk = by('first_chunk')[0];
const idle = (samples[child.pid] ?? []).filter((x) => firstChunk && x.ts < spawnAt + firstChunk.t).map((x) => x.private);
if (idle.length) records.push({ ...base, metric: 'mem_cpu_before_data', value: Math.max(...idle), unit: 'B', samples: idle.length });
for (const d of raw.native.delivered) {
  records.push({ ...records[0], metric: 'delivery', value: d.bytes, unit: 'B', samples: 1, seq: d.seq, kind: d.delivery, view: undefined, fetch_ms: d.fetch_ms });
}

if (args.raw) fs.writeFileSync(args.raw, JSON.stringify(raw));
if (args.metrics) fs.writeFileSync(args.metrics, records.map((r) => JSON.stringify(r)).join('\n') + '\n');
for (const r of records) {
  if (r.seq != null) continue;
  console.log(`${r.metric.padEnd(30)} ${String(r.value).padStart(14)} ${r.unit}`);
}
