// Chunk stream framing for the browser client (decision 0022, 0030).

/**
 * Cuts a byte stream into chunks (decision 0022: point_count at byte 16, 80 B + 16 B/point) and
 * calls onChunk(bytes, last) for each complete one. Each chunk is copied once into its own buffer.
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
        this.last = (view.getUint32(20, true) & 1) !== 0;
        this.body = new Uint8Array(80 + 16 * points);
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
        this.onChunk(chunk, this.last);
      }
    }
  }
}
