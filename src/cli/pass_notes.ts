// What `loft` says about a scene's passes (src/frame/pass.ts): each pass, its size, what it keeps, how many draws it
// holds, and which draws were left out and why (the rule about readers, missing depth entries). One line per pass.
import type { Artefact } from "../artefact.ts";

export function passNotes(a: Pick<Artefact, "passes">): string[] {
  return (a.passes ?? []).map((p) => {
    const size = p.size.kind === "fixed" ? `${p.size.width}x${p.size.height}` : `view x ${p.size.scale}`;
    const reads = p.reads.length > 0 ? `, after ${p.reads.join(", ")}` : "";
    switch (p.what.kind) {
      case "screen": return `pass     ${p.name}  ${size} screen${reads}`;
      case "scene": {
        const why = new Map<string, string[]>();
        for (const l of p.what.leftOut) why.set(l.why, [...(why.get(l.why) ?? []), l.label]);
        const out = [...why].map(([w, labels]) => `${labels.length} ${w} (${labels.slice(0, 4).join(", ")}${labels.length > 4 ? ", ..." : ""})`);
        return `pass     ${p.name}  ${size} ${p.what.keep}: ${p.what.draws.length} draws${reads}${out.length > 0 ? `; left out ${out.join("; ")}` : ""}`;
      }
    }
  });
}
