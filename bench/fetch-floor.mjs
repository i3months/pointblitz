// Browser receive floor (decision 0050 diagnosis, P4.11): how long Chrome takes to receive a large
// static file from the replay server, with no conversion on the server. Prints JSON.
//
// usage: node bench/fetch-floor.mjs --server http://127.0.0.1:8700 --path static/<file> [--times 5]
// (path without the leading slash: Git Bash rewrites arguments that start with one)

import { createRequire } from 'node:module';
import { parseArgs, chromeArgs, CHROME } from '../baseline/three/args.mjs';

const require = createRequire(new URL('../baseline/three/package.json', import.meta.url));
const { chromium } = require('playwright-core');

const args = parseArgs();
const server = args.server ?? 'http://127.0.0.1:8700';
const times = Number(args.times ?? 5);
const browser = await chromium.launch({ executablePath: args.chrome ?? CHROME, headless: !args.headed, args: chromeArgs(args) });
const page = await browser.newPage();
await page.goto(`${server}/manifest.json`);
const runs = await page.evaluate(
  async ({ path, times }) => {
    const out = [];
    for (let i = 0; i < times; i++) {
      const t0 = performance.now();
      const res = await fetch(path, { cache: 'no-store' });
      const t1 = performance.now();
      const reader = res.body.getReader();
      let bytes = 0;
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        bytes += value.length;
      }
      out.push({ headers_ms: t1 - t0, total_ms: performance.now() - t0, bytes });
    }
    return out;
  },
  { path: '/' + String(args.path).replace(/^\/+/, ''), times },
);
await browser.close();
const ms = runs.map((r) => r.total_ms).sort((a, b) => a - b);
console.log(JSON.stringify({ server, path: args.path, runs, median_ms: ms[Math.floor(ms.length / 2)], min_ms: ms[0], max_ms: ms[ms.length - 1] }));
