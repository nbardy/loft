// The checks a scene with baked state gets without asking (docs/proposals/state.md 4.5 and 6): the seam of a loop,
// measured, and the drift certificate of a checkpointed state. Both read the baked states the frame draws (an
// unbaked artefact is refused by the CPU readings: `loft check` bakes on the GPU first).

import { replayClocks, replayStep, type Loop, type Recurrence } from "./artefact.ts";
import type { Probe, Verdict } from "./check.ts";

/** A relative tolerance on f32 values, from its one constructor: never tighter than 1e-4 (f32 rounding fails correct runs). */
type Relative = { readonly kind: "relative"; readonly of: number };
function relative(of: number): Relative {
  if (!(of >= 1e-4)) throw new Error(`a relative tolerance on f32 values must be at least 1e-4, got ${of}`);
  return { kind: "relative", of };
}
/** A loop's seam, relative to each lane's range over the loop (state.md 4.5). */
const SEAM = relative(1e-3);
/** A CPU replay of K steps against the next checkpoint the GPU kept, relative to each lane's range there. */
const DRIFT = relative(1e-3);

/** Floats per state: the entries, then the clock entry, each `lanes` vec4s. */
const floats = (r: Recurrence) => (r.entries + 1) * r.lanes * 4;
const kept = (r: Recurrence, k: number) => r.data.subarray(k * floats(r), (k + 1) * floats(r));

/** The state at time t on the CPU, read as the frame reads it: linear between steps n and n + 1. */
function stateAt(probe: Probe, r: Recurrence, t: number): Float32Array {
  const out = new Float32Array(floats(r));
  const mixInto = (a: Float32Array, b: Float32Array, f: number) => { for (let i = 0; i < out.length; i++) out[i] = a[i]! + (b[i]! - a[i]!) * f; };
  switch (r.kept.kind) {
    case "every": {
      if (r.steps === 0) return kept(r, 0).slice();
      const s = Math.min(Math.max((t - r.start) * r.rate, 0), r.steps), n = Math.min(Math.floor(s), r.steps - 1);
      mixInto(kept(r, n), kept(r, n + 1), s - n);
      return out;
    }
    case "checkpoints": {
      const clock = replayClocks([r])[0]!;
      const { n, k } = replayStep(clock, t);
      const frame = probe.compiled.frame(probe.state(t));
      const [even, odd] = ["even", "odd"].map((h) => frame.table(probe.compiled.table(`${r.name}_${h}`)));
      const [now, next] = k % 2 === 0 ? [even!, odd!] : [odd!, even!];
      mixInto(now, next, Math.min(Math.max((t - r.start) * r.rate - n, 0), 1));
      return out;
    }
  }
}

/** Per lane component (lane l, x..w), min and max over the entries of each state given. */
function ranges(r: Recurrence, states: readonly Float32Array[]): { lo: Float64Array; hi: Float64Array } {
  const C = r.lanes * 4, lo = new Float64Array(C).fill(Infinity), hi = new Float64Array(C).fill(-Infinity);
  for (const s of states) for (let e = 0; e < r.entries; e++) for (let c = 0; c < C; c++) { const v = s[e * C + c]!; lo[c] = Math.min(lo[c]!, v); hi[c] = Math.max(hi[c]!, v); }
  return { lo, hi };
}

/** The worst over lane components of max |a - b| over the entries, relative to that component's range (0 / 0 is 0). */
function worst(r: Recurrence, a: Float32Array, b: Float32Array, range: { lo: Float64Array; hi: Float64Array }): { rel: number; at: string } {
  const C = r.lanes * 4;
  let rel = 0, at = "none";
  for (let c = 0; c < C; c++) {
    let d = 0;
    for (let e = 0; e < r.entries; e++) d = Math.max(d, Math.abs(a[e * C + c]! - b[e * C + c]!));
    const span = range.hi[c]! - range.lo[c]!;
    const x = d === 0 ? 0 : span > 0 ? d / span : Infinity;
    if (x > rel) { rel = x; at = `lane ${Math.floor(c / 4)}.${"xyzw"[c % 4]}`; }
  }
  return { rel, at };
}

/** The seam of each recurrence over a loop, and the drift certificate of each that keeps checkpoints. */
export function stateChecks(probe: Probe, loop: Loop): Verdict[] {
  const out: Verdict[] = [];
  for (const r of probe.artefact.recurrences ?? []) {
    if (loop.kind === "every") {
      const P = loop.seconds;
      const a = stateAt(probe, r, 0), b = stateAt(probe, r, P);
      // The range over the loop: every kept state within it (each checkpoint within it, when checkpointed), and the ends.
      const within: Float32Array[] = [a, b];
      const K = r.kept.kind === "every" ? 1 : r.kept.every;
      for (let k = 0; k < r.states; k++) { const at = r.start + (k * K) / r.rate; if (at >= 0 && at <= P) within.push(kept(r, k)); }
      const w = worst(r, a, b, ranges(r, within));
      out.push({ name: `${r.name}: the loop's seam, |S(${P}) - S(0)| over the range`, pass: w.rel <= SEAM.of, value: w.rel, measured: `${w.rel.toExponential(2)} (${w.at})`, limit: `<= ${SEAM.of}` });
    }
    if (r.kept.kind === "checkpoints") {
      const K = r.kept.every, last = r.states - 2;
      if (last < 0) continue;
      const clock = replayClocks([r])[0]!;
      let rel = 0, at = "none";
      const sampled = [...new Set([0, Math.floor(last / 2), last])];
      try {
        for (const j of sampled) {
          // At n = (j + 1) K - 1 the CPU replays checkpoint j for K steps: its state n + 1 is checkpoint j + 1's step.
          const n = (j + 1) * K - 1;
          if (n > clock.steps - 1) continue;
          const t = r.start + (n + 0.5) / r.rate;
          const frame = probe.compiled.frame(probe.state(t));
          const [even, odd] = ["even", "odd"].map((h) => frame.table(probe.compiled.table(`${r.name}_${h}`)));
          const next = replayStep(clock, t).k % 2 === 0 ? odd! : even!;
          const w = worst(r, next, kept(r, j + 1), ranges(r, [kept(r, j + 1)]));
          if (w.rel >= rel) { rel = w.rel; at = `checkpoint ${j + 1}, ${w.at}`; }
        }
      } catch (e) {
        // A step calling a GPU-only function has no CPU twin: not certified, which is not a failure.
        const why = e instanceof Error ? e.message : String(e);
        if (!/GPU-only/.test(why)) throw e;
        out.push({ name: `${r.name}: drift of the CPU twin over ${K} steps`, pass: true, value: NaN, measured: "not certified (a GPU-only function in the step)", limit: `<= ${DRIFT.of}` });
        continue;
      }
      out.push({ name: `${r.name}: drift of the CPU twin over ${K} steps (checkpoints ${sampled.map((j) => j + 1).join(", ")})`, pass: rel <= DRIFT.of, value: rel, measured: `${rel.toExponential(2)} (${at})`, limit: `<= ${DRIFT.of}` });
    }
  }
  return out;
}
