// P3.4: fixed-viewpoint captures of the server video as the browser shows it (decoded with
// WebCodecs, drawn on the canvas), for SSIM against the native capture (SPEC §7.1 video quality).
//
// usage: node bench/web/video-capture.mjs --server http://127.0.0.1:<replay> --ws ws://127.0.0.1:<serve>
//                                         --out <dir>
// Start the server first from the repository root:
//   pointblitz-server serve --replay http://127.0.0.1:<replay> --port <serve> --cold --wait-for-client
// It waits for the last snapshot, then asks for each viewpoint and screenshots the canvas.

import fs from 'node:fs';
import { createRequire } from 'node:module';
import { parseArgs, chromeArgs, CHROME } from '../../baseline/three/args.mjs';

const require = createRequire(new URL('../../baseline/three/package.json', import.meta.url));
const { chromium } = require('playwright-core');

const args = parseArgs();
const server = args.server ?? 'http://127.0.0.1:8700';
const ws = args.ws ?? 'ws://127.0.0.1:8720';
const out = args.out ?? 'target/bench/video-capture';
fs.mkdirSync(out, { recursive: true });

const browser = await chromium.launch({ executablePath: args.chrome ?? CHROME, headless: !args.headed, args: chromeArgs(args) });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
await page.goto(`${server}/static/web/video.html?ws=${encodeURIComponent(ws)}&scenario=cold`);
await page.waitForFunction(() => window.__pb?.marks.some((m) => m.name === 'presented'), null, { timeout: 120_000 });
const views = JSON.parse(fs.readFileSync('bench/viewpoints/flight-01.json', 'utf8')).viewpoints;
for (const v of views) {
  await page.evaluate((n) => window.__pb.view(n), v.name);
  await page.locator('#view').screenshot({ path: `${out}/${v.name}.png` });
  console.log(`${v.name} captured`);
}
await browser.close();
