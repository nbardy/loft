// Picture differences between two directories of lossless shots (tools/triangles.ts --shots), and side-by-side sheets.
//   bun tools/picdiff.ts out/triangles/before out/triangles/after [--sheet out/triangles/compare]
// Per picture: mean absolute difference per channel (0..255), the share of pixels whose largest channel differs by
// more than 16, and the 99.9th percentile of that largest difference.
import { mkdirSync, readdirSync, readFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { writeWebp, type Frame } from "../src/cli/image.ts";

/** Reads the PNGs `encodePng` writes (8-bit RGBA, filter 0 on every row). */
export function readPng(path: string): Frame {
  const b = readFileSync(path);
  const width = b.readUInt32BE(16), height = b.readUInt32BE(20);
  const idat: Buffer[] = [];
  for (let o = 8; o < b.length;) {
    const len = b.readUInt32BE(o), type = b.toString("ascii", o + 4, o + 8);
    if (type === "IDAT") idat.push(b.subarray(o + 8, o + 8 + len));
    o += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const rgba = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    if (raw[y * (width * 4 + 1)] !== 0) throw new Error(`${path}: row ${y} uses a PNG filter; only encodePng's output is read`);
    rgba.set(raw.subarray(y * (width * 4 + 1) + 1, (y + 1) * (width * 4 + 1)), y * width * 4);
  }
  return { width, height, rgba };
}

export function difference(a: Frame, b: Frame): { readonly mean: number; readonly over16: number; readonly p999: number } {
  let sum = 0, over = 0;
  const hist = new Uint32Array(256);
  const n = a.width * a.height;
  for (let i = 0; i < n; i++) {
    let m = 0;
    for (let c = 0; c < 3; c++) { const d = Math.abs(a.rgba[4 * i + c]! - b.rgba[4 * i + c]!); sum += d; m = Math.max(m, d); }
    hist[m]!++;
    if (m > 16) over++;
  }
  let acc = 0, p = 0;
  for (; p < 256; p++) { acc += hist[p]!; if (acc >= 0.999 * n) break; }
  return { mean: sum / (3 * n), over16: over / n, p999: p };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const si = args.indexOf("--sheet");
  const sheet = si < 0 ? undefined : args.splice(si, 2)[1];
  const [da, db] = args as [string, string];
  if (sheet) mkdirSync(sheet, { recursive: true });
  for (const f of readdirSync(db).filter((x) => x.endsWith(".png")).sort()) {
    const a = readPng(`${da}/${f}`), b = readPng(`${db}/${f}`);
    const d = difference(a, b);
    console.log(`${f.padEnd(28)} mean ${d.mean.toFixed(3)}  >16: ${(100 * d.over16).toFixed(3)}%  p99.9 ${d.p999}`);
    if (sheet) writeWebp(`${sheet}/${f.replace(".png", ".webp")}`, sideBySide(a, b));
  }
}

function sideBySide(a: Frame, b: Frame): Frame {
  const rgba = new Uint8Array(a.width * 2 * a.height * 4);
  for (let y = 0; y < a.height; y++) {
    rgba.set(a.rgba.subarray(y * a.width * 4, (y + 1) * a.width * 4), y * a.width * 8);
    rgba.set(b.rgba.subarray(y * a.width * 4, (y + 1) * a.width * 4), y * a.width * 8 + a.width * 4);
  }
  return { width: a.width * 2, height: a.height, rgba };
}
