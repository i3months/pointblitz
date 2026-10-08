// B2 baseline (decision 0048): parses a binary little-endian PLY off the main thread.
// In: { id, buffer } (transferred). Out (transferred): positions, colours, bounding box, and the time
// spent here (ms).
import { parsePly } from './ply-parse.js';

self.onmessage = ({ data: { id, buffer } }) => {
  try {
    const t0 = performance.now();
    const r = parsePly(buffer);
    self.postMessage({ id, ...r, ms: performance.now() - t0 }, [r.position.buffer, r.color.buffer]);
  } catch (e) {
    self.postMessage({ id, error: String(e) });
  }
};
