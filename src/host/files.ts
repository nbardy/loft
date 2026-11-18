// The artefact as files (docs/artefact_format.md): a directory holding
//   manifest.json        the lowered host (src/host/manifest.ts)
//   wgsl/<chunk>.wgsl    each WGSL chunk once, named by its hash; a module is its chunks concatenated
//   bin/<blob>.bin       the curves and static tables, little-endian f32
// `writeBundle` is what `loft build` calls; `loadArtefactFiles` draws from the directory alone. Neither imports the
// compiler: tools/check_loader_closure.ts fails if this file's import closure ever reaches it.
//
// bun / node only (node:fs). The browser host does not import this file.

import { lstatSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { LoadError } from "../errors.ts";
import { loadBundle, NotDataError, type GpuLoaded } from "./host.ts";
import { FORMAT, FORMAT_VERSIONS, type Bundle, type Manifest } from "./manifest.ts";

export const MANIFEST = "manifest.json";
const chunkPath = (dir: string, name: string) => join(dir, "wgsl", `${name}.wgsl`);
const blobPath = (dir: string, name: string) => join(dir, "bin", `${name}.bin`);

/** The bytes a written artefact holds, by kind. */
export type FileSizes = { readonly manifest: number; readonly wgsl: number; readonly bin: number; readonly files: number };

/**
 * Writes the bundle into `dir`, replacing what a previous build left there (only the manifest and the `wgsl` and `bin`
 * folders; anything else in `dir` is refused rather than deleted).
 */
export function writeBundle(dir: string, b: Bundle): FileSizes {
  // A `cpu` table is a JavaScript fill: refused by name, before anything is written.
  const cpu = b.manifest.tables.filter((t) => t.fill.kind === "cpu");
  if (cpu.length > 0) {
    throw new NotDataError(`build: ${cpu.length} table(s) are filled by JavaScript, which a file cannot hold: ${cpu.map((t) => `"${t.name}" (${t.fill.kind === "cpu" ? t.fill.why : ""})`).join(", ")}`);
  }
  mkdirSync(dir, { recursive: true });
  const stray = readdirSync(dir).filter((f) => f !== MANIFEST && f !== "wgsl" && f !== "bin");
  if (stray.length > 0) throw new Error(`build: ${dir} holds ${stray.join(", ")}, which a build did not write; give an empty or earlier build directory`);
  rmSync(join(dir, "wgsl"), { recursive: true, force: true });
  rmSync(join(dir, "bin"), { recursive: true, force: true });
  mkdirSync(join(dir, "wgsl"));
  mkdirSync(join(dir, "bin"));
  let wgsl = 0, bin = 0;
  for (const [name, text] of b.chunks) { writeFileSync(chunkPath(dir, name), text); wgsl += statSync(chunkPath(dir, name)).size; }
  for (const [name, bytes] of b.blobs) { writeFileSync(blobPath(dir, name), bytes); bin += bytes.byteLength; }
  // JSON writes NaN and Infinity as null; a number that cannot be written exactly is an error, not a silent null.
  // (Finite doubles round-trip exactly through JSON.stringify and JSON.parse.)
  const json = JSON.stringify(b.manifest, (key, v: unknown) => {
    if (typeof v === "number" && !Number.isFinite(v)) throw new Error(`build: manifest field "${key}" is ${v}, which JSON cannot hold`);
    return v;
  });
  writeFileSync(join(dir, MANIFEST), json);
  return { manifest: statSync(join(dir, MANIFEST)).size, wgsl, bin, files: 1 + b.chunks.size + b.blobs.size };
}

/**
 * What a reader admits from a directory it did not write: the manifest's bytes, the number of files it names, and
 * their bytes together (manifest, WGSL and bin). `loft serve --limits` sets them; `readBundle` checks all three, and
 * the names, from `lstat` alone before it reads a byte of WGSL or bin.
 */
export type ArtefactLimits = { readonly manifestBytes: number; readonly files: number; readonly bytes: number };
/** Generous: the largest scene here (five creatures) is 22 MB of WGSL. A farm sets its own. */
export const ARTEFACT_LIMITS: ArtefactLimits = { manifestBytes: 64 << 20, files: 1 << 16, bytes: 1 << 30 };

/** A file name the manifest gives must be one plain name in its folder: no separator, no `.` or `..`, no NUL. */
function plainName(field: string, v: unknown): string {
  if (typeof v !== "string" || v === "" || v === "." || v === ".." || /[/\\\0]/.test(v) || v.length > 200) {
    throw new LoadError(`manifest ${field}: ${JSON.stringify(v)} is not a plain file name (one name inside its folder, no "/", "\\" or ".."); rebuild the directory with \`loft build\``);
  }
  return v;
}

/** The size of a regular file at `path`, by `lstat` (a link is refused, so nothing outside `dir` is followed). */
function regularSize(path: string, what: string): number {
  let st;
  try { st = lstatSync(path); } catch { throw new LoadError(`${what}: missing (the manifest names it); rebuild the directory with \`loft build\``); }
  if (!st.isFile()) throw new LoadError(`${what}: not a regular file (a link or a folder); a built directory holds only files the build wrote`);
  return st.size;
}

/**
 * Reads a written artefact back. Throws `LoadError` on another format or version, on a file the manifest names but
 * `dir` lacks, and on anything past `limits`. Everything that decides what to read (the manifest's size, its format,
 * the names and declared sizes it lists, each file's size) is checked before any WGSL or bin is read, so a hostile
 * manifest cannot make the reader open a path outside `dir` or pull in more than `limits.bytes`.
 */
export function readBundle(dir: string, limits: ArtefactLimits = ARTEFACT_LIMITS): Bundle {
  const over = (what: string, n: number, limit: number, key: keyof ArtefactLimits) =>
    new LoadError(`${dir}: ${what} is ${n}, past the limit ${limit} (--limits ${key === "manifestBytes" ? "manifest" : key}=${limit}); make the scene smaller, or raise the limit`);
  for (const folder of ["wgsl", "bin"]) {
    const st = (() => { try { return lstatSync(join(dir, folder)); } catch { return undefined; } })();
    if (st !== undefined && !st.isDirectory()) throw new LoadError(`${dir}: ${folder}/ is not a folder (a link or a file); a built directory holds only what the build wrote`);
  }
  const manifestBytes = regularSize(join(dir, MANIFEST), `${dir}/${MANIFEST}`);
  if (manifestBytes > limits.manifestBytes) throw over("the manifest's size in bytes", manifestBytes, limits.manifestBytes, "manifestBytes");
  let manifest: Manifest;
  try { manifest = JSON.parse(readFileSync(join(dir, MANIFEST), "utf8")) as Manifest; } catch (e) { throw new LoadError(`${dir}/${MANIFEST}: not JSON: ${e instanceof Error ? e.message : String(e)}`); }
  if (typeof manifest !== "object" || manifest === null || Array.isArray(manifest)) throw new LoadError(`${dir}/${MANIFEST}: not a JSON object`);
  if (manifest.format !== FORMAT) throw new LoadError(`${dir}: not a Loft artefact (format "${String(manifest.format)}")`);
  if (!(FORMAT_VERSIONS as readonly number[]).includes(manifest.version)) throw new LoadError(`${dir}: artefact format version ${manifest.version}; this loader reads versions ${FORMAT_VERSIONS.join(", ")}`);
  if (!Array.isArray(manifest.chunks) || !Array.isArray(manifest.blobs)) throw new LoadError(`${dir}/${MANIFEST}: "chunks" and "blobs" must be lists`);
  const files = 1 + manifest.chunks.length + manifest.blobs.length;
  if (files > limits.files) throw over("the number of files", files, limits.files, "files");
  // Names and sizes, all from lstat; a name given twice would be read twice and is refused.
  const seen = new Set<string>();
  const once = (path: string) => { if (seen.has(path)) throw new LoadError(`${dir}: the manifest names ${path} twice`); seen.add(path); return path; };
  let bytes = manifestBytes;
  const chunkFiles = manifest.chunks.map((v, k) => {
    const name = plainName(`chunks[${k}]`, v), path = once(chunkPath(dir, name));
    const size = regularSize(path, `${dir}/wgsl/${name}.wgsl`);
    bytes += size;
    return [name, path, size] as const;
  });
  const blobFiles = manifest.blobs.map((b, k) => {
    const name = plainName(`blobs[${k}].name`, (b as { name?: unknown } | null)?.name), path = once(blobPath(dir, name));
    const declared = (b as { bytes?: unknown }).bytes, size = regularSize(path, `${dir}/bin/${name}.bin`);
    if (typeof declared !== "number" || !Number.isSafeInteger(declared) || declared !== size) throw new LoadError(`${dir}: bin/${name}.bin has ${size} bytes; the manifest says ${JSON.stringify(declared)}`);
    bytes += size;
    return [name, path, size] as const;
  });
  if (bytes > limits.bytes) throw over("the artefact's size in bytes", bytes, limits.bytes, "bytes");
  // A file that grew between its lstat and its read is caught here, not trusted.
  const read = (path: string, size: number): Buffer => {
    const data = readFileSync(path);
    if (data.byteLength !== size) throw new LoadError(`${path}: changed while it was read (${size} bytes, then ${data.byteLength})`);
    return data;
  };
  const chunks = new Map(chunkFiles.map(([name, path, size]) => [name, read(path, size).toString("utf8")]));
  const blobs = new Map(blobFiles.map(([name, path, size]) => [name, new Uint8Array(read(path, size))]));
  return { manifest, chunks, blobs };
}

/** Draws from a built directory alone: no scene module, no compiler, no `cpu` tables (a bundle with one is refused by name). */
export async function loadArtefactFiles(device: GPUDevice, dir: string): Promise<{ readonly bundle: Bundle; readonly loaded: GpuLoaded }> {
  const bundle = readBundle(dir);
  return { bundle, loaded: await loadBundle(device, bundle, { kind: "none" }) };
}
