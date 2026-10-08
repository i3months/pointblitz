// P3.1: decode encode-test streams in Chrome (WebCodecs, the client's decoder) and save the
// sampled frames as PNG for SSIM against the native capture and the pre-encode frame.
//
// usage: node bench/web/decode.mjs --server http://127.0.0.1:8700 --dir target/bench/p31/qp23
//        (the replay server must serve the repository root with --web .)

import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { parseArgs, chromeArgs, CHROME } from '../../baseline/three/args.mjs';

const require = createRequire(new URL('../../baseline/three/package.json', import.meta.url));
const { chromium } = require('playwright-core');

const args = parseArgs();
const server = args.server ?? 'http://127.0.0.1:8700';
const dir = args.dir;
if (!dir) throw new Error('--dir <encode-test QP dir> is required');
const rel = path.relative(process.cwd(), dir).split(path.sep).join('/');

const browser = await chromium.launch({ executablePath: args.chrome ?? CHROME, headless: !args.headed, args: chromeArgs(args) });
for (const stream of ['views', 'orbit']) {
  const out = path.join(dir, `${stream}-dec`);
  fs.mkdirSync(out, { recursive: true });
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  await page.goto(`${server}/static/bench/web/decode.html?h264=/static/${rel}/${stream}.h264&index=/static/${rel}/${stream}.json`);
  await page.waitForFunction(() => window.__pb?.ready, null, { timeout: 120_000 });
  const pb = await page.evaluate(() => ({ error: window.__pb.error, samples: window.__pb.samples, codec: window.__pb.codec, decoded: window.__pb.decoded }));
  if (pb.error) throw new Error(`${stream}: ${pb.error}`);
  console.log(`${stream}: ${pb.codec}, ${pb.decoded} frames decoded, ${pb.samples.length} samples`);
  for (const name of pb.samples) {
    await page.evaluate((n) => window.__pb.draw(n), name);
    await page.locator('#view').screenshot({ path: path.join(out, `${name}.png`) });
  }
  await page.close();
}
await browser.close();
