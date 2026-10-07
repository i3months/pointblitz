// P2.2: fixed-viewpoint captures of PointBlitz in the browser (decisions 0017, 0029).
//
// usage: node bench/web/capture.mjs --server http://127.0.0.1:8700 --out <dir> [--headed]
//        [--query backend=webgl] [--chrome-args disable-features=WebGPU]
// The page loads the last snapshot (scenario=still); each view is drawn, the GPU is waited for,
// then the canvas is screenshotted. Same Chrome and flags as the baseline (baseline/three/args.mjs).

import fs from 'node:fs';
import { createRequire } from 'node:module';
import { parseArgs, chromeArgs, CHROME } from '../../baseline/three/args.mjs';

const require = createRequire(new URL('../../baseline/three/package.json', import.meta.url));
const { chromium } = require('playwright-core');

const args = parseArgs();
const server = args.server ?? 'http://127.0.0.1:8700';
const out = args.out ?? 'target/bench/pointblitz-web';
fs.mkdirSync(out, { recursive: true });

const browser = await chromium.launch({ executablePath: args.chrome ?? CHROME, headless: !args.headed, args: chromeArgs(args) });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on('console', (m) => m.type() === 'error' && console.error('page:', m.text()));
await page.goto(`${server}/static/web/index.html?scenario=still${args.query ? `&${args.query}` : ''}`);
await page.waitForFunction(() => window.__pb?.done, null, { timeout: 120_000 });
const pb = await page.evaluate(() => ({ info: window.__pb.info, gpu: window.__pb.gpu, error: window.__pb.error, marks: window.__pb.marks }));
if (pb.error) throw new Error(pb.error);
console.error(`viewer: ${JSON.stringify(pb.info)} gpu: ${JSON.stringify(pb.gpu)}`);
const up = pb.marks.find((m) => m.name === 'uploaded');
console.error(`${up.points} points, ${up.gpu_bytes} GPU bytes`);

const views = JSON.parse(fs.readFileSync('bench/viewpoints/flight-01.json', 'utf8')).viewpoints;
const canvas = page.locator('#view');
for (const v of views) {
  await page.evaluate((name) => window.__pb.setView(name), v.name);
  await canvas.screenshot({ path: `${out}/${v.name}.png` });
  console.log(`${v.name} captured`);
}
await browser.close();
