import seams from "./wgsl/seams.wgsl" with { type: "text" };
import support from "./wgsl/support.wgsl" with { type: "text" };
import pin from "./wgsl/pin.wgsl" with { type: "text" };
import step from "./wgsl/roam.wgsl" with { type: "text" };
import present from "./wgsl/present.wgsl" with { type: "text" };

/** Each file is a law with a value-only WGSL interface. Replacing the whole module also works. */
export function roam(laws: Partial<{ arch: string; step: string; present: string }> = {}) {
  const swing = laws.arch === undefined ? pin : laws.arch + pin.slice(pin.indexOf("\n"));
  return [seams,support,swing,laws.step ?? step,laws.present ?? present].join("\n");
}
