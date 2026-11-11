// The crab lineup's deliverables from one load of the scene (four crabs take minutes to load; examples/bosses/crab_lineup.ts):
//   bun tools/lineup_deliver.ts <out dir> strip            a frame per bar, and one per accent of the score (two sheets)
//   bun tools/lineup_deliver.ts <out dir> all [fps ...]    the sheets, the seam proof, the frame time, a clip of exactly
//                                                          one period per frame rate (default 30), and README.txt
// The seam proof: the hash of the frame at t = 0 and at t = period (the scene declares its period, so a host wraps its
// clock and they are one frame), and what that alone does not show, that nothing jumps: the mean change of a pixel from
// the last frame of the loop to the first, beside the mean change between consecutive frames elsewhere.
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { loopTime, OPEN } from "../src/artefact.ts";
import { resolveCamera } from "../src/cli/cameras.ts";
import { contactSheet, writeWebp, type Frame } from "../src/cli/image.ts";
import { openVideo } from "../src/cli/video.ts";
import { compile } from "../src/compile/index.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { createHost } from "../src/host/host.ts";
import scene, { BAR, BARS, BEAT, BPM, LINE, PERIOD, SCORE } from "../examples/bosses/crab_lineup.ts";

const [dir, what, ...rates] = process.argv.slice(2) as [string, string, ...string[]];
if (dir === undefined || (what !== "strip" && what !== "all")) throw new Error("usage: bun tools/lineup_deliver.ts <out dir> strip | all [fps ...]");
mkdirSync(dir, { recursive: true });

let t0 = performance.now();
const artefact = compile(scene);
const compileMs = performance.now() - t0;
t0 = performance.now();
const device = await headlessDevice();
const loaded = await createHost(device).load(artefact);
const loadMs = performance.now() - t0;
const camera = resolveCamera(artefact, "lineup"), loop = artefact.loop ?? OPEN;
console.log(`compile ${(compileMs / 1000).toFixed(1)} s, load ${(loadMs / 1000).toFixed(1)} s; ${artefact.stats.draws} draws, at most ${artefact.stats.maxTriangles} triangles`);

const at = (t: number, width: number, height: number): Promise<Uint8Array> => loaded.frame({ camera, width, height, time: loopTime(loop, t) });
const sheet = async (name: string, times: readonly number[], width: number, height: number): Promise<void> => {
  const frames: Frame[] = [];
  for (const t of times) frames.push({ width, height, rgba: await at(t, width, height) });
  const s = contactSheet(frames);
  writeWebp(`${dir}/${name}`, s);
  console.log(`wrote ${dir}/${name}  ${s.width}x${s.height}  times [${times.map((t) => +t.toFixed(3)).join(", ")}]`);
};

// One frame per bar, a beat into it (so each bar's first accent shows landed); and the score's accents themselves.
const bars = Array.from({ length: BARS }, (_, b) => b * BAR + BEAT);
const accents = [SCORE.slam(0), SCORE.slam(1), SCORE.slam(2), SCORE.slam(3), SCORE.slam(3) + 1.5, SCORE.grip, SCORE.grip + 0.5, SCORE.release - 0.25, SCORE.release + 0.3, SCORE.release + 1.5, SCORE.lands + 0.15, ...SCORE.taunt(0).slice(0, 2), SCORE.taunt(1)[0]!, SCORE.taunt(0)[4]!, SCORE.taunt(3)[4]!, SCORE.taunt(0)[5]!, SCORE.taunt(0)[6]!];
await sheet("strip_per_bar.webp", bars, 960, 540);
await sheet("strip_accents.webp", accents, 960, 540);

if (what === "all") {
  const W = 1920, H = 1080, hash = (px: Uint8Array): string => createHash("sha256").update(px).digest("hex").slice(0, 16);
  // ---- the seam ----
  const dt = 1 / 30, first = await at(0, W, H), again = await at(PERIOD, W, H), last = await at(PERIOD - dt, W, H);
  const change = (a: Uint8Array, b: Uint8Array): number => { let s = 0; for (let i = 0; i < a.length; i++) s += Math.abs(a[i]! - b[i]!); return s / a.length; };
  const across = change(last, first);
  // Ordinary consecutive frames, sampled through the loop (two a bar).
  const ordinary: number[] = [];
  for (let k = 0; k < 2 * BARS; k++) { const t = (k + 0.37) * (PERIOD / (2 * BARS)); ordinary.push(change(await at(t, W, H), await at(t + dt, W, H))); }
  const sorted = [...ordinary].sort((x, y) => x - y), median = sorted[Math.floor(sorted.length / 2)]!, most = sorted[sorted.length - 1]!;
  const seam = [
    `frame at t = 0:            ${hash(first)}`,
    `frame at t = ${PERIOD} (period): ${hash(again)}   ${hash(first) === hash(again) ? "the same picture" : "DIFFERENT"}`,
    `mean change of a channel (0..255) from the loop's last frame (t = ${(PERIOD - dt).toFixed(4)}) to its first: ${across.toFixed(4)}`,
    `the same between consecutive frames at ${ordinary.length} other instants: median ${median.toFixed(4)}, least ${sorted[0]!.toFixed(4)}, most ${most.toFixed(4)}`,
    `across the seam is ${(across / median).toFixed(2)} times the median`,
  ];
  writeFileSync(`${dir}/seam_proof.txt`, seam.join("\n") + "\n");
  writeWebp(`${dir}/seam_frames.webp`, contactSheet([{ width: W / 2, height: H / 2, rgba: await at(PERIOD - dt, W / 2, H / 2) }, { width: W / 2, height: H / 2, rgba: await at(0, W / 2, H / 2) }, { width: W / 2, height: H / 2, rgba: await at(dt, W / 2, H / 2) }]));
  console.log(seam.join("\n"));

  // ---- the frame time (GPU, normal frames; 1280 x 800 as the single crab's is measured, and the clip's own size) ----
  const timing: string[] = [];
  for (const [w, h] of [[1280, 800], [W, H]] as const) {
    const rounds: number[] = [];
    for (let r = 0; r < 8; r++) rounds.push((await loaded.frameGpu({ camera, width: w, height: h, time: 1 }, 20)).ms);
    const s = [...rounds].sort((x, y) => x - y);
    timing.push(`normal frame, ${w} x ${h}, t = 1: least ${s[0]!.toFixed(2)} ms, median ${s[4]!.toFixed(2)} ms, most ${s[7]!.toFixed(2)} ms GPU (8 rounds, least of 20 frames each)`);
  }
  console.log(timing.join("\n"));

  // ---- the clips: exactly one period ----
  for (const fps of rates.length > 0 ? rates.map(Number) : [30]) {
    const n = Math.round(PERIOD * fps), name = `lineup_1080p${fps === 30 ? "" : `_${fps}fps`}.mp4`;
    const video = openVideo(`${dir}/${name}`, W, H, fps);
    const start = performance.now();
    for (let i = 0; i < n; i++) await video.write(await at(i / fps, W, H));
    await video.finish();
    console.log(`wrote ${dir}/${name}  ${n} frames at ${fps} fps (${((performance.now() - start) / 1000).toFixed(0)} s)`);
  }

  const beats = (ts: readonly number[]): string => ts.map((t) => `${t.toFixed(1)} s (bar ${Math.floor(t / BAR) + 1} beat ${((t % BAR) / BEAT + 1).toFixed(0)})`).join(", ");
  writeFileSync(`${dir}/README.txt`, [
    "The crab lineup: examples/bosses/crab_lineup.ts, camera `lineup`. No audio.",
    "",
    `Tempo ${BPM} BPM, 4/4. A beat is ${BEAT} s, a bar ${BAR} s. ${BARS} bars: the period is ${PERIOD} s, and the clip is exactly one period, so it loops when repeated.`,
    `Left to right: ${LINE.join(", ")}.`,
    "",
    "Accents (seconds from the loop's first downbeat; every one is on a beat):",
    `  slam, a cascade: claws start up at ${LINE.map((_, k) => (SCORE.slam(k) - 3 * BEAT).toFixed(1)).join(", ")} s and land at ${beats(LINE.map((_, k) => SCORE.slam(k)))}`,
    `  throw, in unison: grips close ${beats([SCORE.grip])}; squeezes ${beats([SCORE.grip + BEAT, SCORE.grip + 3 * BEAT])}; the whip starts ${beats([SCORE.release - BEAT])}; all four let go ${beats([SCORE.release])}; the boats come down ${beats([SCORE.lands])}`,
    ...LINE.map((name, k) => `  taunt, ${name}: ${beats(SCORE.taunt(k))}`),
    "",
    ...seam, "", ...timing,
    `compile ${(compileMs / 1000).toFixed(0)} s, load ${(loadMs / 1000).toFixed(0)} s; ${artefact.stats.draws} draws, at most ${artefact.stats.maxTriangles} triangles`,
  ].join("\n") + "\n");
  console.log(`wrote ${dir}/README.txt`);
}
loaded.destroy();
process.exit(0);
