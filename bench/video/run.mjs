// Runs one PointBlitz server-video scenario and writes SPEC §6.2 metrics (P3.4, decisions 0036–0038).
//
// usage: node bench/video/run.mjs --server http://127.0.0.1:<replay> --port <serve port>
//                                 --scenario replay|cold|orbit [--speed 60] [--metrics <file.jsonl>]
//                                 [--exe target/release/pointblitz-server.exe] [--log <server log>]
// Run from the repository root (serve reads bench/viewpoints/flight-01.json).
//
// Starts `pointblitz-server serve` (waits for the client, then follows the replay server), runs the
// browser client through the shared harness (baseline/three/run.mjs --target video, inputs on),
// reads the server process's OS peak commit before letting it exit, and appends the server's own
// figures to the browser client's records.

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { parseArgs } from '../../baseline/three/args.mjs';
import { peakCommit } from '../../baseline/three/procmem.mjs';

const args = parseArgs();
const server = args.server ?? 'http://127.0.0.1:8700';
const port = Number(args.port ?? 8720);
const scenario = args.scenario ?? 'replay';
const speed = Number(args.speed ?? 60);
const exe = args.exe ?? 'target/release/pointblitz-server.exe';
const metrics = args.metrics ?? path.join(os.tmpdir(), `pb-video-${process.pid}.jsonl`);
const log = args.log ?? path.join(os.tmpdir(), `pb-video-${process.pid}-server.jsonl`);

const serveArgs = ['serve', '--replay', server, '--port', String(port), '--speed', String(speed),
  '--wait-for-client', '--wait-before-exit', '--log', log];
if (scenario === 'cold') serveArgs.push('--cold', '--exit-after-end', '2');
else if (scenario === 'orbit') serveArgs.push('--orbit');
else serveArgs.push('--exit-after-end', '2');

const serve = spawn(exe, serveArgs, { stdio: ['pipe', 'ignore', 'pipe'] });
let serveErr = '';
let peak = 0;
const finished = new Promise((resolve) => {
  serve.stderr.on('data', (d) => {
    serveErr += d;
    if (!peak && /^finished\r?$/m.test(serveErr)) {
      peak = peakCommit([serve.pid])[serve.pid] ?? 0;
      serve.stdin.end('\n');
      resolve();
    }
  });
});
const serveExit = new Promise((resolve) => serve.on('exit', resolve));
await new Promise((r) => setTimeout(r, 1500)); // let it bind and open the encoder

const client = spawn(process.execPath, ['run.mjs', '--server', server, '--target', 'video',
  '--query', `ws=ws://127.0.0.1:${port}&inputs=1`, '--scenario', scenario, '--speed', String(speed),
  '--metrics', path.resolve(metrics), ...(args.raw ? ['--raw', path.resolve(args.raw)] : [])], { cwd: 'baseline/three', stdio: ['ignore', 'ignore', 'inherit'] });
const clientCode = await new Promise((resolve) => client.on('exit', resolve));
await Promise.race([finished, serveExit]);
const serveCode = await serveExit;
if (clientCode !== 0 || serveCode !== 0) {
  process.stderr.write(serveErr);
  throw new Error(`client exited ${clientCode}, server exited ${serveCode}`);
}

// Server figures, appended to the client's records with the same device/commit fields.
const records = fs.readFileSync(metrics, 'utf8').trim().split('\n').map((l) => JSON.parse(l));
const base = { ...records[0], seq: undefined, kind: undefined, view: undefined, samples: 1, method: undefined };
const summary = fs.readFileSync(log, 'utf8').trim().split('\n').map((l) => JSON.parse(l)).find((x) => x.type === 'summary');
const add = (metric, value, unit, extra = {}) => records.push({ ...base, metric, value, unit, ...extra });
add('mem_server_peak', peak, 'B', { method: 'OS peak commit of the server process (PeakPagefileUsage)' });
if (summary) {
  add('server_frames', summary.frames, 'count');
  add('server_late_ticks_pct', (100 * summary.over_budget_ticks) / summary.frames, '%');
  for (const k of ['render', 'encode']) for (const q of ['p50', 'p95', 'p99']) add(`server_${k}_ms_${q}`, summary[`${k}_ms_${q}`], 'ms');
}
fs.writeFileSync(metrics, records.map((r) => JSON.stringify(r)).join('\n') + '\n');
for (const r of records) if (r.seq == null) console.log(`${r.metric.padEnd(30)} ${String(r.value).padStart(14)} ${r.unit}`);
