// Records the GPU while a measurement runs (P3.4 validity rule, decision 0038).
//
// usage: node bench/gpu-watch.mjs --out <file.csv> -- <command> [args...]
// Samples `nvidia-smi` every 500 ms from 2 s before the command starts until it ends, and writes
// `phase,t_ms,util_gpu_pct,util_enc_pct,clock_gr_mhz,pstate` rows (phase = before | during), plus
// the GPU compute/graphics process list before and after as comment lines. Exits with the
// command's exit code.

import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const argv = process.argv.slice(2);
const sep = argv.indexOf('--');
const outIdx = argv.indexOf('--out');
if (sep < 0 || outIdx < 0) throw new Error('usage: gpu-watch.mjs --out <file.csv> -- <command> [args...]');
const out = argv[outIdx + 1];
const [cmd, ...cmdArgs] = argv.slice(sep + 1);

const apps = () =>
  (spawnSync('nvidia-smi', ['--query-compute-apps=pid,process_name', '--format=csv,noheader'], { encoding: 'utf8' }).stdout ?? '')
    .trim()
    .split(/\r?\n/)
    .filter(Boolean);

const rows = [];
const t0 = Date.now();
let phase = 'before';
// Measurement environment for the whole run (bench/measure-env.ps1, decisions 0040, 0041): display
// kept on (rule C3), Windows power throttling off for the run's processes, and the C4 CPU
// calibration + % Processor Performance written to <out>.cpu.log. Waits for the calibration so
// it does not overlap the run. The helper exits with this process.
if (process.platform === 'win32') {
  const cpuOut = out.replace(/\.gpu\.csv$/, '') + '.cpu.log';
  fs.writeFileSync(cpuOut, '');
  const env = spawn('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'bench/measure-env.ps1',
    '-ParentPid', String(process.pid), '-Out', path.resolve(cpuOut), '-Repo', process.cwd()], { stdio: ['ignore', 'pipe', 'ignore'] });
  await new Promise((resolve) => {
    const timer = setTimeout(resolve, 15000);
    env.stdout.on('data', (d) => {
      if (String(d).includes('ready')) {
        clearTimeout(timer);
        resolve();
      }
    });
  });
}
const smi = spawn('nvidia-smi', ['--query-gpu=utilization.gpu,utilization.encoder,clocks.gr,pstate', '--format=csv,noheader,nounits', '-lms', '500'], { stdio: ['ignore', 'pipe', 'ignore'] });
let buf = '';
smi.stdout.on('data', (d) => {
  buf += d;
  const lines = buf.split(/\r?\n/);
  buf = lines.pop();
  for (const l of lines) if (l.trim()) rows.push(`${phase},${Date.now() - t0},${l.split(',').map((x) => x.trim()).join(',')}`);
});
const before = apps();
await new Promise((r) => setTimeout(r, 2000));
phase = 'during';
const child = spawn(cmd, cmdArgs, { stdio: 'inherit', shell: false });
const code = await new Promise((resolve) => child.on('exit', resolve));
smi.kill();
const after = apps();
fs.writeFileSync(
  out,
  [
    ...before.map((a) => `# before: ${a}`),
    ...after.map((a) => `# after: ${a}`),
    'phase,t_ms,util_gpu_pct,util_enc_pct,clock_gr_mhz,pstate',
    ...rows,
  ].join('\n') + '\n',
);
process.exit(code ?? 1);
