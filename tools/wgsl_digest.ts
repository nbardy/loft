// Shader-text digests of scenes in every built-in style, on the CPU: the byte-identity gate for a change that must not
// alter any existing scene's WGSL. One line per scene and style: the digest of `sceneWgsl` and every module's text.
//   bun tools/wgsl_digest.ts [scene.ts ...]     default: a spread of examples
import { createHash } from "node:crypto";
import { resolve } from "node:path";
import { compile } from "../src/compile/index.ts";
import type { Scene } from "../src/scene.ts";
import { LIGHT, STYLE_NAMES, styleNamed, withLight, withStyle } from "../src/style/index.ts";

const DEFAULT = [
  "examples/first/lantern.ts", "examples/serpent.ts", "examples/storm.ts", "examples/stormwing.ts", "examples/coral.ts",
  "examples/shell_relief.ts", "examples/maelstrom.ts", "examples/scenes/night_hunt.ts", "examples/scenes/stormfront.ts",
  "examples/scenes/island_tide.ts", "examples/second/lighthouse.ts", "examples/bosses/tithe_fleet.ts", "examples/pearls.ts",
];

const files = process.argv.slice(2).length > 0 ? process.argv.slice(2) : DEFAULT;
// Params register globally per process, so several scenes run as one process each, three at a time.
if (files.length > 1) {
  const out: string[] = new Array(files.length).fill("");
  let next = 0;
  const worker = async () => {
    for (let i = next++; i < files.length; i = next++) {
      const p = Bun.spawn(["bun", import.meta.path, files[i]!], { stdout: "pipe", stderr: "pipe" });
      out[i] = (await new Response(p.stdout).text()) + (await p.exited === 0 ? "" : `${files[i]} FAILED ${(await new Response(p.stderr).text()).split("\n").find((l) => l.includes("rror")) ?? ""}\n`);
    }
  };
  await Promise.all([worker(), worker(), worker()]);
  process.stdout.write(out.join(""));
  process.exit(0);
}
for (const f of files) {
  const mod = (await import(resolve(f))) as { default: Scene };
  const s = mod.default;
  // Every style, then toon and woodblock under every lighting recipe (litBodies).
  const recipes = Object.keys(LIGHT).flatMap((l) => [`toon/${l}`, `woodblock/${l}`]);
  for (const style of ["scene", ...STYLE_NAMES, ...recipes]) {
    const [name, light] = style.split("/") as [string, string | undefined];
    let a;
    try {
      const styled = name === "scene" ? s : withStyle(s, styleNamed(name));
      a = compile(light === undefined ? styled : withLight(styled, light));
    }
    catch (e) { console.log(`${f} ${style} refused: ${(e as Error).message.slice(0, 80)}`); continue; }
    const text = [a.sceneWgsl, ...Object.keys(a.shaders).sort().flatMap((k) => [k, a.shaders[k]!])].join("\n");
    // The second digest renames expression temporaries (e12, h3, aL4s0, ...) by first appearance: it ignores a change
    // that only renumbers the global interning counter, so it tells a semantic change from a shifted numbering.
    const seen = new Map<string, string>();
    const norm = text.replace(/\b([aeh]L?)(\d+)/g, (_, p: string, n: string) => {
      const k = p + n;
      if (!seen.has(k)) seen.set(k, `${p}#${seen.size}`);
      return seen.get(k)!;
    });
    const d = (x: string) => createHash("sha256").update(x).digest("hex").slice(0, 16);
    console.log(`${f} ${style} ${d(text)} ${d(norm)}`);
  }
}
