// The Reefback Titan (game design notes 2026-11-01, section 6.2): a hermit crab the size of an island, wearing a dead
// atoll for its shell with a ruined fort on the crown. Woodblock print (docs/art_direction/DIRECTION.md): a brick-red
// crab, a moss-green atoll with stone cliffs, a cream ruin; indigo sea, cream foam; one small blue-sailed sloop for
// scale.
//
// This file is the crab's design (docs/variants.md): its slots and their options, its named variants, its cameras, and
// how a choice is assembled into a scene with its checks. The parts are under titan/:
// - numbers.ts: every number an art director changes (`S`);
// - rig.ts: the walk in place, Claw Crush (a space-time `attack` on a disc ahead-right), the Grab (a `grip` attack whose
//   effect is a `hold`: docs/attacks_that_hold.md), the body and shell frames, the ship, the warnings, the crash;
// - slots.ts: what an option of each slot receives and returns;
// - carapace/, legs/, claws/, face/, back/: one file per option. `base` is the crab as first drawn.
//
// What the game reads: `strikes` (Claw Crush on the crusher's palm) and `holds.grab` (`held()`, `thing()`,
// `squeezed()`), exported below for the base variant and returned by every variant's build.

import { BASE_ASK, design as makeDesign } from "../../src/design.ts";
import type { Camera } from "../../src/artefact.ts";
import { gripReaches, landsInside, print, scene, tolerance, warned, woodblock } from "../../src/index.ts";
import { base as backBase } from "./titan/back/base.ts";
import { base as carapaceBase } from "./titan/carapace/base.ts";
import { base as clawsBase } from "./titan/claws/base.ts";
import { base as faceBase } from "./titan/face/base.ts";
import { base as legsBase } from "./titan/legs/base.ts";
import { S, type Numbers } from "./titan/numbers.ts";
import { rig } from "./titan/rig.ts";
import type { BackOption, CarapaceOption, ClawsOption, FaceOption, LegsOption } from "./titan/slots.ts";

const CAMERAS: Readonly<Record<string, Camera>> = {
  game: { kind: "perspective", eye: [23, 27, 21], target: [3, 0.5, 2.5], fovY: 0.6 },
  three: { kind: "perspective", eye: [27, 12, 9], target: [1.5, 2, 5.5], fovY: 0.6 },
  // The face, from the front and above (the claws, folded at rest, cover the mouth from lower down), the fort behind.
  close: { kind: "perspective", eye: [15.5, 9, 3], target: [4.2, 1.4, 0.2], fovY: 0.55 },
  top: { kind: "orthographic", eye: [1, 50, 0.001], target: [1, 0, 1], height: 34 },
  // The Grab from low and to the side: the claw closing across the beam, the lift and the squeeze.
  grab: { kind: "perspective", eye: [21, 5.5, -3], target: [8.5, 2.2, -6.5], fovY: 0.6 },
};

export const design = makeDesign({
  name: "reefback_titan",
  numbers: S as Numbers,
  slots: {
    carapace: { base: carapaceBase } satisfies Record<string, CarapaceOption>,
    legs: { base: legsBase } satisfies Record<string, LegsOption>,
    claws: { base: clawsBase } satisfies Record<string, ClawsOption>,
    face: { base: faceBase } satisfies Record<string, FaceOption>,
    back: { base: backBase } satisfies Record<string, BackOption>,
  },
  variants: {},
  compare: ["far", "face", "legs", "back"],
  build: (o, n) => {
    // Numbers, then what the claws offer the rig, then the rig, then each slot drawn on it, then the scene.
    const offer = o.claws.offer(n);
    const r = rig(n, offer);
    const carapace = o.carapace.draw(r);
    const legs = o.legs.draw(r);
    const claws = o.claws.draw(r, offer);
    const face = o.face.draw(r, carapace);
    const back = o.back.draw(r);
    return {
      scene: scene({
        sea: r.world.sea,
        things: [...carapace.things, ...back, ...face, ...legs, ...claws.things, ...r.world.warnings, r.world.crash, ...r.world.boat],
        style: woodblock({ print: print.overprint({ ink: n.ink.indigo, amount: n.shadowIndigo, value: n.shadowValue, black: 0.1 }) }),
        sun: [0.15, 0.8, 0.6],
        cameras: CAMERAS,
      }),
      strikes: { crush: claws.crush },
      holds: { grab: r.grab },
      checks: [
        landsInside(claws.crush), warned(r.crush), warned(r.grab),
        // The fingers are curled horns a fifth of the beam thick near their tips: a tenth of the beam either way is a close grip.
        gripReaches(r.grab, claws.cutterFingers, { middle: tolerance.share(0.1), gap: tolerance.share(0.1) }),
      ],
    };
  },
});

const base = design.build(design.resolve(BASE_ASK));
export default base.scene;
export const strikes = base.strikes;
/** What the game reads of the Grab: `held()`, `thing()`, `squeezed()`, functions of the clock. */
export const holds = base.holds;
export const checks = base.checks;
