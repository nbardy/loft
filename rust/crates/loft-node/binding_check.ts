// Lane 1.1's binding checks (plan §8.2), the same script under Bun and Node 22:
//   bun rust/crates/loft-node/binding_check.ts ADDON
//   node --experimental-transform-types rust/crates/loft-node/binding_check.ts ADDON
// ADDON is a build with `--features probe` (it adds `panic`). Prints one JSON line; exits 1 on any failed check.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { NativeError, open } from "../../../src/core/native.ts";

const addon = process.argv[2]!;
const fails: string[] = [];
const check = (ok: boolean, what: string) => { if (!ok) fails.push(what); };

/** A chain job: lit 1.0, then n-1 nodes add(prev, lit). Wire layout: loft-ir/src/wire.rs. */
function chain(n: number): Uint8Array {
  const ADD = 16; // Prim::ALL[0]
  const nodes = new Uint32Array(4 * n), args = new Uint32Array(2 * (n - 1));
  for (let i = 1; i < n; i++) {
    nodes.set([ADD, 2 * (i - 1), 2, 0], 4 * i);
    args.set([i - 1, 0], 2 * (i - 1));
  }
  const spans = [nodes, args, new Float64Array([1]), new Uint32Array(0), new Uint8Array(0), new Uint8Array(0), new Uint32Array(0), new Uint32Array(0), new Uint32Array(n), new Uint32Array(0), new Uint32Array(0)];
  let at = 16 + 88;
  const table = spans.map((s) => { const off = at; at = (at + s.byteLength + 7) & ~7; return [off, s.byteLength]; });
  const out = new Uint8Array(at), head = new Uint32Array(out.buffer, 0, 26);
  head.set([0x5754464c, 1, 0, 11, ...table.flat()]);
  spans.forEach((s, i) => out.set(new Uint8Array(s.buffer, s.byteOffset, s.byteLength), table[i]![0]!));
  return out;
}

const refusal = (f: () => unknown): NativeError | undefined => {
  try { f(); return undefined; } catch (e) { return e instanceof NativeError ? e : undefined; }
};

const n = open(addon);
const small = n.run(chain(3));
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
const bytes = readFileSync(stale), at = bytes.indexOf("loft-node abi 1 wire 1");
check(at >= 0, "the stamp is in the binary");
bytes[at + 14] = "9".charCodeAt(0);
writeFileSync(stale, bytes);
if (process.platform === "darwin") execFileSync("codesign", ["-s", "-", "-f", stale], { stdio: "ignore" });
const old = refusal(() => open(stale));
check(old?.kind === "environment" && old.message.includes("stale"), "a stale addon is environment (stale)");

const runtime = "Bun" in globalThis ? `bun ${process.versions.bun}` : `node ${process.versions.node}`;
console.log(JSON.stringify({ runtime, crossing_ms: +crossing.toFixed(3), total_ms: +med.total.toFixed(3), decode_ms: +med.decode.toFixed(3), job_mb: +(big.byteLength / 1e6).toFixed(1), fails }));
process.exit(fails.length === 0 ? 0 : 1);
