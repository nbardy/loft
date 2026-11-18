// Lane 1.1's binding checks (plan §8.2), the same script under Bun and Node 22:
//   bun rust/crates/loft-node/binding_check.ts ADDON
//   node --experimental-transform-types rust/crates/loft-node/binding_check.ts ADDON
// ADDON is a build with `--features probe` (it adds `panic`). Prints one JSON line; exits 1 on any failed check.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { type Limits, NativeError, open, UNLIMITED } from "../../../src/core/native.ts";
import { Op, pack, PRIM0, PRIMS, Program } from "../../../src/record/wire.ts";

const addon = process.argv[2]!;
const fails: string[] = [];
const check = (ok: boolean, what: string) => { if (!ok) fails.push(what); };

/** A chain job: lit 1.0, then n-1 nodes add(prev, lit), packed by the recorder's writer (src/record/wire.ts). */
function chain(n: number): Uint8Array {
  const ADD = PRIM0 + PRIMS.indexOf("add");
  const nodes = new Uint32Array(4 * n), args = new Uint32Array(2 * (n - 1));
  nodes.set([Op.Lit, 0, 1, 0]);
  for (let i = 1; i < n; i++) {
    nodes.set([ADD, 2 * (i - 1), 2, 0], 4 * i);
    args.set([i - 1, 0], 2 * (i - 1));
  }
  return pack({
    program: Program.Scene, nodes, args, lits: new Float64Array([1]), rows: new Float32Array(0), blobs: new Uint8Array(0),
    syms: new Uint8Array(0), symAt: new Uint32Array(1), sites: new Uint32Array(0), siteOf: new Uint32Array(n),
    decls: Uint32Array.from([1]), roots: Uint32Array.from([n - 1]),
  });
}

const refusal = (f: () => unknown): NativeError | undefined => {
  try { f(); return undefined; } catch (e) { return e instanceof NativeError ? e : undefined; }
};

const free: Limits = { nodes: UNLIMITED, tapeOps: UNLIMITED, boxes: UNLIMITED, gpuNanos: UNLIMITED };
const opened = open(addon), n = { run: (job: Uint8Array) => opened.run(job, free) };
const small = n.run(chain(3));

// Lane 4.2: a node limit one under the job's count refuses as budget, at the same count every time; at the count it runs.
const over = [0, 1].map(() => refusal(() => opened.run(chain(1000), { ...free, nodes: 999n })));
check(over.every((e) => e?.kind === "budget" && e.code === 7 && JSON.stringify(e.error) === JSON.stringify({ Budget: { meter: "Nodes", limit: 999, spent: 1000 } })), "a job one node over its limit is Budget{Nodes, 999, 1000}, twice");
check(opened.run(chain(1000), { ...free, nodes: 1000n }).nodes === 1000, "a job at its node limit runs");
check(small.nodes === 3 && small.program === "Scene", "a 3-node job decodes");

const big = chain(1_000_000);
const times: { total: number; decode: number }[] = [];
for (let k = 0; k < 15; k++) {
  const t = process.hrtime.bigint();
  const r = n.run(big);
  times.push({ total: Number(process.hrtime.bigint() - t) / 1e6, decode: r.decode_ns / 1e6 });
}
times.sort((a, b) => a.total - b.total);
const med = times[7]!;
const crossing = med.total - med.decode;
check(crossing < 5, `1M-node crossing ${crossing.toFixed(2)} ms >= 5 ms`);

const broken = chain(3);
broken[0] = 0;
const bad = refusal(() => n.run(broken));
check(bad?.kind === "internal" && bad.code === 1 && JSON.stringify(bad.error).includes("Magic"), "a malformed job is Internal(Wire Magic)");

const raw = { exports: {} as { panic?: () => string } };
process.dlopen(raw, addon);
const panicked = JSON.parse(raw.exports.panic!()) as { Err?: { kind: string; error: { Internal?: { Panic?: string } } } };
check(panicked.Err?.kind === "internal" && typeof panicked.Err.error.Internal?.Panic === "string", "a panic is Internal(Panic)");
check(n.run(chain(2)).nodes === 2, "the addon still runs after a panic");

const missing = refusal(() => open("/nonexistent/libloft_node.dylib"));
check(missing?.kind === "environment" && missing.code === 6, "a missing addon is environment");

// A stale build: the same addon with its stamp's abi number changed, re-signed so macOS still loads it.
const dir = mkdtempSync(join(tmpdir(), "loft-stale-"));
const stale = join(dir, "libloft_node_stale.dylib");
copyFileSync(addon, stale);
const bytes = readFileSync(stale), at = bytes.indexOf("loft-node abi 2 wire 1");
check(at >= 0, "the stamp is in the binary");
bytes[at + 14] = "9".charCodeAt(0);
writeFileSync(stale, bytes);
if (process.platform === "darwin") execFileSync("codesign", ["-s", "-", "-f", stale], { stdio: "ignore" });
const old = refusal(() => open(stale));
check(old?.kind === "environment" && old.message.includes("stale"), "a stale addon is environment (stale)");

const runtime = "Bun" in globalThis ? `bun ${process.versions.bun}` : `node ${process.versions.node}`;
console.log(JSON.stringify({ runtime, crossing_ms: +crossing.toFixed(3), total_ms: +med.total.toFixed(3), decode_ms: +med.decode.toFixed(3), job_mb: +(big.byteLength / 1e6).toFixed(1), fails }));
process.exit(fails.length === 0 ? 0 : 1);
