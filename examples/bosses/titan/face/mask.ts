// A mask: the base face's parts (face/base.ts, `faceKit`) composed into a symmetric, wide-awake stare. Two tall black
// stalks, round black pupils, lids raised (still shut at the slam), an arched brow with no scowl, no beard.

import { add, length, lift, mix, unit, vec2, vec3 } from "../../../../src/index.ts";
import { cut, type V3 } from "../ink.ts";
import type { FaceOption } from "../slots.ts";
import { faceKit, type Eye } from "./base.ts";

const ARCH: readonly V3[] = [[3.3, 1.0, -3.0], [4.4, 1.25, -1.6], [4.95, 0.95, 0], [4.4, 1.25, 1.6], [3.3, 1.0, 3.0]];

export const mask: FaceOption = {
  about: "a symmetric stare: tall black stalks, round pupils, an arched brow, no beard",
  draw: (rig) => {
    const { n, local } = rig;
    const k = faceKit(rig);
    const eye = (tag: "L" | "R", s: 1 | -1): Eye => {
      const base = local([4.2, 0.95, s * 1.1]);
      return {
        tag, base, tip: add(add(base, k.inBody(0.35, 2.7, s * 0.8)), k.stalkWhip(0.4, 2.1)), look: unit(k.inBody(1, 0.2, s * 0.3)), radius: 0.55,
        lid: mix(0.4, 2.6, k.brace), slant: lift<"f32">(0),
        iris: (x, y) => mix(vec3(...n.ink.black), vec3(...n.ink.cream), cut(length(vec2(x, y)), 0.45)),
        stalkPaint: () => vec3(...n.ink.black), lidPaint: k.lidInk,
      };
    };
    return [k.brow(ARCH), ...k.eyeParts(eye("R", 1)), ...k.eyeParts(eye("L", -1)), k.plate(1), k.plate(-1), k.gullet()];
  },
};
