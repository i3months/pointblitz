// Binary little-endian PLY → positions / colours for the B2 baseline (decision 0048). Used by
// ply-worker.js; pure, so node --test can check it.

const SIZE = { char: 1, int8: 1, uchar: 1, uint8: 1, short: 2, int16: 2, ushort: 2, uint16: 2, int: 4, int32: 4, uint: 4, uint32: 4, float: 4, float32: 4, double: 8, float64: 8 };

export function header(bytes) {
  const end = new TextDecoder().decode(bytes.subarray(0, Math.min(bytes.length, 65536))).indexOf('end_header\n');
  if (end < 0) throw new Error('no end_header');
  const text = new TextDecoder().decode(bytes.subarray(0, end));
  let inVertex = false;
  let stride = 0;
  const at = {};
  for (const line of text.split('\n')) {
    const w = line.trim().split(/\s+/);
    if (w[0] === 'format' && w[1] !== 'binary_little_endian') throw new Error(`unsupported ${w[1]}`);
    if (w[0] === 'element') inVertex = w[1] === 'vertex';
    if (w[0] === 'property' && inVertex) {
      at[w[2]] = { offset: stride, type: w[1] };
      stride += SIZE[w[1]];
    }
  }
  for (const k of ['x', 'y', 'z']) if (at[k]?.type !== 'float' && at[k]?.type !== 'float32') throw new Error(`${k} is not float32`);
  for (const k of ['red', 'green', 'blue']) if (at[k]?.type !== 'uchar' && at[k]?.type !== 'uint8') throw new Error(`${k} is not uint8`);
  return { length: end + 'end_header\n'.length, stride, at };
}


/** Parses a PLY header + records (a delta may hold only the tail records; the header still states
 *  the total). Returns positions float32×3, colours uint8×4 (a = 255) and the bounding box. */
export function parsePly(buffer) {
  const bytes = new Uint8Array(buffer);
  const h = header(bytes);
  const count = Math.floor((bytes.length - h.length) / h.stride);
  const v = new DataView(buffer, h.length);
  const position = new Float32Array(count * 3);
  const color = new Uint8Array(count * 4);
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  const [ox, oy, oz] = ['x', 'y', 'z'].map((k) => h.at[k].offset);
  const [or, og, ob] = ['red', 'green', 'blue'].map((k) => h.at[k].offset);
  for (let i = 0, o = 0; i < count; i++, o += h.stride) {
    const x = v.getFloat32(o + ox, true);
    const y = v.getFloat32(o + oy, true);
    const z = v.getFloat32(o + oz, true);
    position[i * 3] = x;
    position[i * 3 + 1] = y;
    position[i * 3 + 2] = z;
    if (x < min[0]) min[0] = x;
    if (y < min[1]) min[1] = y;
    if (z < min[2]) min[2] = z;
    if (x > max[0]) max[0] = x;
    if (y > max[1]) max[1] = y;
    if (z > max[2]) max[2] = z;
    color[i * 4] = v.getUint8(o + or);
    color[i * 4 + 1] = v.getUint8(o + og);
    color[i * 4 + 2] = v.getUint8(o + ob);
    color[i * 4 + 3] = 255;
  }
  return { position, color, count, min, max };
}
