// The crab lineup's deliverables from one load of the scene (four crabs take minutes to load; examples/bosses/crab_lineup.ts,
// shots in examples/bosses/lineup_shots.ts, docs/lineup.md "Staging"):
//
//   bun tools/lineup_deliver.ts <out dir> strip|all [--line islander,fiddler,mask,stilt] [--look default]
//                               [--formats wide,square] [--fps 30,60] [--pick t,t,...] [--strip-width 960]
//
//   strip   per format: a frame per bar and one per accent of the score (two sheets), at --strip-width (default 960)
//   all     the strips, the seam proof, the frame time, a clip of exactly one period per format and frame rate,
//           a 3 x 3 sheet of the --pick instants (the first is the thumbnail), and README.txt
//
// --line: the four variants left to right (a variant may carry picks: `stilt,face=mask` is not possible here since the
// list is comma separated; use `+` for picks: `stilt+face=mask`). --look: a look of `LOOKS` in the scene; an unknown one is
// refused. Each format's clip names: wide lineup_1080p.mp4 (30 fps) and lineup_1080p_<fps>fps.mp4; square
// lineup_square.mp4 and lineup_square_<fps>fps.mp4. H.264 yuv420p, exactly one period each.
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { loopTime, OPEN, type Camera } from "../src/artefact.ts";
import { contactSheet, writeWebp, type Frame } from "../src/cli/image.ts";
import { openVideo } from "../src/cli/video.ts";
import { compile } from "../src/compile/index.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { createHost } from "../src/host/host.ts";
import { FAN, lineupScene, LINE, SCORE } from "../examples/bosses/crab_lineup.ts";
import { BAR, BARS, BEAT, BPM, FORMATS, hostInputs, PERIOD, shotAt, type Format, type FormatName } from "../examples/bosses/lineup_shots.ts";

const argv = process.argv.slice(2);
const flag = (name: string, d: string): string => { const i = argv.indexOf(name); if (i < 0) return d; const v = argv[i + 1]; if (v === undefined) throw new Error(`${name} needs a value`); argv.splice(i, 2); return v; };
const lineArg = flag("--line", LINE.join(",")).split(",").map((v) => v.replaceAll("+", ","));
const look = flag("--look", "default");
const formatNames = flag("--formats", "wide,square").split(",");
const rates = flag("--fps", "30,60").split(",").map(Number);
const pick = flag("--pick", "").split(",").filter((x) => x !== "").map(Number);
const stripWidth = Number(flag("--strip-width", "960"));
const [dir, what] = argv as [string | undefined, string | undefined];
if (dir === undefined || (what !== "strip" && what !== "all")) throw new Error("usage: bun tools/lineup_deliver.ts <out dir> strip|all [--line a,b,c,d] [--look name] [--formats wide,square] [--fps 30,60] [--pick t,...] [--strip-width 960]");
const formats: Format[] = formatNames.map((n) => { if (!(n in FORMATS)) throw new Error(`no format "${n}": [${Object.keys(FORMATS).join(", ")}]`); return FORMATS[n as FormatName]; });
for (const r of rates) if (!(r > 0 && Number.isInteger(r))) throw new Error(`--fps: whole frame rates, got ${r}`);
mkdirSync(dir, { recursive: true });

let t0 = performance.now();
const artefact = compile(lineupScene({ line: lineArg, look }).scene);
const compileMs = performance.now() - t0;
t0 = performance.now();
const loaded = await createHost(await headlessDevice()).load(artefact);
const loadMs = performance.now() - t0;
const loop = artefact.loop ?? OPEN;
console.log(`line ${lineArg.join(" | ")}; look ${look}; compile ${(compileMs / 1000).toFixed(1)} s, load ${(loadMs / 1000).toFixed(1)} s; ${artefact.stats.draws} draws`);

// The host's boats (the landed thrown boats parked between cuts): every frame of every deliverable reads them.
const members = lineArg.map((_, k) => `crab${k}`);
const at = (camera: Camera, t: number, width: number, height: number): Promise<Uint8Array> =>
  loaded.frame({ camera, width, height, time: loopTime(loop, t), inputs: hostInputs(members, (k) => FAN[k]!, t) });
/** A frame of a format at time t, its camera the shot's, at a size scaled to `width`. */
const shot = (f: Format, t: number, width: number): Promise<Frame> => { const height = Math.round((width * f.height) / f.width / 2) * 2; return at(shotAt(f, t).camera, t, width, height).then((rgba) => ({ width, height, rgba })); };
const sheet = async (name: string, f: Format, times: readonly number[], width: number): Promise<void> => {
  const frames: Frame[] = [];
  for (const t of times) frames.push(await shot(f, t, width));
  const s = contactSheet(frames);
  writeWebp(`${dir}/${name}`, s);
  console.log(`wrote ${dir}/${name}  ${s.width}x${s.height}  times [${times.map((t) => +t.toFixed(3)).join(", ")}]`);
};
const suffix = (f: Format) => (f.name === "wide" ? "" : `_${f.name}`);

// One frame per bar, a beat into it (so each bar's first accent shows landed); and the score's accents themselves.
const bars = Array.from({ length: BARS }, (_, b) => b * BAR + BEAT);
const accents = [SCORE.slam(0), SCORE.slam(1), SCORE.slam(2), SCORE.slam(3), SCORE.slam(3) + 1.5, SCORE.grip, SCORE.grip + 0.5, SCORE.release - 0.25, SCORE.release + 0.3, SCORE.release + 1.5, SCORE.lands + 0.15, ...SCORE.taunt(0).slice(0, 2), SCORE.taunt(1)[0]!, SCORE.taunt(0)[4]!, SCORE.taunt(3)[4]!, SCORE.taunt(0)[5]!, SCORE.taunt(0)[6]!];
for (const f of formats) {
  await sheet(`strip_per_bar${suffix(f)}.webp`, f, bars, f.width === f.height ? Math.round(stripWidth * 0.5625) : stripWidth);
  await sheet(`strip_accents${suffix(f)}.webp`, f, accents, f.width === f.height ? Math.round(stripWidth * 0.5625) : stripWidth);
}

if (what === "all") {
  const hash = (px: Uint8Array): string => createHash("sha256").update(px).digest("hex").slice(0, 16);
  const change = (a: Uint8Array, b: Uint8Array): number => { let s = 0; for (let i = 0; i < a.length; i++) s += Math.abs(a[i]! - b[i]!); return s / a.length; };
  const seam: string[] = [];
  for (const f of formats) {
    // ---- the seam: one frame at 0 and at the period; the seam is a cut (the taunt's last frame to the slam's first),
    // so what must also hold is that the shot running into it has no jump of its own: its last two frames change as
    // ordinary consecutive frames do.
    const W = f.width, H = f.height, dt = 1 / 30, px = (t: number) => at(shotAt(f, t).camera, t, W, H);
    const first = await px(0), again = await px(PERIOD), last = await px(PERIOD - dt), before = await px(PERIOD - 2 * dt);
    const ordinary: number[] = [];
    for (let k = 0; k < 2 * BARS; k++) { const t = (k + 0.37) * (PERIOD / (2 * BARS)); ordinary.push(change(await px(t), await px(t + dt))); }
    const sorted = [...ordinary].sort((x, y) => x - y), median = sorted[Math.floor(sorted.length / 2)]!;
    seam.push(
      `${f.name} ${W}x${H}:`,
      `  frame at t = 0: ${hash(first)}; at t = ${PERIOD} (the period): ${hash(again)}   ${hash(first) === hash(again) ? "the same picture" : "DIFFERENT"}`,
      `  across the seam (t = ${(PERIOD - dt).toFixed(4)} to 0) is a cut, as at every shot change: mean change of a channel ${change(last, first).toFixed(3)}`,
      `  the two frames before it (t = ${(PERIOD - 2 * dt).toFixed(4)} to ${(PERIOD - dt).toFixed(4)}): ${change(before, last).toFixed(3)}; ordinary consecutive frames at ${ordinary.length} instants: median ${median.toFixed(3)}, least ${sorted[0]!.toFixed(3)}, most ${sorted[sorted.length - 1]!.toFixed(3)}: ${(change(before, last) / median).toFixed(2)} times the median`,
    );
    writeWebp(`${dir}/seam_frames${suffix(f)}.webp`, contactSheet([{ width: W, height: H, rgba: before }, { width: W, height: H, rgba: last }, { width: W, height: H, rgba: first }].map((x) => x)));
  }
  writeFileSync(`${dir}/seam_proof.txt`, seam.join("\n") + "\n");
  console.log(seam.join("\n"));

  // ---- the frame time (GPU, normal frames, each format's first shot at its own size) ----
  const timing: string[] = [];
  for (const f of formats) {
    const rounds: number[] = [];
    for (let r = 0; r < 8; r++) rounds.push((await loaded.frameGpu({ camera: shotAt(f, 1).camera, width: f.width, height: f.height, time: 1 }, 20)).ms);
    const s = [...rounds].sort((x, y) => x - y);
    timing.push(`normal frame, ${f.name} ${f.width} x ${f.height}, t = 1: least ${s[0]!.toFixed(2)} ms, median ${s[4]!.toFixed(2)} ms GPU (8 rounds, least of 20 frames each)`);
  }
  console.log(timing.join("\n"));

  // ---- the pick: a 3 x 3 sheet of chosen instants of the wide format (the first the thumbnail) ----
  if (pick.length > 0) {
    const f = formats.find((x) => x.name === "wide") ?? formats[0]!;
    await sheet("picks.webp", f, pick.slice(0, 9), 640);
    const thumb = await shot(f, pick[0]!, f.width);
    writeWebp(`${dir}/thumbnail.webp`, contactSheet([thumb]));
  }

  // ---- the clips: exactly one period per format and rate ----
  for (const f of formats) for (const fps of rates) {
    const n = Math.round(PERIOD * fps), name = `${f.name === "wide" ? "lineup_1080p" : `lineup_${f.name}`}${fps === 30 ? "" : `_${fps}fps`}.mp4`;
    const video = openVideo(`${dir}/${name}`, f.width, f.height, fps);
    const start = performance.now();
    for (let i = 0; i < n; i++) await video.write(await at(shotAt(f, i / fps).camera, i / fps, f.width, f.height));
    await video.finish();
    console.log(`wrote ${dir}/${name}  ${n} frames at ${fps} fps (${((performance.now() - start) / 1000).toFixed(0)} s)`);
  }

  const beats = (ts: readonly number[]): string => ts.map((t) => `${t.toFixed(1)} s (bar ${Math.floor(t / BAR) + 1} beat ${((t % BAR) / BEAT + 1).toFixed(0)})`).join(", ");
  writeFileSync(`${dir}/README.txt`, [
    "The crab lineup: examples/bosses/crab_lineup.ts, shots in examples/bosses/lineup_shots.ts (docs/lineup.md). No audio.",
    "",
    `Tempo ${BPM} BPM, 4/4. A beat is ${BEAT} s, a bar ${BAR} s. ${BARS} bars: the period is ${PERIOD} s, and each clip is exactly one period, so it loops when repeated.`,
    `Left to right: ${lineArg.join(", ")}. Look: ${look}.`,
    "",
    "Shots (each cut on a downbeat; the camera at the period is the camera at 0):",
    ...formats.flatMap((f) => f.cuts.map((c) => `  ${f.name}: from ${beats([c.at])}, ${c.shot.name} (${c.shot.kind})`)),
    "",
    "Accents (seconds from the loop's first downbeat; every one is on a beat):",
    `  slam, a cascade: claws start up at ${lineArg.map((_, k) => (SCORE.slam(k) - 3 * BEAT).toFixed(1)).join(", ")} s and land at ${beats(lineArg.map((_, k) => SCORE.slam(k)))}`,
    `  throw, in unison: grips close ${beats([SCORE.grip])}; squeezes ${beats([SCORE.grip + BEAT, SCORE.grip + 3 * BEAT])}; the whip starts ${beats([SCORE.release - BEAT])}; all four let go ${beats([SCORE.release])}; the boats come down ${beats([SCORE.lands])}`,
    ...lineArg.map((name, k) => `  taunt, ${name}: ${beats(SCORE.taunt(k))}`),
    "",
    ...seam, "", ...timing,
    `compile ${(compileMs / 1000).toFixed(0)} s, load ${(loadMs / 1000).toFixed(0)} s; ${artefact.stats.draws} draws`,
  ].join("\n") + "\n");
  console.log(`wrote ${dir}/README.txt`);
}
loaded.destroy();
process.exit(0);
