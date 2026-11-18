// Additional whole-scene reference. Time the production host from its first module
// creation to completion of the parallel pipeline batch, on a fresh device/cache.
import { compile } from "../../src/compile/index.ts";
import { lower } from "../../src/host/lower.ts";
import { loadBundle } from "../../src/host/host.ts";
import type { Scene } from "../../src/scene.ts";

export async function sceneSetup(device: GPUDevice, scene: Scene, token: number) {
  const a = compile(scene), b = lower(a);
  let start = 0, unsalted = 0;
  const variants = new Map<string, string>();
  for (const mod of b.manifest.modules) {
    const src = mod.chunks.map(k => b.chunks.get(b.manifest.chunks[k])!).join("");
    // A scene-independent blit has no frame and can remain cached in a new scene.
    if (!src.includes("var<uniform> frame:")) { unsalted++; variants.set(src, src); continue; }
    const code = src.replace(/@(vertex|fragment|compute)\b[^{}]*?\bfn\s+\w+\s*\([^{}]*?\)\s*(?:->\s*[^{}]+?)?\s*\{/g,
      (head, stage: string) => {
        let stop: string;
        switch (stage) {
          case "compute": stop = "return;"; break;
          case "fragment": stop = "discard;"; break;
          case "vertex": {
            const type = head.match(/->\s*(\w+)\s*\{$/)?.[1];
            if (!type) throw new Error(`unknown vertex output: ${head}`);
            stop = `return ${type}();`; break;
          }
          default: throw new Error(`unknown shader stage ${stage}`);
        }
        return `${head}\n if (frame.time.y == ${token}.0) { ${stop} }\n`;
      });
    if (code === src) throw new Error(`no entry point in ${mod.label}`);
    variants.set(src, code);
  }
  // Assemble variants before timing. loadBundle still creates real layouts and requests
  // the whole batch with normal descriptors. No persistent cache is cleared or inspected.
  const proxy = new Proxy(device, { get(target, key) {
    if (key === "createShaderModule") return (desc: GPUShaderModuleDescriptor) => {
      if (!start) start = performance.now();
      const code = variants.get(desc.code);
      if (code === undefined) throw new Error("unknown module source");
      return target.createShaderModule({ ...desc, code });
    };
    const v = Reflect.get(target, key, target);
    return typeof v === "function" ? v.bind(target) : v;
  } });
  const loaded = await loadBundle(proxy, b, { kind: "js", uploads: state => a.cpuPose(state) });
  const ms = performance.now() - start;
  loaded.destroy();
  return { ms, modules: b.manifest.modules.length, unsalted, render: b.manifest.renderPipelines.length, compute: b.manifest.computePipelines.length };
}
