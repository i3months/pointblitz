// Chunk stream framing for the browser client (decisions 0022, 0030, 0051).

/**
 * Cuts a byte stream into chunks (decisions 0022, 0051: point_count at byte 16, point stride at
 * byte 72, 80 B + stride × points) and calls onChunk(bytes, last, firstPass) for each complete one —
 * last = last chunk of its generation, firstPass = completes the coarse first pass. Each chunk is
 * copied once into its own buffer.
 */
export class ChunkSplitter {
  constructor(onChunk) {
    this.onChunk = onChunk;
    this.head = new Uint8Array(80);
    this.headLen = 0;
    this.body = null;
    this.bodyLen = 0;
  }

  push(bytes) {
    let off = 0;
    while (off < bytes.length) {
      if (!this.body) {
        const n = Math.min(80 - this.headLen, bytes.length - off);
        this.head.set(bytes.subarray(off, off + n), this.headLen);
        this.headLen += n;
        off += n;
        if (this.headLen < 80) return;
        const view = new DataView(this.head.buffer);
        const points = view.getUint32(16, true);
        const flags = view.getUint32(20, true);
        this.last = (flags & 1) !== 0;
        this.firstPass = (flags & 2) !== 0;
        const stride = view.getUint16(72, true);
        if (stride !== 12 && stride !== 8) throw new Error(`chunk point stride ${stride}`);
        this.body = new Uint8Array(80 + stride * points);
        this.body.set(this.head);
        this.bodyLen = 80;
      }
      const n = Math.min(this.body.length - this.bodyLen, bytes.length - off);
      this.body.set(bytes.subarray(off, off + n), this.bodyLen);
      this.bodyLen += n;
      off += n;
      if (this.bodyLen === this.body.length) {
        const chunk = this.body;
        this.body = null;
        this.headLen = 0;
        this.onChunk(chunk, this.last, this.firstPass);
      }
    }
  }
}
