// P2.1 check: open web/index.html from the replay server in Chrome with the measurement flags and
// print the WebGPU adapter the page got (decision 0028).
//
// usage: node bench/web/check.mjs --server http://127.0.0.1:8700 [--headed]
// Playwright comes from the baseline's install (baseline/three/package.json), the only Node setup.

import { createRequire } from 'node:module';
import { parseArgs, chromeArgs, CHROME } from '../../baseline/three/args.mjs';

const require = createRequire(new URL('../../baseline/three/package.json', import.meta.url));
const { chromium } = require('playwright-core');

const args = parseArgs();
const server = args.server ?? 'http://127.0.0.1:8700';
const browser = await chromium.launch({ executablePath: args.chrome ?? CHROME, headless: !args.headed, args: chromeArgs(args) });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on('console', (m) => console.error(`page ${m.type()}: ${m.text()}`));
await page.goto(`${server}/static/web/index.html?scenario=check`);
await page.waitForFunction(() => window.__pb?.done, null, { timeout: 60_000 });
const pb = await page.evaluate(() => window.__pb);
await browser.close();
console.log(JSON.stringify({ chrome: browser.version(), info: pb.info, gpu: pb.gpu, error: pb.error, marks: pb.marks }, null, 2));
process.exit(pb.info && !pb.error ? 0 : 1);
