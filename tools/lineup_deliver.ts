// The crab lineup's deliverables from one load of the scene (five crabs take minutes to load; examples/bosses/crab_lineup.ts,
// shots in examples/bosses/lineup_shots.ts, docs/lineup.md "A drilled group"):
//
//   bun tools/lineup_deliver.ts <out dir> strip|shots|all [--line fiddler,mask,islander,bruiser,stilt] [--look default]
//                               [--formats wide,square] [--fps 30,60] [--pick t,t,...] [--strip-width 960]
//
//   strip   per format: a frame per count of the dance, a tenth of a count after it (one sheet), at --strip-width / 2
//           a tile, and the sync instrument's table (sync.txt: CPU, from the same build)
//   shots   per format: the first frame of every shot (a tenth of a count in), to check the framing of the edit
//   all     the strips, the seam proof, the frame time, a clip of exactly one period per format and frame rate,
//           a 3 x 3 sheet of the --pick instants (the first is the thumbnail), and README.txt
//
// --line: the variants left to right, as many as the dance has members (a variant may carry picks: `stilt,face=mask` is
// not possible here since the list is comma separated; use `+` for picks: `stilt+face=mask`). --look: a look of `LOOKS` in the scene; an unknown one is
// refused. Each format's clip names: wide lineup_1080p.mp4 (30 fps) and lineup_1080p_<fps>fps.mp4; square
// lineup_square.mp4 and lineup_square_<fps>fps.mp4. H.264 yuv420p, exactly one period each.
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { loopTime, OPEN, type Camera } from "../src/artefact.ts";
import { contactSheet, writeWebp, type Frame } from "../src/cli/image.ts";
import { openVideo } from "../src/cli/video.ts";
import { compile, compileScene } from "../src/compile/index.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { createHost } from "../src/host/host.ts";
import { Probe } from "../src/check.ts";
import { DANCE, FAN, lineupScene, LINE } from "../examples/bosses/crab_lineup.ts";
import { BAR, BARS, BEAT, BPM, formats as formatsOf, hostInputs, PERIOD, shotAt, type Format, type FormatName } from "../examples/bosses/lineup_shots.ts";

const argv = process.argv.slice(2);
const flag = (name: string, d: string): string => { const i = argv.indexOf(name); if (i < 0) return d; const v = argv[i + 1]; if (v === undefined) throw new Error(`${name} needs a value`); argv.splice(i, 2); return v; };
const lineArg = flag("--line", LINE.join(",")).split(",").map((v) => v.replaceAll("+", ","));
const look = flag("--look", "default");
const formatNames = flag("--formats", "wide,square").split(",");
const rates = flag("--fps", "30,60").split(",").map(Number);
const pick = flag("--pick", "").split(",").filter((x) => x !== "").map(Number);
const stripWidth = Number(flag("--strip-width", "960"));
const [dir, what] = argv as [string | undefined, string | undefined];
if (dir === undefined || (what !== "strip" && what !== "all" && what !== "shots")) throw new Error("usage: bun tools/lineup_deliver.ts <out dir> strip|shots|all [--line a,b,c,d] [--look name] [--formats wide,square] [--fps 30,60] [--pick t,...] [--strip-width 960]");
const FORMATS = formatsOf(lineArg);
const formats: Format[] = formatNames.map((n) => { if (!(n in FORMATS)) throw new Error(`no format "${n}": [${Object.keys(FORMATS).join(", ")}]`); return FORMATS[n as FormatName]; });
for (const r of rates) if (!(r > 0 && Number.isInteger(r))) throw new Error(`--fps: whole frame rates, got ${r}`);
mkdirSync(dir, { recursive: true });

let t0 = performance.now();
const built = lineupScene({ line: lineArg, look });
const artefact = compile(built.scene);
const compileMs = performance.now() - t0;
// The sync instrument on the CPU from the same build (a few seconds): what the clip should show, measured.
{
  const rows = built.sync.table(new Probe(compileScene(built.scene), {}));
  const lines = [
    `the routine: ${DANCE.members} members (${lineArg.join(", ")}), ${DANCE.period} s; ${rows.length} key-channels measured from each member's rig (CPU)`,
    "count  channel  who          arrival spread  size spread  pass",
    ...rows.map((r) => `${String(r.count).padStart(5)}  ${r.channel.padEnd(7)}  ${r.who.padEnd(11)}  ${(1000 * r.arrival).toFixed(2).padStart(9)} ms  ${(100 * r.size).toFixed(2).padStart(9)}%  ${r.pass ? "yes" : "NO"}`),
    `worst arrival spread ${(1000 * Math.max(...rows.map((r) => r.arrival))).toFixed(3)} ms (limit 16.7); worst size spread ${(100 * Math.max(...rows.map((r) => r.size))).toFixed(3)}% (limit 5); ${rows.filter((r) => !r.pass).length} out of limits`,
  ];
  writeFileSync(`${dir}/sync.txt`, lines.join("\n") + "\n");
  console.log(lines[lines.length - 1]);
}
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

if (what === "shots") {
  for (const f of formats) await sheet(`shots${suffix(f)}.webp`, f, f.cuts.map((c) => c.at + 0.1 * BEAT), f.width === f.height ? Math.round(stripWidth * 0.5625) : stripWidth);
  loaded.destroy();
  process.exit(0);
}

// One frame per count, a tenth of a count after it (so each count's key shows landed): 64 tiles, eight a row.
const counts = Array.from({ length: Math.round(PERIOD / BEAT) }, (_, c) => (c + 0.1) * BEAT);
for (const f of formats) await sheet(`strip_per_count${suffix(f)}.webp`, f, counts, f.width === f.height ? Math.round(stripWidth * 0.28) : Math.round(stripWidth / 2));

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
    `Left to right: ${lineArg.join(", ")}. Look: ${look}. The dance's score: examples/bosses/crab_lineup.ts (DANCE); every count's frame: strip_per_count.webp; the sync instrument: sync.txt.`,
    "",
    "Shots (each cut on a downbeat; the camera at the period is the camera at 0):",
    ...formats.flatMap((f) => f.cuts.map((c) => `  ${f.name}: from ${beats([c.at])}, ${c.shot.name} (${c.shot.kind})`)),
    "",
    "The attacks (seconds from the loop's first downbeat), in unison:",
    `  slam: five claws up on ${beats([9 * BEAT])}, down on ${beats([12 * BEAT])}`,
    `  throw: grips close ${beats([21 * BEAT])}; squeezes ${beats([22 * BEAT, 24 * BEAT])}; the whip ${beats([25 * BEAT])}; five boats let go ${beats([26 * BEAT])}; they come down ${beats([32 * BEAT])}`,
    "",
    ...seam, "", ...timing,
    `compile ${(compileMs / 1000).toFixed(0)} s, load ${(loadMs / 1000).toFixed(0)} s; ${artefact.stats.draws} draws`,
  ].join("\n") + "\n");
  console.log(`wrote ${dir}/README.txt`);
}
loaded.destroy();
process.exit(0);
