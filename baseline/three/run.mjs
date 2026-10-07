// Runs one baseline scenario in Chrome and writes SPEC §6.2 metrics (P0.5, decision 0020).
//
// usage: node run.mjs --server http://127.0.0.1:8700 --scenario replay|cold|orbit [--speed 60]
//                     [--metrics <file.jsonl>] [--raw <file.json>] [--chrome <path>] [--headed]
//
// Raw hooks come from window.__pb (marks, frames, longtasks); JS heap is sampled over CDP.

import { chromium } from 'playwright-core';
import fs from 'node:fs';
import os from 'node:os';
import { execSync } from 'node:child_process';
import { summarize } from './metrics.mjs';

const args = {};
for (let i = 2; i < process.argv.length; i++) {
  const a = process.argv[i];
  if (a.startsWith('--')) args[a.slice(2)] = process.argv[i + 1]?.startsWith('--') ? true : process.argv[++i] ?? true;
}
const server = args.server ?? 'http://127.0.0.1:8700';
const scenario = args.scenario ?? 'replay';
const speed = Number(args.speed ?? 60);
const chrome = args.chrome ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe';

const browser = await chromium.launch({
  executablePath: chrome,
  headless: !args.headed,
  args: [
    '--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist', '--window-size=1920,1080',
    // Uncapped frames: without these, rAF runs at the display rate and frame_time shows vsync, not render cost.
    ...(args.vsync ? [] : ['--disable-gpu-vsync', '--disable-frame-rate-limit']),
  ],
});
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on('console', (m) => m.type() === 'error' && !m.text().includes('404') && console.error('page:', m.text()));

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
await page.goto(`${server}/static/baseline/three/index.html?scenario=${scenario}&speed=${speed}`);
await page.waitForFunction(() => window.__pb?.done, null, { timeout: 3_600_000, polling: 500 });
clearInterval(sampler);

const raw = await page.evaluate(() => ({
  marks: window.__pb.marks,
  frames: window.__pb.frames,
  longtasks: window.__pb.longtasks,
  syncFrames: window.__pb.syncFrames,
}));
const renderer = await page.evaluate(() => {
  const gl = document.querySelector('canvas').getContext('webgl2');
  const ext = gl.getExtension('WEBGL_debug_renderer_info');
  return ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
});
raw.heap = heap;
raw.scenario = scenario;
raw.speed = speed;
raw.wall_ms = Date.now() - t0;
await browser.close();

let commit = 'unknown';
try {
  commit = execSync('git rev-parse --short HEAD').toString().trim();
} catch {}
const device = {
  impl: 'three.js 0.185.1',
  browser: `Chrome ${browser.version()}`,
  renderer,
  cpu: os.cpus()[0].model.trim(),
  os: `${os.type()} ${os.release()}`,
};

if (args.raw) fs.writeFileSync(args.raw, JSON.stringify(raw));
const records = summarize(raw, { device, commit });
if (args.metrics) fs.writeFileSync(args.metrics, records.map((r) => JSON.stringify(r)).join('\n') + '\n');
for (const r of records) {
  console.log(`${r.metric.padEnd(28)} ${String(r.value).padStart(14)} ${r.unit}`);
}
