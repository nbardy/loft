// Command-line parsing. Every command is one variant of `Command`; main.ts dispatches on `kind` and nothing else
// looks at argv. Bad input throws `UsageError` naming the offending flag.

export class UsageError extends Error {}

export type Source = { readonly kind: "file"; readonly path: string } | { readonly kind: "fixture" };
export type Size = { readonly width: number; readonly height: number };
export type ViewName = string;

type Shared = { readonly source: Source; readonly size: Size; readonly params: Readonly<Record<string, number>> };

export type Command =
  | (Shared & { readonly kind: "render"; readonly out: string; readonly views: readonly ViewName[]; readonly time: number; readonly construction: boolean })
  | (Shared & { readonly kind: "clip"; readonly out: string; readonly seconds: number; readonly fps: number; readonly view: ViewName })
  | (Shared & { readonly kind: "frames"; readonly out: string; readonly times: readonly number[]; readonly view: ViewName })
  | { readonly kind: "dev"; readonly source: Source; readonly port: number };

export const USAGE = `usage: loft <command> <scene.ts | --fixture> [flags]
  render <scene> --out f.webp [--views front,side,top,three] [--time 1.0] [--size 1280x800] [--param name=value ...] [--construction]
  clip   <scene> --out f.mp4  [--seconds 8] [--fps 30] [--view three] [--size 1280x800]
  frames <scene> --out f.webp --times 0,0.5,1 [--view three] [--size 1280x800]
  dev    <scene> [--port 5173]`;

const FLAGS: Record<string, { readonly value: boolean; readonly commands: readonly string[] }> = {
  "--fixture": { value: false, commands: ["render", "clip", "frames", "dev"] },
  "--out": { value: true, commands: ["render", "clip", "frames"] },
  "--size": { value: true, commands: ["render", "clip", "frames"] },
  "--param": { value: true, commands: ["render", "clip", "frames"] },
  "--views": { value: true, commands: ["render"] },
  "--time": { value: true, commands: ["render"] },
  "--construction": { value: false, commands: ["render"] },
  "--seconds": { value: true, commands: ["clip"] },
  "--fps": { value: true, commands: ["clip"] },
  "--view": { value: true, commands: ["clip", "frames"] },
  "--times": { value: true, commands: ["frames"] },
  "--port": { value: true, commands: ["dev"] },
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

export function parseArgs(argv: readonly string[]): Command {
  const [name, ...rest] = argv;
  if (name === undefined) throw new UsageError("no command given");
  if (!["render", "clip", "frames", "dev"].includes(name)) throw new UsageError(`unknown command "${name}"`);

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
  for (const [flag, values] of flags) if (flag !== "--param" && values.length > 1) throw new UsageError(`flag ${flag} given twice`);

  const one = (flag: string): string | undefined => flags.get(flag)?.[0];
  const fixture = flags.has("--fixture");
  if (positional.length > 1) throw new UsageError(`unexpected argument "${positional[1]}"`);
  if (fixture && positional.length === 1) throw new UsageError("give either a scene file or --fixture, not both");
  if (!fixture && positional.length === 0) throw new UsageError(`${name}: needs a scene file (or --fixture)`);
  const source: Source = fixture ? { kind: "fixture" } : { kind: "file", path: positional[0]! };

  if (name === "dev") return { kind: "dev", source, port: one("--port") === undefined ? 5173 : positive("--port", one("--port")!) };

  const out = one("--out");
  if (out === undefined) throw new UsageError(`${name}: --out is required`);
  const wantExt = name === "clip" ? ".mp4" : ".webp";
  if (!out.endsWith(wantExt)) throw new UsageError(`--out: ${name} writes ${wantExt} files, got "${out}"`);
  const shared: Shared = {
    source,
    size: parseSize(one("--size") ?? "1280x800"),
    params: Object.fromEntries((flags.get("--param") ?? []).map(parseParam)),
  };

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
        view: one("--view") ?? "three",
      };
    case "frames": {
      const t = one("--times");
      if (t === undefined) throw new UsageError("frames: --times is required, e.g. --times 0,0.5,1");
      return { ...shared, kind: "frames", out, times: t.split(",").map((s) => num("--times", s)), view: one("--view") ?? "three" };
    }
    default:
      throw new UsageError(`unhandled command "${name}"`);
  }
}
