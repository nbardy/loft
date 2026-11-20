// Sections of a port-diff entry, each with the module that owns its bytes (tools/port/diff.ts).
//
// Two layers, compared in pipeline order so the first differing section is the most upstream one:
//   compile.*   the compiler's Artefact (src/artefact.ts) before lowering: the boundary between the compile lanes and the
//               lower lanes. Compared as exact JSON values (typed arrays as their bytes, base64), never by key order.
//   files.*     the artefact directory, the contract: every manifest.json key's value, every chunk and blob by its index
//               in the manifest, then manifest.json's bytes whole and the set of files. Compared byte for byte.
// The Rust side writes compile.json in the same shape (rust/crates/loft-lower `dump`).
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

/** One section: its name, the module whose output it is, and its bytes (a canonical text for compile sections). */
export type Section = { readonly name: string; readonly owner: string; readonly bytes: Uint8Array };

// ---- compile.json -------------------------------------------------------------------------------------------------

/**
 * The Artefact as compile.json holds it: functions and the bake record dropped, typed arrays as their type and bytes
 * (base64), fields in the producer's order. Both sides write this; `lower` reads it.
 */
export function plain(x: unknown): unknown {
  if (typeof x === "function") return undefined;
  if (ArrayBuffer.isView(x)) return { type: x.constructor.name, base64: Buffer.from(x.buffer, x.byteOffset, x.byteLength).toString("base64") };
  if (x instanceof Map) return Object.fromEntries([...x].map(([k, v]) => [String(k), plain(v)]));
  if (Array.isArray(x)) return x.map(plain);
  if (x !== null && typeof x === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(x)) {
      if (k === "baked") continue; // src/artefact.ts `Baked`: whether a host baked it, not compiler output
      const p = plain(v);
      if (p !== undefined) out[k] = p;
    }
    return out;
  }
  if (typeof x === "number" && !Number.isFinite(x)) return String(x);
  return x;
}

/**
 * A compile.json's Artefact as named sections, in pipeline order (slots, tables and stages, scene WGSL, shaders, draws, passes, post,
 * compute, recurrences, the rest). A field the artefact lacks is absent, not null.
 */
export function dumpArtefact(p: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  const one = (k: string) => { if (p[k] !== undefined) out[`compile.${k}`] = p[k]; };
  const each = (k: string, key: (v: Record<string, unknown>, i: number) => string) => {
    const v = p[k];
    if (Array.isArray(v)) v.forEach((x, i) => { out[`compile.${k}.${key(x as Record<string, unknown>, i)}`] = x; });
    else if (v !== undefined && v !== null) for (const [n, x] of Object.entries(v)) out[`compile.${k}.${n}`] = x;
  };
  for (const k of ["inputs", "inputFloats", "params", "curves", "tables", "dynamic"]) one(k);
  each("statics", (s) => String(s.name));
  each("stages", (_, i) => String(i));
  one("sceneWgsl");
  each("shaders", () => "");
  each("draws", (_, i) => String(i));
  for (const k of ["passes", "post", "compute"]) one(k);
  each("recurrences", (r) => String(r.name));
  const mixed = p.mixed as Record<string, unknown> | undefined;
  if (mixed !== undefined) for (const [k, v] of Object.entries(mixed)) out[`compile.mixed.${k}`] = v;
  for (const k of ["notes", "cameras", "clear", "loop", "stats", "version"]) one(k);
  const known = new Set(["inputs", "inputFloats", "params", "curves", "tables", "dynamic", "statics", "stages", "sceneWgsl", "shaders", "draws", "passes", "post", "compute", "recurrences", "mixed", "notes", "cameras", "clear", "loop", "stats", "version"]);
  // A field this dump does not know is still compared, so a new Artefact field cannot escape the gate.
  for (const [k, v] of Object.entries(p)) if (!known.has(k)) out[`compile.${k}`] = v;
  return out;
}

/** A JSON value as canonical text: object keys sorted, so the comparison is of values, not of key order. */
function canonical(x: unknown): string {
  if (Array.isArray(x)) return `[${x.map(canonical).join(",")}]`;
  if (x !== null && typeof x === "object") return `{${Object.keys(x).sort().map((k) => `${JSON.stringify(k)}:${canonical((x as Record<string, unknown>)[k])}`).join(",")}}`;
  return JSON.stringify(x);
}

export function compileSections(file: string): Section[] {
  const sections = dumpArtefact(JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>);
  const enc = new TextEncoder();
  return Object.entries(sections).map(([name, v]) => ({ name, owner: ownerOf(name, v), bytes: enc.encode(canonical(v)) }));
}

// ---- the artefact directory ---------------------------------------------------------------------------------------

type Manifest = { chunks?: string[]; blobs?: { name: string }[]; modules?: { label: string; chunks: number[] }[] } & Record<string, unknown>;

/** The artefact directory's sections; a directory with no manifest.json has one, `files.manifest.json`, empty. */
export function fileSections(dir: string): Section[] {
  const enc = new TextEncoder();
  const path = join(dir, "manifest.json");
  if (!existsSync(path)) return [];
  const raw = readFileSync(path);
  let m: Manifest;
  try { m = JSON.parse(raw.toString("utf8")) as Manifest; } catch { return [{ name: "files.manifest.json", owner: "SW", bytes: raw }]; }
  const out: Section[] = [];
  for (const [k, v] of Object.entries(m)) out.push({ name: `files.manifest.${k}`, owner: manifestOwner(k), bytes: enc.encode(JSON.stringify(v)) });
  // Each chunk by its index, owned by the module of the first shader module that lists it (the prelude is lower's).
  const usedBy = new Map<number, string>();
  for (const mod of m.modules ?? []) for (const c of mod.chunks) if (!usedBy.has(c)) usedBy.set(c, mod.label);
  (m.chunks ?? []).forEach((name, i) => {
    const file = join(dir, "wgsl", `${name}.wgsl`);
    out.push({ name: `files.chunk.${i} (${usedBy.get(i) ?? "unused"})`, owner: chunkOwner(usedBy.get(i)), bytes: existsSync(file) ? readFileSync(file) : enc.encode(`(missing wgsl/${name}.wgsl)`) });
  });
  (m.blobs ?? []).forEach((b, i) => {
    const file = join(dir, "bin", `${b.name}.bin`);
    out.push({ name: `files.blob.${i}`, owner: "SW", bytes: existsSync(file) ? readFileSync(file) : enc.encode(`(missing bin/${b.name}.bin)`) });
  });
  out.push({ name: "files.manifest.json", owner: "SW", bytes: raw });
  const listed = new Set(["manifest.json", ...(m.chunks ?? []).map((c) => `wgsl/${c}.wgsl`), ...(m.blobs ?? []).map((b) => `bin/${b.name}.bin`)]);
  const present = ["manifest.json", ...["wgsl", "bin"].flatMap((d) => existsSync(join(dir, d)) ? readdirSync(join(dir, d)).map((f) => `${d}/${f}`) : [])];
  out.push({ name: "files.set", owner: "SW", bytes: enc.encode([...new Set([...present, ...listed])].sort().map((f) => `${listed.has(f) ? "" : "unlisted "}${present.includes(f) ? "" : "missing "}${f}`).join("\n")) });
  return out;
}

// ---- probes ---------------------------------------------------------------------------------------------------

/** probe/<name>.txt of one side: a lane's own text dump (tools/port/ts_side.ts writes the TS side's). */
export function probeSections(dir: string): Section[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir).filter((f) => f.endsWith(".txt")).sort().map((f) => {
    const name = `probe.${f.slice(0, -4)}`;
    return { name, owner: probeOwner(name), bytes: readFileSync(join(dir, f)) };
  });
}

// ---- owners -------------------------------------------------------------------------------------------------------

/**
 * A section's owner is the port lane whose translation writes its bytes (agent_notes/2026-11-19_port/PLAN.md, the lane
 * table). `--scope B` compares only lane B's sections. The cut is the emission tape: until the draw lanes land, a
 * drawable's shaders and draws come from the tape and lane S assembles them, so compile.shaders and compile.draws are
 * S's; a draw lane gates on its kind's tape probes (probe.tape.<k>.<kind>).
 */
export const LANES = ["P0", "LIB", "W", "G1", "G2", "B", "S", "LW", "SW", "D1", "D2", "D3", "D4", "D5"] as const;
export type Lane = (typeof LANES)[number];

/** Drawable kinds (src/compile/context.ts implementors) by the draw lane that translates them. */
const KIND_LANE: Readonly<Record<string, Lane>> = {
  surface: "D1", relief: "D1", ground: "D1", sky: "D1", sea: "D1", strand: "D1", telegraph: "D1", smear: "D1",
  field: "D2", traced: "D2",
  mesh: "D3", copies: "D3", impostors: "D3", shader: "D3",
  cast: "D4", light: "D4", shadows: "D4",
  volume: "D5", density: "D5", glow: "D5", lightning: "D5", rain: "D5", specks: "D5", wake: "D5",
};

function probeOwner(name: string): string {
  const [, head, , kind] = name.split(".");
  switch (head) {
    case "wire": return "W";
    case "graph": return "G1";
    case "eval": return "G2";
    case "tape": return kind !== undefined && kind in KIND_LANE ? KIND_LANE[kind]! : `unowned tape kind "${kind}"`;
    default: return `unowned probe "${name}"`;
  }
}

function ownerOf(name: string, _v: unknown): string {
  const head = name.replace(/^compile\./, "").split(".")[0]!;
  switch (head) {
    case "inputs": case "inputFloats": case "params": case "curves": case "tables": case "dynamic": case "statics":
    case "stages": case "compute": case "recurrences": case "mixed": return "B";
    default: return "S"; // sceneWgsl, shaders, draws, passes, post, notes, cameras, clear, loop, stats, version
  }
}

function manifestOwner(key: string): string {
  switch (key) {
    case "format": case "version": case "chunks": case "blobs": case "authored": return "SW";
    default: return "LW";
  }
}

/** A chunk's owner, by the label of the first module that uses it: a shared draw shader's is share.ts's. */
function chunkOwner(label: string | undefined): string {
  return label !== undefined && /^shader "shared /.test(label) ? "SW" : "LW";
}
