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
import { lighthouse } from "./titan/back/lighthouse.ts";
import { spires } from "./titan/back/spires.ts";
import { base as carapaceBase } from "./titan/carapace/base.ts";
import { pebble } from "./titan/carapace/pebble.ts";
import { plated } from "./titan/carapace/plated.ts";
import { base as clawsBase } from "./titan/claws/base.ts";
import { fiddler } from "./titan/claws/fiddler.ts";
import { base as faceBase } from "./titan/face/base.ts";
import { mask } from "./titan/face/mask.ts";
import { periscope } from "./titan/face/periscope.ts";
import { base as legsBase } from "./titan/legs/base.ts";
import { slim } from "./titan/legs/slim.ts";
import { stilt } from "./titan/legs/stilt.ts";
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
  // For comparing variants (`loft variants`): a far step back (the whole silhouette, the ship and the crush disc), the
  // face head on, the legs from the right side at water level, the back from above and behind.
  far: { kind: "perspective", eye: [33, 21, 21], target: [3, 1.5, -1], fovY: 0.6 },
  // `close`'s eye with a longer lens: from lower or nearer, the resting claws cover the mouth.
  face: { kind: "perspective", eye: [15.5, 9, 3], target: [4.5, 2.8, 0], fovY: 0.46 },
  legs: { kind: "perspective", eye: [2, 0.9, 26], target: [1.5, 1.4, 0], fovY: 0.6 },
  back: { kind: "perspective", eye: [-15, 19, 9], target: [-1.5, 2.5, 0], fovY: 0.6 },
};

export const design = makeDesign({
  name: "reefback_titan",
  numbers: S as Numbers,
  slots: {
    carapace: { base: carapaceBase, plated, pebble } satisfies Record<string, CarapaceOption>,
    legs: { base: legsBase, stilt, slim } satisfies Record<string, LegsOption>,
    claws: { base: clawsBase, fiddler } satisfies Record<string, ClawsOption>,
    face: { base: faceBase, mask, periscope } satisfies Record<string, FaceOption>,
    back: { base: backBase, spires, lighthouse } satisfies Record<string, BackOption>,
  },
  variants: {
    stilt: { about: "the base crab up on stilt legs", pick: { legs: "stilt" } },
    // A colossal fiddler (docs/art_direction/crab_round2/fiddler_*.webp): one great claw as long as the body, a slender
    // little cutter, periscope eyes, a dark pebble shell, a ruined lighthouse. Its inks: the body darkens to oxblood and
    // near-black; the great claw is the one accent, vermilion-brick to cream. Its claws ask for a crush disc of 4.4 (the
    // base's is 3.4): a bigger hand lands on a bigger area (claws/fiddler.ts), a gameplay change the owner decides.
    fiddler: {
      about: "a colossal fiddler: one great claw, periscope eyes, a dark pebble, a ruined lighthouse",
      pick: { carapace: "pebble", legs: "slim", claws: "fiddler", face: "periscope", back: "lighthouse" },
      numbers: { ink: { brick: [0.34, 0.14, 0.14], brickDark: [0.16, 0.09, 0.11], accent: [0.78, 0.22, 0.12] } },
    },
    // The same crab if the crush disc must stay the base's 3.4: the largest great claw that lands inside it. Measured: 1.85
    // is 1.5% outside, 1.9 is at the 2% limit (theory.ts predicted 3.4 holds about g = 1.9 with the contact squash).
    fiddlerBaseDisc: {
      about: "the fiddler with the base crush disc (3.4): its great claw at the largest that fits",
      pick: { carapace: "pebble", legs: "slim", claws: "fiddler", face: "periscope", back: "lighthouse" },
      numbers: { ink: { brick: [0.34, 0.14, 0.14], brickDark: [0.16, 0.09, 0.11], accent: [0.78, 0.22, 0.12] }, crusher: 1.85, crushRadius: 3.4 },
    },
    ironclad: { about: "plated shell, stone spires, a masked stare", pick: { carapace: "plated", back: "spires", face: "mask" } },
    // Stilt legs put the cutter's shoulder 2.5 above the water; the fiddler's 0.8 cutter spans 4.46, short of a wrist
    // 4.2 out and 2.25 down (4.76): the pair alone fails the grip check (middle 0.19 off). Closing 3.6 out (4.25) it grips
    // as the fiddler does on base legs (0.058 off). Neither option is wrong; the variant settles their meeting.
    stiltFiddler: { about: "a fiddler on stilts, taking the ship nearer", pick: { legs: "stilt", claws: "fiddler" }, numbers: { grabWristReach: 3.6 } },
  },
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
