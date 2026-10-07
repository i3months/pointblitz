// Runs one baseline scenario in Chrome and dumps the raw measurement hooks (window.__pb).
//
// usage: node run.mjs --server http://127.0.0.1:8700 --scenario replay|cold|orbit [--speed 60]
//                     [--out <file.json>] [--chrome <path>] [--headed]
// The P0.5 harness turns this raw dump into SPEC §6.2 metrics.

import { chromium } from 'playwright-core';
import fs from 'node:fs';

const args = {};
for (let i = 2; i < process.argv.length; i++) {
  const a = process.argv[i];
  if (a.startsWith('--')) args[a.slice(2)] = process.argv[i + 1]?.startsWith('--') ? true : process.argv[++i] ?? true;
}
const server = args.server ?? 'http://127.0.0.1:8700';
const scenario = args.scenario ?? 'replay';
const speed = args.speed ?? 60;
const chrome = args.chrome ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe';

const browser = await chromium.launch({
  executablePath: chrome,
  headless: !args.headed,
  args: ['--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist', '--window-size=1920,1080'],
});
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on('console', (m) => m.type() === 'error' && console.error('page:', m.text()));
const t0 = Date.now();
await page.goto(`${server}/static/baseline/three/index.html?scenario=${scenario}&speed=${speed}`);
await page.waitForFunction(() => window.__pb?.done, null, { timeout: 3_600_000, polling: 500 });
const dump = await page.evaluate(() => ({ marks: window.__pb.marks, frames: window.__pb.frames }));
dump.scenario = scenario;
dump.speed = Number(speed);
dump.wall_ms = Date.now() - t0;
if (args.out) fs.writeFileSync(args.out, JSON.stringify(dump));
const presented = dump.marks.filter((m) => m.name === 'presented').length;
console.log(`${scenario}: ${presented} snapshots presented, ${dump.frames.length} frames, ${dump.wall_ms} ms`);
await browser.close();
