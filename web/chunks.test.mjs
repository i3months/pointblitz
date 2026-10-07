// node --test web/  — framing of streamed chunk deliveries (no browser needed).
import test from 'node:test';
import assert from 'node:assert/strict';
import { ChunkSplitter } from './chunks.js';

function chunk(points, last, fill) {
  const c = new Uint8Array(80 + 16 * points).fill(fill);
  const v = new DataView(c.buffer);
  v.setUint32(16, points, true);
  v.setUint32(20, last ? 1 : 0, true);
  return c;
}

test('splits a stream cut at every possible byte boundary', () => {
  const a = chunk(3, false, 7);
  const b = chunk(0, false, 8);
  const c = chunk(2, true, 9);
  const stream = new Uint8Array([...a, ...b, ...c]);
  for (let step = 1; step <= stream.length; step += 13) {
    const got = [];
    const s = new ChunkSplitter((bytes, last) => got.push([bytes, last]));
    for (let off = 0; off < stream.length; off += step) s.push(stream.subarray(off, off + step));
    assert.equal(got.length, 3, `step ${step}`);
    assert.deepEqual([...got[0][0]], [...a]);
    assert.deepEqual([...got[1][0]], [...b]);
    assert.deepEqual([...got[2][0]], [...c]);
    assert.deepEqual(got.map((g) => g[1]), [false, false, true]);
  }
});

test('one byte at a time', () => {
  const a = chunk(1, true, 5);
  const got = [];
  const s = new ChunkSplitter((bytes) => got.push(bytes));
  for (const byte of a) s.push(new Uint8Array([byte]));
  assert.equal(got.length, 1);
  assert.deepEqual([...got[0]], [...a]);
});
