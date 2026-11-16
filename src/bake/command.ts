// `loft bake`: the command body, kept out of src/cli/main.ts so that file only dispatches.
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import type { Command } from "../cli/args.ts";
import type { Scene } from "../scene.ts";
import { summary, writeGlb } from "./gltf.ts";
import { bakeScene } from "./mesh.ts";
import { bakeAnimated, motionSummary } from "./motion.ts";

export async function bake(cmd: Extract<Command, { kind: "bake" }>): Promise<void> {
  const exported = ((await import(resolve(cmd.path))) as { default?: unknown }).default;
  if (exported === undefined) throw new Error(`${cmd.path} has no default export (it must export a scene)`);
  const scene = exported as Scene;
  const t0 = performance.now();
  const write = (glb: Uint8Array, bodies: number, triangles: number, what: string): void => {
    mkdirSync(dirname(resolve(cmd.out)), { recursive: true });
    writeFileSync(cmd.out, glb);
    console.log(`wrote ${cmd.out}  ${glb.byteLength} bytes, ${bodies} bodies, ${triangles} triangles, ${what}  (${(performance.now() - t0).toFixed(0)} ms)`);
  };
  switch (cmd.when.kind) {
    case "at": {
      const baked = bakeScene(scene, { time: cmd.when.time, density: cmd.density });
      for (const line of summary(baked)) console.log(line);
      for (const s of baked.skipped) console.log(`skipped ${s.name}: ${s.reason}`);
      write(writeGlb(baked), baked.bodies.length, baked.bodies.reduce((n, b) => n + b.indices.length / 3, 0), `t=${cmd.when.time}`);
      return;
    }
    case "animated": {
      const a = bakeAnimated(scene, { density: cmd.density, seconds: cmd.when.seconds });
      for (const line of summary(a)) console.log(line);
      for (const line of motionSummary(a)) console.log(line);
      for (const s of a.skipped) console.log(`skipped ${s.name}: ${s.reason}`);
      const count = (k: string) => a.motions.filter((m) => m.kind === k).length;
      write(writeGlb(a, a), a.bodies.length, a.bodies.reduce((n, b) => n + b.indices.length / 3, 0),
        `${a.times.length - 1} keys over ${a.span.toFixed(3)} s (${a.spanFrom}): ${count("skinned")} skinned, ${count("morphed")} morphed, ${count("still")} still, ${count("refused")} refused`);
      return;
    }
  }
}
