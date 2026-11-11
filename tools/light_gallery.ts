// The lighting library drawn for comparison (docs/lighting.md): every named recipe of the woodblock style, and the print
// as it is, on the crab from four cameras, on another boss, and on the four-crab lineup; then labelled sheets.
//
//   bun tools/light_gallery.ts stills [--only cel,rim]     one process per scene and recipe (a scene per process)
//   bun tools/light_gallery.ts sheets                      the sheets, from the stills (ImageMagick)
//   bun tools/light_gallery.ts clips [--only cel]          8 s of the crab's game camera per recipe, and the sun swung under `cel`
//   bun tools/light_gallery.ts cost [--rounds 3]           the normal frame per recipe, the print as it is measured beside each
//
// Everything goes under out/shade/ (git-ignored).

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { LIGHT } from "../src/style/light.ts";

const OUT = "out/shade";
const CRAB = "examples/bosses/reefback_titan.ts", WYRM = "examples/bosses/maelstrom_wyrm.ts", LINEUP = "examples/bosses/crab_lineup.ts";
/** The print as it is first, then the library. */
const RECIPES = ["asPrinted", ...Object.keys(LIGHT)];
const REFS: Readonly<Record<string, string>> = {
  flat: "shade_flat", twoTone: "shade_two_tone", threeTone: "shade_three_tone", bokashi: "shade_bokashi", hatched: "shade_hatched", contact: "shade_contact", rim: "shade_rim", cel: "shade_best",
};
const REF_DIR = `${process.env.HOME}/git/loft/docs/art_direction/shading`;

const args = process.argv.slice(2);
const flag = (name: string): string | undefined => { const i = args.indexOf(name); return i < 0 ? undefined : args[i + 1]; };
const only = flag("--only")?.split(",");
const chosen = RECIPES.filter((r) => only === undefined || only.includes(r));
const lightOf = (r: string): string[] => (r === "asPrinted" ? [] : ["--light", r]);

function run(cmd: string, argv: readonly string[]): string {
  const r = spawnSync(cmd, argv, { encoding: "utf8", maxBuffer: 1 << 26 });
  if (r.status !== 0) throw new Error(`${cmd} ${argv.join(" ")}\n${r.stdout}\n${r.stderr}`);
  return r.stdout + r.stderr;
}
const loft = (argv: readonly string[]): string => run("bun", ["src/cli/main.ts", ...argv]);

function stills(): void {
  mkdirSync(`${OUT}/stills`, { recursive: true });
  const jobs: { readonly name: string; readonly argv: readonly string[] }[] = chosen.flatMap((r) => [
    { name: `crab_${r}`, argv: ["render", CRAB, "--variant", "bruiser", ...lightOf(r), "--views", "game,three,face,back", "--time", "6.3", "--size", "1280x800"] },
    { name: `wyrm_${r}`, argv: ["render", WYRM, ...lightOf(r), "--views", "game,close", "--time", "3.0", "--size", "1280x800"] },
    { name: `lineup_${r}`, argv: ["render", LINEUP, ...lightOf(r), "--views", "lineup", "--time", "2.5", "--size", "1280x720"] },
  ]);
  // `--scenes crab,wyrm,lineup`: which of the three (default all), in that order, so the cheap ones land first.
  const scenes = (flag("--scenes") ?? "crab,wyrm,lineup").split(",");
  const ordered = scenes.flatMap((sc) => jobs.filter((j) => j.name.startsWith(`${sc}_`)));
  for (const j of ordered) {
    const out = `${OUT}/stills/${j.name}.webp`;
    if (existsSync(out) && !args.includes("--force")) continue;
    const t = performance.now();
    const log = loft([...j.argv, "--out", out]);
    console.log(`${j.name}  ${((performance.now() - t) / 1000).toFixed(0)} s  ${log.split("\n").filter((l) => l.includes("rgba")).map((l) => l.trim().split(/\s+/).slice(-1)[0]).join(" ")}`);
  }
}

/** One tile of a still: crop (w x h + x + y of the 1280 x 800 view at column, row of the sheet), scaled, labelled. */
function tile(src: string, view: readonly [number, number], crop: readonly [number, number, number, number], size: readonly [number, number], label: string, dest: string): void {
  const [w, h, x, y] = crop;
  run("magick", [src, "-crop", `${w}x${h}+${view[0] * 1280 + x}+${view[1] * 800 + y}`, "+repage", "-filter", "point", "-resize", `${size[0]}x${size[1]}!`,
    "-gravity", "NorthWest", "-fill", "white", "-undercolor", "#000000b0", "-pointsize", "20", "-annotate", "+6+4", ` ${label} `, dest]);
}

function sheets(): void {
  const tmp = `${OUT}/tiles`;
  mkdirSync(tmp, { recursive: true });
  const rows: string[] = [];
  for (const r of RECIPES) {
    const crab = `${OUT}/stills/crab_${r}.webp`;
    if (!existsSync(crab)) continue;
    // game at true size (the crab's part of the frame), game doubled on the crab, face, three on the legs and claw, back.
    const parts: [readonly [number, number], readonly [number, number, number, number], readonly [number, number], string][] = [
      [[0, 0], [640, 400, 200, 20], [640, 400], `${r}: game, true size`],
      [[0, 0], [320, 200, 330, 150], [640, 400], "game x2"],
      [[0, 1], [1280, 800, 0, 0], [640, 400], "face"],
      [[1, 0], [800, 500, 380, 150], [640, 400], "three: legs and claw"],
      [[1, 1], [1280, 800, 0, 0], [640, 400], "back"],
    ];
    const files = parts.map(([view, crop, size, label], i) => { const d = `${tmp}/${r}_${i}.png`; tile(crab, view, crop, size, label, d); return d; });
    const ref = REFS[r];
    if (ref !== undefined) {
      const d = `${tmp}/${r}_ref.png`;
      run("magick", [`${REF_DIR}/${ref}.webp`, "-resize", "640x400!", "-gravity", "NorthWest", "-fill", "white", "-undercolor", "#000000b0", "-pointsize", "20", "-annotate", "+6+4", " painted reference ", d]);
      files.push(d);
    } else {
      const d = `${tmp}/${r}_ref.png`;
      run("magick", ["-size", "640x400", "xc:#101018", d]);
      files.push(d);
    }
    const row = `${tmp}/row_${r}.png`;
    run("magick", [...files, "+append", row]);
    rows.push(row);
  }
  if (rows.length > 0) {
    run("magick", [...rows, "-append", "-quality", "92", `${OUT}/sheet_recipes.webp`]);
    // And small enough to take in at once.
    run("magick", [`${OUT}/sheet_recipes.webp`, "-resize", "1920x", "-quality", "90", `${OUT}/sheet_recipes_small.webp`]);
    console.log(`wrote ${OUT}/sheet_recipes.webp and sheet_recipes_small.webp`);
  }

  // The other boss and the lineup: a row per recipe.
  const others: string[] = [];
  for (const r of RECIPES) {
    const wyrm = `${OUT}/stills/wyrm_${r}.webp`, line = `${OUT}/stills/lineup_${r}.webp`;
    if (!existsSync(wyrm) || !existsSync(line)) continue;
    const a = `${tmp}/${r}_w0.png`, b = `${tmp}/${r}_w1.png`, c = `${tmp}/${r}_l.png`;
    tile(wyrm, [0, 0], [1280, 800, 0, 0], [640, 400], `${r}: wyrm, game`, a);
    tile(wyrm, [1, 0], [1280, 800, 0, 0], [640, 400], "wyrm, close", b);
    run("magick", [line, "-resize", "1280x720!", "-gravity", "center", "-background", "#101018", "-extent", "1280x720", "-crop", "1280x400+0+200", "+repage", "-gravity", "NorthWest", "-fill", "white", "-undercolor", "#000000b0", "-pointsize", "20", "-annotate", "+6+4", " lineup, t = 2.5 ", c]);
    const row = `${tmp}/row2_${r}.png`;
    run("magick", [a, b, c, "+append", row]);
    others.push(row);
  }
  if (others.length > 0) { run("magick", [...others, "-append", "-quality", "92", `${OUT}/sheet_other_boss_and_lineup.webp`]); console.log(`wrote ${OUT}/sheet_other_boss_and_lineup.webp`); }

  // The cel recipe beside the painted pictures it answers.
  const cel = `${OUT}/stills/crab_cel.webp`;
  if (existsSync(cel)) {
    const t = (name: string, argv: readonly string[]) => { const d = `${tmp}/cel_${name}.png`; run("magick", [...argv, d]); return d; };
    const lab = (text: string) => ["-gravity", "NorthWest", "-fill", "white", "-undercolor", "#000000b0", "-pointsize", "24", "-annotate", "+8+6", ` ${text} `];
    const ours3 = t("ours3", [cel, "-crop", "1280x800+1280+0", "+repage", ...lab("cel, three")]);
    const ref3 = t("ref3", [`${REF_DIR}/shade_best.webp`, "-resize", "1280x800!", ...lab("painted: shade_best")]);
    const oursG = t("oursG", [cel, "-crop", "1280x800+0+0", "+repage", ...lab("cel, game")]);
    const refG = t("refG", [`${REF_DIR}/shade_best_game.webp`, "-resize", "1280x800!", ...lab("painted: shade_best_game")]);
    const top = t("top", [ours3, ref3, "+append"]), bottom = t("bottom", [oursG, refG, "+append"]);
    run("magick", [top, bottom, "-append", "-quality", "92", `${OUT}/sheet_cel_against_painted.webp`]);
    console.log(`wrote ${OUT}/sheet_cel_against_painted.webp`);
  }
  run("rm", ["-rf", tmp]);
}

function clips(): void {
  mkdirSync(`${OUT}/clips`, { recursive: true });
  for (const r of chosen) {
    const out = `${OUT}/clips/${r}.mp4`;
    if (existsSync(out) && !args.includes("--force")) continue;
    loft(["clip", CRAB, "--variant", "bruiser", ...lightOf(r), "--view", "game", "--seconds", flag("--seconds") ?? "8", "--fps", "30", "--size", "1280x800", "--out", out]);
    console.log(`wrote ${out}`);
  }
  if (only === undefined || only.includes("sun")) {
    // The sun swung through 90 degrees about the vertical under the cel recipe, the crab's clock running: the cast
    // shadows and the terminator must move only as the sun does.
    const out = `${OUT}/clips/cel_sun_swing.mp4`;
    loft(["clip", CRAB, "--variant", "bruiser", "--light", "cel", "--view", "three", "--seconds", "6", "--fps", "30", "--size", "1280x800", "--sweep", "sun=0.15,0.8,0.6..-0.6,0.8,0.15", "--out", out]);
    console.log(`wrote ${out}`);
  }
}

/** The normal frame in ms, as `loft profile` prints it. */
function frameMs(scene: string, extra: readonly string[], view: string, size: string): number {
  const text = loft(["profile", scene, ...extra, "--view", view, "--size", size, "--frames", "30"]);
  const m = /normal frame: ([\d.]+) ms/.exec(text);
  if (!m) throw new Error(`no normal frame in:\n${text.slice(0, 400)}`);
  return Number(m[1]);
}

function cost(): void {
  const rounds = Number(flag("--rounds") ?? "3");
  const which = flag("--scene") ?? "crab";
  const [scene, extra, view, size] = which === "lineup" ? [LINEUP, [] as string[], "lineup", "1280x720"] : [CRAB, ["--variant", "bruiser"], "game", "1280x800"];
  const file = `${OUT}/cost_${which}.json`;
  const seen: Record<string, { base: number[]; with: number[] }> = existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) : {};
  for (let k = 0; k < rounds; k++) {
    for (const r of chosen.filter((x) => x !== "asPrinted")) {
      // Alternated, the print as it is beside each recipe, so a pair shares the GPU's clock state as nearly as it can.
      const base = frameMs(scene, extra, view, size), lit = frameMs(scene, [...extra, "--light", r], view, size);
      (seen[r] ??= { base: [], with: [] }).base.push(base);
      seen[r]!.with.push(lit);
      console.log(`round ${k}  ${r.padEnd(10)} as printed ${base.toFixed(2)}  ${r} ${lit.toFixed(2)}  difference ${(lit - base).toFixed(2)} ms`);
      writeFileSync(file, JSON.stringify(seen, null, 1));
    }
  }
}

const command = args[0];
switch (command) {
  case "stills": stills(); break;
  case "sheets": sheets(); break;
  case "clips": clips(); break;
  case "cost": cost(); break;
  default: console.error("usage: bun tools/light_gallery.ts stills|sheets|clips|cost [--only a,b] [--force]"); process.exit(2);
}
