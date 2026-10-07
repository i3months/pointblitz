// Runs one scenario of a browser implementation in Chrome and writes SPEC §6.2 metrics (P0.5,
// decision 0020). The same harness measures the three.js baseline and PointBlitz web (P2).
//
// usage: node run.mjs --server http://127.0.0.1:8700 --scenario replay|cold|orbit|memtest|memspike
//                     [--speed 60] [--metrics <file.jsonl>] [--raw <file.json>] [--chrome <path>] [--headed]
//                     [--target three.js|web]   (web = PointBlitz web/index.html; default three.js)
//
// Raw hooks come from window.__pb (marks, frames, longtasks, syncFrames). Memory: JS heap over CDP
// (secondary) and renderer / GPU process private bytes from the OS (mem_cpu, procmem.mjs).

import { chromium } from 'playwright-core';
import { parseArgs, chromeArgs, CHROME } from './args.mjs';
import fs from 'node:fs';
import os from 'node:os';
import { execSync } from 'node:child_process';
import { summarize } from './metrics.mjs';
import { chromeProcessIds, peakCommit, sampleProcesses } from './procmem.mjs';

const args = parseArgs();
const server = args.server ?? 'http://127.0.0.1:8700';
const scenario = args.scenario ?? 'replay';
const speed = Number(args.speed ?? 60);
const chrome = args.chrome ?? CHROME;
const target = args.target ?? 'three.js';
const pagePath = target === 'web' ? 'web/index.html' : 'baseline/three/index.html';

const browser = await chromium.launch({
  executablePath: chrome,
  headless: !args.headed,
  args: chromeArgs(args),
});
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on('console', (m) => m.type() === 'error' && console.error('page:', m.text()));

const cdp = await page.context().newCDPSession(page);
await cdp.send('Performance.enable');
const heap = [];
const sampler = setInterval(async () => {
  try {
    const { metrics } = await cdp.send('Performance.getMetrics');
    const used = metrics.find((m) => m.name === 'JSHeapUsedSize');
    if (used) heap.push(used.value);
  } catch {}
}, 250);

const t0 = Date.now();
// --query a=b&c=d appends page parameters (A/B switches).
// Process memory is sampled from before the page loads (PR #6 review, PR #14 review): open a
// same-origin URL first so the renderer process already exists, start sampling, then navigate.
await page.goto(`${server}/manifest.json`);
const procs = await chromeProcessIds(browser);
const stopProc = sampleProcesses([...procs.renderer, ...procs.gpu]);
await new Promise((r) => setTimeout(r, 1000)); // a few samples before navigation
await page.goto(`${server}/static/${pagePath}?scenario=${scenario}&speed=${speed}${args.query ? `&${args.query}` : ''}`);
const after = await chromeProcessIds(browser);
const missed = after.renderer.filter((pid) => !procs.renderer.includes(pid));
if (missed.length) console.error(`warning: renderer process changed on navigation (${missed}); mem_cpu misses it`);
await page.waitForFunction(() => window.__pb?.done, null, { timeout: 3_600_000, polling: 500 });
clearInterval(sampler);
const procSamples = stopProc();

const raw = await page.evaluate(() => ({
  timeOrigin: performance.timeOrigin,
  marks: window.__pb.marks,
  frames: window.__pb.frames,
  longtasks: window.__pb.longtasks,
  syncFrames: window.__pb.syncFrames,
}));
const renderer = await page.evaluate(() => {
  if (window.__pb.gpu) return `${window.__pb.gpu.vendor} ${window.__pb.gpu.architecture} (WebGPU, ${window.__pb.info?.format})`;
  const gl = document.querySelector('canvas').getContext('webgl2');
  const ext = gl.getExtension('WEBGL_debug_renderer_info');
  return ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
});
raw.heap = heap;
raw.proc = {
  renderer: procs.renderer.flatMap((pid) => procSamples[pid] ?? []),
  gpu: procs.gpu.flatMap((pid) => procSamples[pid] ?? []),
};
raw.scenario = scenario;
raw.speed = speed;
raw.wall_ms = Date.now() - t0;
// mem_cpu = peak commit the OS recorded for the renderer (and, separately, the GPU process) over
// the whole run (decision 0032). Read before closing, while the processes still exist.
const rendererPids = [...new Set([...procs.renderer, ...after.renderer])];
const gpuPids = [...new Set([...procs.gpu, ...after.gpu])];
const peaks = peakCommit([...rendererPids, ...gpuPids]);
raw.peak = {
  renderer: Math.max(0, ...rendererPids.map((pid) => peaks[pid] ?? 0)),
  gpu: Math.max(0, ...gpuPids.map((pid) => peaks[pid] ?? 0)),
};
await browser.close();

const sh = (cmd) => {
  try {
    return execSync(cmd, { stdio: ['ignore', 'pipe', 'ignore'] }).toString().trim();
  } catch {
    return 'unknown';
  }
};
const commit = sh('git describe --always --dirty --abbrev=7');
const device = {
  impl: target === 'web' ? 'PointBlitz web (wgpu 30.0.1)' : 'three.js 0.185.1',
  browser: `Chrome ${browser.version()}`,
  renderer,
  gpu_driver: sh('nvidia-smi --query-gpu=driver_version --format=csv,noheader'),
  // Power plan GUID (Windows) and refresh rate: both affect timing (PR #7 review). Headless Chrome runs at 60 Hz.
  power_plan: sh('powercfg /getactivescheme').match(/[0-9a-f]{8}-[0-9a-f-]{27}/i)?.[0] ?? 'unknown',
  refresh_hz: 60,
  cpu: os.cpus()[0].model.trim(),
  os: `${os.type()} ${os.release()}`,
};

if (args.raw) fs.writeFileSync(args.raw, JSON.stringify(raw));
const records = summarize(raw, { device, commit, target });
if (args.metrics) fs.writeFileSync(args.metrics, records.map((r) => JSON.stringify(r)).join('\n') + '\n');
for (const r of records) {
  console.log(`${r.metric.padEnd(30)} ${String(r.value).padStart(14)} ${r.unit}`);
}
