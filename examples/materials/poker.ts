// A poker whose tip glows by Planck's law from its own temperature, which pulses with the bellows every 3 s. The
// temperature is a channel this file invents (`kelvin`), and the style below is the only thing that reads it: in any
// other style `loft` notes that `poker` gives `kelvin`, which no part of that style reads.
import { add, div, exp, mul, plane, scene, sin, strand, sub, time, toon, vec3, TAU, type F, type P3, type ShadeCtx, type Style } from "../../src/index.ts";

/** Planck's law at 650, 530 and 450 nm: a body's light at `kelvin`, relative to green at 1300 K, compressed below 1. */
function planck(kelvin: F): P3 {
  const at = (nm: number): F => {
    const l = nm * 1e-9, g = 530e-9, c2 = 0.014388; // metres, metres, metre kelvins
    return div((g / l) ** 5 * (Math.exp(c2 / (g * 1300)) - 1), sub(exp(div(c2 / l, kelvin)), 1));
  };
  const b = vec3(at(650), at(530), at(450));
  return div(b, add(b, 1));
}

const poker = strand({
  name: "poker",
  spine: (m: F) => vec3(mul(m, 2.5), add(0.3, mul(m, 0.4)), 0),
  girth: 0.04,
  paint: (m: F) => ({
    base: [0.18, 0.18, 0.2],
    // 1250 K at the tip, falling off over the last eighth; the bellows add up to 120 K every 3 s.
    kelvin: add(300, mul(add(950, mul(sin(mul(time, TAU / 3)), 120)), exp(mul(sub(m, 1), 8)))),
  }),
});

const lit = toon();
/** Toon, plus a body's own light from its temperature: `kelvin`, 300 K (no visible light) where a paint gives none. */
const forge: Style = { ...lit, name: "forge", shade: (ctx: ShadeCtx) => add(lit.shade(ctx), planck(ctx.channel("kelvin", 300))) };

export default scene({
  ground: plane, things: [poker], style: forge, sun: [-0.4, 0.8, 0.45], period: 3,
  cameras: {
    whole: { kind: "perspective", eye: [3, 2, 4], target: [1.2, 0.6, 0], fovY: 0.6 },
    tip: { kind: "perspective", eye: [3, 1.2, 1], target: [2.4, 0.7, 0], fovY: 0.4 },
  },
});
