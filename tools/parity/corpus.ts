// The corpus (tools/parity/corpus.json): every examples/**/*.ts and the creative seeds, each seed migrated by
// tools/migrate.ts into a scene file under `seedDir`. Read by the picture-parity gate (tools/parity/run.ts) and the
// port's byte-diff harness (tools/port/diff.ts).
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "../..");

export type Corpus = {
  examples: string;
  seeds: { default: string; env: string; commit: string; sets: { set: string; path: string; rows: number; sha256: string }[] };
  frames: { cameras: number; times: number[]; size: [number, number]; stepsTime: number; detail: { time: number; size: [number, number] } };
};
export type Origin = { kind: "example" } | { kind: "seed"; set: string; seed: string; unported: number };
export type Entry = { id: string; path: string; origin: Origin };

export const corpus = JSON.parse(readFileSync(join(import.meta.dir, "corpus.json"), "utf8")) as Corpus;

/** A corpus the run cannot read as pinned: its message names the fix. */
export class CorpusError extends Error {}

const sha = (b: Uint8Array | string) => createHash("sha256").update(b).digest("hex");

/** Every entry whose id contains `only`, examples first (sorted), then each seed set in corpus.json's order. */
export async function corpusEntries(only: string, seedDir: string): Promise<Entry[]> {
  const examples = [...new Bun.Glob(corpus.examples).scanSync({ cwd: ROOT })].sort()
    .map((path): Entry => ({ id: path.replace(/\.ts$/, "").replaceAll("/", "__"), path, origin: { kind: "example" } }));
  const { migrate } = await import("../migrate.ts");
  const creative = (process.env[corpus.seeds.env] ?? corpus.seeds.default).replace(/^~/, homedir());
  const seeds = corpus.seeds.sets.flatMap(({ set, path, rows, sha256 }) => {
    const file = join(creative, path);
    if (!existsSync(file)) throw new CorpusError(`${file} is missing. Fix: set ${corpus.seeds.env} to a creative checkout at ${corpus.seeds.commit} or later`);
    const text = readFileSync(file, "utf8");
    if (sha(text) !== sha256) throw new CorpusError(`${file} has sha256 ${sha(text)}, corpus.json pins ${sha256}. Fix: check out creative ${corpus.seeds.commit}, or re-pin corpus.json and say so in the gate report`);
    const lines = text.split("\n").filter((l) => l.trim() !== "");
    if (lines.length !== rows) throw new CorpusError(`${file}: ${lines.length} rows, corpus.json says ${rows}`);
    return lines.map((line): Entry => {
      const { id, code } = JSON.parse(line) as { id: string; code: string };
      const m = migrate(code, "scene.ts", null);
      const safe = id.replaceAll("/", "__").replace(/[^A-Za-z0-9_.-]/g, "_");
      const ts = `${seedDir}/${set}/${safe}.ts`;
      mkdirSync(join(ROOT, `${seedDir}/${set}`), { recursive: true });
      writeFileSync(join(ROOT, ts), m.code);
      return { id: `seed__${set}__${safe}`, path: ts, origin: { kind: "seed", set, seed: id, unported: m.unported.length } };
    });
  });
  return [...examples, ...seeds].filter((e) => e.id.includes(only));
}
