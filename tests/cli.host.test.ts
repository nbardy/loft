// What a training host asks of the CLI (REPORT.md of branch training-host-contract): lossless frames, a `whole` view
// that always draws, and limits refused before any GPU work. Each runs the real command in a fresh process.
import { expect, test } from "bun:test";
import { mkdtempSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { inflateSync } from "node:zlib";
import { parseArgs } from "../src/cli/args.ts";

const FIX = "tests/fixtures/host";
const loft = (...args: string[]) => {
  const r = Bun.spawnSync(["bun", "src/cli/main.ts", ...args], { cwd: join(import.meta.dir, "..") });
  return { code: r.exitCode, out: r.stdout.toString(), err: r.stderr.toString() };
};
const lastLine = (s: string) => s.trimEnd().split("\n").at(-1)!;

/** RGBA of a PNG this CLI wrote (8-bit RGBA, filter 0 on every row: src/cli/image.ts encodePng). */
function pngRgba(path: string): { width: number; height: number; rgba: Uint8Array } {
  const b = new Uint8Array(readFileSync(path));
  const dv = new DataView(b.buffer, b.byteOffset);
  let i = 8, width = 0, height = 0;
  const idat: Uint8Array[] = [];
  while (i < b.length) {
    const n = dv.getUint32(i), type = String.fromCharCode(...b.subarray(i + 4, i + 8));
    if (type === "IHDR") { width = dv.getUint32(i + 8); height = dv.getUint32(i + 12); }
    if (type === "IDAT") idat.push(b.subarray(i + 8, i + 8 + n));
    i += 12 + n;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const rgba = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) rgba.set(raw.subarray(y * (width * 4 + 1) + 1, (y + 1) * (width * 4 + 1)), y * width * 4);
  return { width, height, rgba };
}

test("--out dir/ parses as one PNG per picture; anything but .webp or a directory is refused naming both", () => {
  expect(parseArgs(["frames", "a.ts", "--out", "o/", "--times", "0,1"])).toMatchObject({ out: { kind: "pngs", dir: "o/" } });
  expect(parseArgs(["render", "a.ts", "--out", "o.webp"])).toMatchObject({ out: { kind: "sheet", path: "o.webp" } });
  expect(() => parseArgs(["frames", "a.ts", "--out", "o.png", "--times", "0"])).toThrow("trailing slash");
});

test("--budget parses either limit alone; a bad one is refused with an example", () => {
  expect(parseArgs(["render", "a.ts", "--out", "o.webp", "--budget", "triangles=120000,draws=96"])).toMatchObject({ budget: { triangles: 120000, draws: 96 } });
  expect(parseArgs(["clip", "a.ts", "--out", "o.mp4", "--budget", "draws=8"])).toMatchObject({ budget: { triangles: Infinity, draws: 8 } });
  expect(parseArgs(["render", "a.ts", "--out", "o.webp"])).toMatchObject({ budget: { triangles: Infinity, draws: Infinity } });
  expect(() => parseArgs(["render", "a.ts", "--out", "o.webp", "--budget", "tris=5"])).toThrow("e.g. --budget triangles=120000,draws=96");
});

test("frames --out dir/ writes one lossless PNG per time, at the size asked, with exactly the pixels drawn", () => {
  const dir = mkdtempSync(join(tmpdir(), "loft-pngs-"));
  const r = loft("frames", `${FIX}/own_whole.ts`, "--times", "0,0.5,1", "--view", "whole", "--size", "96x64", "--out", `${dir}/`);
  expect(r.code).toBe(0);
  expect(readdirSync(dir).sort()).toEqual(["0.png", "1.png", "2.png"]);
  // The CLI prints the sha256 of each frame's RGBA as drawn; the PNG must decode to the same bytes.
  const drawn = [...r.out.matchAll(/rgba ([0-9a-f]{16})/g)].map((m) => m[1]);
  expect(drawn.length).toBe(3);
  for (const k of [0, 1, 2]) {
    const p = pngRgba(join(dir, `${k}.png`));
    expect([p.width, p.height]).toEqual([96, 64]);
    expect(createHash("sha256").update(p.rgba).digest("hex").slice(0, 16)).toBe(drawn[k]!);
  }
});

test("--view whole on a scene with no camera of that name derives one, says so on stderr, and frames both things", () => {
  const dir = mkdtempSync(join(tmpdir(), "loft-whole-"));
  const r = loft("frames", `${FIX}/no_whole.ts`, "--times", "0,1", "--view", "whole", "--size", "64x64", "--out", `${dir}/`);
  expect(r.code).toBe(0);
  expect(r.err).toContain(`camera "whole": the scene declares none, so it was derived: framing left, right within`);
  // The scene's other camera ("tip") is never used in its place: the derived one looks from in front and above.
  expect(r.err).toMatch(/eye \[[\d.-]+, [\d.]+, [\d.]+\], target/);
  // Both reeds stand within the bounds, and the times are fixed, not the ones asked for.
  expect(r.err).toMatch(/within \[-1\.\d+, -?0(\.\d+)?, [\d.-]+\]\.\.\[1\.\d+, 4(\.\d+)?, [\d.-]+\] over t=0,0.5,1,1.5,2,2.5,3,3.5,4/);
  // Something other than the ground is in the middle of the picture: the camera looks at the reeds.
  const p = pngRgba(join(dir, "0.png"));
  const ground = [...p.rgba.subarray(0, 3)];
  let differs = 0;
  for (let y = 16; y < 48; y++) for (let x = 16; x < 48; x++) if (Math.abs(p.rgba[(y * 64 + x) * 4]! - ground[0]!) > 20) differs++;
  expect(differs).toBeGreaterThan(20);
});

test("a scene's own whole camera is used as declared, and nothing is derived", () => {
  const dir = mkdtempSync(join(tmpdir(), "loft-own-"));
  const r = loft("render", `${FIX}/own_whole.ts`, "--views", "whole", "--size", "64x64", "--out", `${dir}/`);
  expect(r.code).toBe(0);
  expect(r.err).not.toContain("derived");
});

test("--view whole with nothing to measure and no camera of that name is a scene refusal naming the fix", () => {
  const r = loft("frames", `${FIX}/nothing_to_frame.ts`, "--times", "0", "--view", "whole", "--size", "32x32", "--out", `${mkdtempSync(join(tmpdir(), "loft-n-"))}/`);
  expect(r.code).toBe(4);
  expect(lastLine(r.err)).toContain(`loft: scene: camera "whole": given a scene with no camera "whole" and no thing whose shape Loft can measure on the CPU (spark)`);
  expect(lastLine(r.err)).toContain(`Fix: add cameras: { whole:`);
});

test("--budget over the limit refuses after compiling and before any GPU work, with the counts and where to cut", () => {
  const r = loft("frames", `${FIX}/no_whole.ts`, "--times", "0", "--view", "tip", "--size", "32x32", "--out", "out/never.webp", "--budget", "triangles=500,draws=4");
  expect(r.code).toBe(4);
  expect(r.out).toContain("compile");
  expect(r.out).not.toContain("load "); // the GPU load line never printed: refused before it
  const line = lastLine(r.err);
  expect(line).toMatch(/^loft: scene: --budget: given \d+ triangles \(limit 500\) and \d+ draws \(limit 4\); expected at most 500 triangles and 4 draws\. Fix: /);
  expect(line).toContain("heaviest things (triangles: left ");
  expect(line).toContain("draw fewer things");
  // Under the limits it draws.
  expect(loft("frames", `${FIX}/no_whole.ts`, "--times", "0", "--view", "tip", "--size", "32x32", "--out", `${mkdtempSync(join(tmpdir(), "loft-b-"))}/`, "--budget", "triangles=120000,draws=96").code).toBe(0);
});

test("an absent camera is the command's mistake (exit 2, usage), naming the cameras there are", () => {
  const r = loft("render", `${FIX}/no_whole.ts`, "--views", "nope", "--size", "32x32", "--out", "out/never.webp");
  expect(r.code).toBe(2);
  expect(lastLine(r.err)).toBe(`loft: usage: no camera "nope": the scene names [tip], defaults are [front, side, top, three]`);
});

test("a strand without paint is a scene refusal (exit 4) naming the fix, not an internal TypeError", () => {
  const r = loft("frames", `${FIX}/no_paint.ts`, "--times", "0", "--view", "front", "--size", "32x32", "--out", `${mkdtempSync(join(tmpdir(), "loft-p-"))}/`);
  expect(r.code).toBe(4);
  expect(lastLine(r.err)).toBe(`loft: scene: strand "bare": given paint undefined; expected paint: a colour [r, g, b] in 0..1, or a function of the body coordinates returning vec3. Fix: paint: [0.8, 0.4, 0.3] (at tests/fixtures/host/no_paint.ts:7)`);
});
