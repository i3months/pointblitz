// node --test baseline/three/  — the B2 baseline's PLY parser (decision 0048), no browser needed.
import test from 'node:test';
import assert from 'node:assert/strict';
import { parsePly } from './ply-parse.js';

// skyrecon's layout: x y z float32, red green blue uint8, nx ny nz float32 (27 B per point).
const HEADER =
  'ply\nformat binary_little_endian 1.0\nelement vertex 3\nproperty float32 x\nproperty float32 y\nproperty float32 z\n' +
  'property uint8 red\nproperty uint8 green\nproperty uint8 blue\nproperty float32 nx\nproperty float32 ny\nproperty float32 nz\nend_header\n';
const POINTS = [
  [1.5, -2, 3, 10, 20, 30],
  [-4, 5.25, -6, 40, 50, 60],
  [7, 8, 9.75, 70, 80, 255],
];

function ply(points) {
  const head = new TextEncoder().encode(HEADER);
  const out = new Uint8Array(head.length + 27 * points.length);
  out.set(head);
  const v = new DataView(out.buffer, head.length);
  points.forEach(([x, y, z, r, g, b], i) => {
    const o = 27 * i;
    v.setFloat32(o, x, true);
    v.setFloat32(o + 4, y, true);
    v.setFloat32(o + 8, z, true);
    v.setUint8(o + 12, r);
    v.setUint8(o + 13, g);
    v.setUint8(o + 14, b);
    v.setFloat32(o + 15, 0.5, true); // normals are ignored
  });
  return out.buffer;
}

test('parses positions, colours and the bounding box', () => {
  const r = parsePly(ply(POINTS));
  assert.equal(r.count, 3);
  assert.deepEqual([...r.position], POINTS.flatMap(([x, y, z]) => [x, y, z]));
  assert.deepEqual([...r.color], POINTS.flatMap(([, , , r, g, b]) => [r, g, b, 255]));
  assert.deepEqual(r.min, [-4, -2, -6]);
  assert.deepEqual(r.max, [7, 8, 9.75]);
});

test('a delta (header + tail records only) yields just the appended points', () => {
  // What /data/<file>?have= sends: the header still says 3 vertices, the body holds the last one.
  const whole = new Uint8Array(ply(POINTS));
  const headLen = new TextEncoder().encode(HEADER).length;
  const delta = new Uint8Array(headLen + 27);
  delta.set(whole.subarray(0, headLen));
  delta.set(whole.subarray(headLen + 2 * 27), headLen);
  const r = parsePly(delta.buffer);
  assert.equal(r.count, 1);
  assert.deepEqual([...r.position], [7, 8, 9.75]);
  assert.deepEqual([...r.color], [70, 80, 255, 255]);
});

test('rejects what it cannot read', () => {
  assert.throws(() => parsePly(new TextEncoder().encode('ply\nformat ascii 1.0\nend_header\n').buffer), /unsupported ascii/);
  assert.throws(() => parsePly(new TextEncoder().encode('ply\nformat binary_little_endian 1.0\n').buffer), /no end_header/);
});
