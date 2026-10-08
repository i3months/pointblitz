// Captures the three.js baseline at the 8 fixed viewpoints (P0.4, recheck of P0.2).
//
// usage: node capture.mjs --server http://127.0.0.1:8700 --out <dir> [--chrome <path>] [--headed]
//                        [--query mode=b1&upload=a]   (baseline variant, decision 0048)
// The replay server must serve the repository root as its web root:
//   pointblitz-bench replay --data <ply dir> --web <repo root>
//
// Writes <out>/<view>.png and <out>/capture.json (renderer string, per-view coverage).

import { chromium } from 'playwright-core';
import { parseArgs, chromeArgs, CHROME } from './args.mjs';
import fs from 'node:fs';
import path from 'node:path';

const args = parseArgs();
const server = args.server ?? 'http://127.0.0.1:8700';
const out = args.out ?? 'out/baseline-three';
const chrome = args.chrome ?? CHROME;
fs.mkdirSync(out, { recursive: true });

// Real GPU in headless mode: ANGLE on D3D11 (Windows). The renderer string is recorded so a
// software fallback (SwiftShader) is visible in the result instead of silently skewing numbers.
const browser = await chromium.launch({
  executablePath: chrome,
  headless: !args.headed,
  args: chromeArgs(args),
});
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on('console', (m) => m.type() === 'error' && console.error('page:', m.text()));
await page.goto(`${server}/static/baseline/three/index.html?scenario=cold${args.query ? `&${args.query}` : ""}`);
await page.waitForFunction(() => window.__pb?.done, null, { timeout: 300_000 });

const gpu = await page.evaluate(() => {
  if (window.__pb.gpu) return `${window.__pb.gpu.vendor} ${window.__pb.gpu.architecture} (three.js WebGPURenderer)`;
  const gl = document.querySelector('canvas').getContext('webgl2');
  const ext = gl.getExtension('WEBGL_debug_renderer_info');
  return ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
});
console.log('renderer:', gpu);

const views = await page.evaluate(async () =>
  (await (await fetch('/static/bench/viewpoints/flight-01.json')).json()).viewpoints.map((v) => v.name),
);
const result = { renderer: gpu, url: page.url(), views: [] };
for (const name of views) {
  await page.evaluate((n) => window.__pb.setView(n), name);
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  const file = path.join(out, `${name}.png`);
  await page.locator('canvas').screenshot({ path: file });
  // Coverage measured on the rendered image: non-black pixels and the 64×36 coarse grid used by
  // the P0.2 projection check (decision 0017).
  const cov = await page.evaluate(() => {
    const src = document.querySelector('canvas');
    const c = document.createElement('canvas');
    c.width = src.width;
    c.height = src.height;
    const g = c.getContext('2d');
    g.drawImage(src, 0, 0);
    const d = g.getImageData(0, 0, c.width, c.height).data;
    const GX = 64, GY = 36;
    const cells = new Uint8Array(GX * GY);
    let lit = 0;
    for (let y = 0; y < c.height; y++) {
      for (let x = 0; x < c.width; x++) {
        const i = (y * c.width + x) * 4;
        if (d[i] | d[i + 1] | d[i + 2]) {
          lit++;
          cells[Math.min(GY - 1, Math.floor((y / c.height) * GY)) * GX + Math.min(GX - 1, Math.floor((x / c.width) * GX))] = 1;
        }
      }
    }
    return { lit_pixels: lit / (c.width * c.height), grid_coverage: cells.reduce((a, b) => a + b, 0) / cells.length };
  });
  result.views.push({ name, file: path.basename(file), ...cov });
  console.log(`${name.padEnd(12)} lit ${(cov.lit_pixels * 100).toFixed(2)} %  grid ${(cov.grid_coverage * 100).toFixed(2)} %`);
}
fs.writeFileSync(path.join(out, 'capture.json'), JSON.stringify(result, null, 2));
await browser.close();
