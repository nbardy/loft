// Command-line parsing. Every command is one variant of `Command`; main.ts dispatches on `kind` and nothing else
// looks at argv. Bad input throws `UsageError` naming the offending flag.
import { parseGalleryArgs, type GalleryCommand } from "./gallery_args.ts";

import type { OutlineMethod } from "../scene.ts";
import { parseInputs, parseSweep, type InputValues, type Sweep } from "./inputs.ts";

export class UsageError extends Error {}

/** The style a scene is drawn in: its own, or one of `src/style/index.ts`'s by name (`--style`). */
export type StyleChoice = { readonly kind: "scene" } | { readonly kind: "named"; readonly name: string };
export type Source = { readonly kind: "file"; readonly path: string; readonly style: StyleChoice } | { readonly kind: "fixture" };
export type Size = { readonly width: number; readonly height: number };
export type ViewName = string;

type Shared = { readonly source: Source; readonly size: Size; readonly params: Readonly<Record<string, number>>; readonly inputs: InputValues };

export type Command =
  | GalleryCommand
  | (Shared & { readonly kind: "render"; readonly out: string; readonly views: readonly ViewName[]; readonly time: number; readonly construction: boolean })
  | (Shared & { readonly kind: "clip"; readonly out: string; readonly seconds: number; readonly fps: number; readonly view: ViewName; readonly sweep: Sweep })
  | (Shared & { readonly kind: "frames"; readonly out: string; readonly times: readonly number[]; readonly view: ViewName; readonly sweep: Sweep })
  | { readonly kind: "dev"; readonly source: Source; readonly port: number }
  | { readonly kind: "check"; readonly path: string }
  | { readonly kind: "profile"; readonly path: string; readonly style: StyleChoice; readonly view: ViewName; readonly size: Size; readonly frames: number; readonly time: number; readonly outline: "scene" | OutlineMethod }
  | { readonly kind: "bake"; readonly source: Source; readonly path: string; readonly out: string; readonly time: number; readonly density: { readonly along: number; readonly around: number } };

export const USAGE = `usage: loft <command> <scene.ts | --fixture> [flags]
  render <scene> --out f.webp [--views front,side,top,three] [--time 1.0] [--size 1280x800] [--param name=value ...] [--construction]
  clip   <scene> --out f.mp4  [--seconds 8] [--fps 30] [--view three] [--size 1280x800]
  frames <scene> --out f.webp --times 0,0.5,1 [--view three] [--size 1280x800]
  dev    <scene> [--port 5173]
  profile <scene> [--view game] [--size 1280x800] [--frames 30] [--time 1.0] [--style name] [--outline hull|screen]   (GPU ms per draw, best of frames)
  check  <scene>   (runs the scene module's exported \`checks\` on the CPU; exits 1 if any fails)
  --style toon|woodblock|biolume|papercraft  (render, clip, frames, dev) draws the scene in that style instead of its own
  gallery [--out out/gallery] [--scenes "examples/*.ts,examples/scenes/*.ts"] [--styles toon,woodblock,biolume,papercraft] [--size 960x600] [--clip-seconds 6] [--only a,b] [--force]
  --input name=value  (render, clip, frames; repeatable) sets a host input: a scalar, or x,y,z for a vector
  --sweep name=a..b   (clip, frames) moves one input evenly from a to b across the frames; with one --times value,
                      frames draws --count frames (default 6) at that time
  bake   <scene> --out f.glb  [--time 1.0] [--density 48x16]   (glTF mesh of the bodies at one time; the sea is not baked)`;

const FLAGS: Record<string, { readonly value: boolean; readonly commands: readonly string[] }> = {
  "--fixture": { value: false, commands: ["render", "clip", "frames", "dev"] },
  "--out": { value: true, commands: ["render", "clip", "frames", "bake"] },
  "--size": { value: true, commands: ["render", "clip", "frames", "profile"] },
  "--frames": { value: true, commands: ["profile"] },
  "--outline": { value: true, commands: ["profile"] },
  "--param": { value: true, commands: ["render", "clip", "frames"] },
  "--input": { value: true, commands: ["render", "clip", "frames"] },
  "--sweep": { value: true, commands: ["clip", "frames"] },
  "--count": { value: true, commands: ["frames"] },
  "--views": { value: true, commands: ["render"] },
  "--time": { value: true, commands: ["render", "bake", "profile"] },
  "--density": { value: true, commands: ["bake"] },
  "--construction": { value: false, commands: ["render"] },
  "--seconds": { value: true, commands: ["clip"] },
  "--fps": { value: true, commands: ["clip"] },
  "--view": { value: true, commands: ["clip", "frames", "profile"] },
  "--times": { value: true, commands: ["frames"] },
  "--port": { value: true, commands: ["dev"] },
  "--style": { value: true, commands: ["render", "clip", "frames", "dev", "profile"] },
};

function num(flag: string, text: string): number {
  const n = Number(text);
  if (text.trim() === "" || !Number.isFinite(n)) throw new UsageError(`${flag}: "${text}" is not a number`);
  return n;
}

function positive(flag: string, text: string): number {
  const n = num(flag, text);
  if (!(n > 0)) throw new UsageError(`${flag}: ${text} must be positive`);
  return n;
}

export function parseSize(text: string): Size {
  const m = /^(\d+)x(\d+)$/.exec(text);
  if (!m || Number(m[1]) < 1 || Number(m[2]) < 1) throw new UsageError(`--size: "${text}" is not WIDTHxHEIGHT, e.g. 1280x800`);
  return { width: Number(m[1]), height: Number(m[2]) };
}

function parseParam(text: string): [string, number] {
  const i = text.lastIndexOf("=");
  if (i <= 0) throw new UsageError(`--param: "${text}" is not name=value`);
  return [text.slice(0, i), num(`--param ${text.slice(0, i)}`, text.slice(i + 1))];
}

function outlineOf(text: string | undefined): "scene" | OutlineMethod {
  switch (text) {
    case undefined: return "scene";
    case "hull": case "screen": return text;
    default: throw new UsageError(`--outline: "${text}" is not hull or screen`);
  }
}

export function parseArgs(argv: readonly string[]): Command {
  const [name, ...rest] = argv;
  if (name === undefined) throw new UsageError("no command given");
  if (!["render", "clip", "frames", "dev", "bake", "check", "gallery", "profile"].includes(name)) throw new UsageError(`unknown command "${name}"`);
  if (name === "gallery") return parseGalleryArgs(rest);

  const flags = new Map<string, string[]>();
  const positional: string[] = [];
  for (let i = 0; i < rest.length; i++) {
    const a = rest[i]!;
    if (!a.startsWith("--")) { positional.push(a); continue; }
    const spec = FLAGS[a];
    if (!spec) throw new UsageError(`unknown flag ${a}`);
    if (!spec.commands.includes(name)) throw new UsageError(`flag ${a} does not apply to "${name}"`);
    const list = flags.get(a) ?? [];
    if (spec.value) {
      const v = rest[++i];
      if (v === undefined || v.startsWith("--")) throw new UsageError(`flag ${a} needs a value`);
      list.push(v);
    } else list.push("");
    flags.set(a, list);
  }
  for (const [flag, values] of flags) if (flag !== "--param" && flag !== "--input" && values.length > 1) throw new UsageError(`flag ${flag} given twice`);

  const one = (flag: string): string | undefined => flags.get(flag)?.[0];
  const fixture = flags.has("--fixture");
  if (positional.length > 1) throw new UsageError(`unexpected argument "${positional[1]}"`);
  if (fixture && positional.length === 1) throw new UsageError("give either a scene file or --fixture, not both");
  if (!fixture && positional.length === 0) throw new UsageError(`${name}: needs a scene file (or --fixture)`);
  const styleName = one("--style");
  if (fixture && styleName !== undefined) throw new UsageError("--style: the fixture is a compiled artefact, not a scene; it has no style to change");
  const source: Source = fixture ? { kind: "fixture" } : { kind: "file", path: positional[0]!, style: styleName === undefined ? { kind: "scene" } : { kind: "named", name: styleName } };

  if (name === "check") {
    if (source.kind !== "file") throw new UsageError("check: needs a scene file (a fixture has no checks)");
    return { kind: "check", path: source.path };
  }
  if (name === "profile") {
    if (source.kind !== "file") throw new UsageError("profile: needs a scene file");
    const frames = one("--frames") === undefined ? 30 : positive("--frames", one("--frames")!);
    if (!Number.isInteger(frames)) throw new UsageError(`--frames: ${frames} is not a whole number`);
    return {
      kind: "profile", path: source.path, style: source.style, view: one("--view") ?? "game", size: parseSize(one("--size") ?? "1280x800"), frames,
      time: one("--time") === undefined ? 1.0 : num("--time", one("--time")!),
      outline: outlineOf(one("--outline")),
    };
  }
  if (name === "dev") return { kind: "dev", source, port: one("--port") === undefined ? 5173 : positive("--port", one("--port")!) };

  const out = one("--out");
  if (out === undefined) throw new UsageError(`${name}: --out is required`);
  const wantExt = name === "clip" ? ".mp4" : name === "bake" ? ".glb" : ".webp";
  if (!out.endsWith(wantExt)) throw new UsageError(`--out: ${name} writes ${wantExt} files, got "${out}"`);
  if (name === "bake") {
    const d = /^(\d+)x(\d+)$/.exec(one("--density") ?? "48x16");
    if (!d || Number(d[1]) < 2 || Number(d[2]) < 3) throw new UsageError(`--density: "${one("--density")}" is not ALONGxAROUND (at least 2x3), e.g. 48x16`);
    return {
      kind: "bake", source, path: positional[0]!, out, density: { along: Number(d[1]), around: Number(d[2]) },
      time: one("--time") === undefined ? 1.0 : num("--time", one("--time")!),
    };
  }
  const shared: Shared = {
    source,
    size: parseSize(one("--size") ?? "1280x800"),
    params: Object.fromEntries((flags.get("--param") ?? []).map(parseParam)),
    inputs: parseInputs(flags.get("--input") ?? []),
  };
  const sweep = parseSweep(one("--sweep"));

  switch (name) {
    case "render":
      return {
        ...shared, kind: "render", out,
        views: (one("--views") ?? "front,side,top,three").split(",").map((s) => s.trim()).filter((s) => s !== ""),
        time: one("--time") === undefined ? 1.0 : num("--time", one("--time")!),
        construction: flags.has("--construction"),
      };
    case "clip":
      return {
        ...shared, kind: "clip", out,
        seconds: one("--seconds") === undefined ? 8 : positive("--seconds", one("--seconds")!),
        fps: one("--fps") === undefined ? 30 : positive("--fps", one("--fps")!),
        view: one("--view") ?? "three", sweep,
      };
    case "frames": {
      const t = one("--times");
      if (t === undefined) throw new UsageError("frames: --times is required, e.g. --times 0,0.5,1");
      const listed = t.split(",").map((s) => num("--times", s));
      return { ...shared, kind: "frames", out, times: frameTimes(listed, sweep, one("--count")), view: one("--view") ?? "three", sweep };
    }
    default:
      throw new UsageError(`unhandled command "${name}"`);
  }
}

/** A sweep at one time draws `--count` frames there; otherwise one frame per listed time (and no `--count`). */
function frameTimes(listed: readonly number[], sweep: Sweep, count: string | undefined): readonly number[] {
  if (count === undefined) return listed.length === 1 && sweep.kind === "linear" ? Array(6).fill(listed[0]) : listed;
  if (sweep.kind === "none" || listed.length !== 1) throw new UsageError("--count: only with --sweep and a single --times value");
  const n = positive("--count", count);
  if (!Number.isInteger(n) || n < 2) throw new UsageError(`--count: ${count} must be a whole number of at least 2`);
  return Array(n).fill(listed[0]);
}
