// The bytes between Bun and the Rust core (rust/loft-core/src/wire.rs): little-endian, read in the order written.

export class Writer {
  private buf = new ArrayBuffer(1 << 16);
  private view = new DataView(this.buf);
  private bytes = new Uint8Array(this.buf);
  at = 0;
  private room(n: number): void {
    if (this.at + n <= this.buf.byteLength) return;
    let size = this.buf.byteLength * 2;
    while (size < this.at + n) size *= 2;
    const next = new ArrayBuffer(size);
    new Uint8Array(next).set(this.bytes.subarray(0, this.at));
    this.buf = next; this.view = new DataView(next); this.bytes = new Uint8Array(next);
  }
  u32(v: number): void { this.room(4); this.view.setUint32(this.at, v, true); this.at += 4; }
  i32(v: number): void { this.room(4); this.view.setInt32(this.at, v, true); this.at += 4; }
  f64(v: number): void { this.room(8); this.view.setFloat64(this.at, v, true); this.at += 8; }
  f64s(v: ArrayLike<number>): void { this.u32(v.length); this.room(8 * v.length); for (let i = 0; i < v.length; i++) { this.view.setFloat64(this.at, v[i]!, true); this.at += 8; } }
  u32s(v: ArrayLike<number>): void { this.u32(v.length); this.room(4 * v.length); for (let i = 0; i < v.length; i++) { this.view.setUint32(this.at, v[i]!, true); this.at += 4; } }
  str(s: string): void {
    const max = s.length * 3;
    this.room(4 + max);
    const { written } = new TextEncoder().encodeInto(s, this.bytes.subarray(this.at + 4, this.at + 4 + max));
    this.view.setUint32(this.at, written, true);
    this.at += 4 + written;
  }
  finish(): Uint8Array { return this.bytes.subarray(0, this.at); }
}

export class Reader {
  private readonly view: DataView;
  at = 0;
  constructor(private readonly bytes: Uint8Array) { this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength); }
  u8(): number { return this.view.getUint8(this.at++); }
  u32(): number { const v = this.view.getUint32(this.at, true); this.at += 4; return v; }
  f64(): number { const v = this.view.getFloat64(this.at, true); this.at += 8; return v; }
  f32(): number { const v = this.view.getFloat32(this.at, true); this.at += 4; return v; }
  f64s(): Float64Array { const n = this.u32(); const out = new Float64Array(n); for (let i = 0; i < n; i++) out[i] = this.f64(); return out; }
  u32s(): Uint32Array { const n = this.u32(); const out = new Uint32Array(n); for (let i = 0; i < n; i++) out[i] = this.u32(); return out; }
  str(): string { const n = this.u32(); const s = new TextDecoder().decode(this.bytes.subarray(this.at, this.at + n)); this.at += n; return s; }
  get left(): number { return this.bytes.byteLength - this.at; }
}
