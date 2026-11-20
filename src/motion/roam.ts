import seams from "./wgsl/seams.wgsl" with { type: "text" };
import support from "./wgsl/support.wgsl" with { type: "text" };
import pin from "./wgsl/pin.wgsl" with { type: "text" };
import deck from "./wgsl/deck.wgsl" with { type: "text" };
import ranges from "./wgsl/share_range.wgsl" with { type: "text" };
import step from "./wgsl/roam.wgsl" with { type: "text" };
import present from "./wgsl/present.wgsl" with { type: "text" };

/** Each file is a law with a value-only WGSL interface. Replacing the whole module also works. */
export function roam(laws: Partial<{ arch: string; step: string; present: string; deck: string }> = {}) {
  const swing = laws.arch === undefined ? pin : laws.arch + pin.slice(pin.indexOf("\n"));
  const placement = laws.deck === undefined ? `
    fn deck_at(t:f32)->Frame {return Frame(vec4f(0,0,0,1),vec4f(0,0,0,1));}
    fn placed(deck:Frame, joint:Frame)->Frame {return joint;}
    fn deck_site(deck:Frame, world:vec3f)->vec3f {return world;}
  ` : laws.deck + "\n" + deck;
  return [seams, support, ranges, swing, placement, laws.step ?? step, laws.present ?? present].join("\n");
}
