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
import { shrine } from "./titan/back/shrine.ts";
import { spires } from "./titan/back/spires.ts";
import { base as carapaceBase } from "./titan/carapace/base.ts";
import { plated } from "./titan/carapace/plated.ts";
import { thorny } from "./titan/carapace/thorny.ts";
import { base as clawsBase } from "./titan/claws/base.ts";
import { fiddler } from "./titan/claws/fiddler.ts";
import { tongs } from "./titan/claws/tongs.ts";
import { base as faceBase } from "./titan/face/base.ts";
import { mask } from "./titan/face/mask.ts";
import { patient } from "./titan/face/patient.ts";
import { base as legsBase } from "./titan/legs/base.ts";
import { stilt } from "./titan/legs/stilt.ts";
import { S, type Numbers } from "./titan/numbers.ts";
import { rig } from "./titan/rig.ts";
import type { BackOption, CarapaceOption, ClawsOption, FaceOption, LegsOption } from "./titan/slots.ts";

// The cameras, from the build's numbers. The game's cameras (game, three, close, top, grab) are fixed in the world. The
// comparison columns follow the crab where a variant moves it (docs/variants.md, "Limits"): `face` follows the head
// mount (the carapace's front, and the ride height) all the way; `legs`, `back` and `far` rise by a share of the ride,
// so a tall crab stays in frame. For the base every shift is exactly 0 and the cameras are the numbers they always were.
const camerasFor = (n: Numbers): Readonly<Record<string, Camera>> => {
  const rise = n.ride - S.ride, front = n.carapace.to - S.carapace.to;
  const at = (c: { eye: readonly [number, number, number]; target: readonly [number, number, number]; fovY: number }, dx: number, dy: number): Camera =>
    ({ kind: "perspective", eye: [c.eye[0] + dx, c.eye[1] + dy, c.eye[2]], target: [c.target[0] + dx, c.target[1] + dy, c.target[2]], fovY: c.fovY });
  return {
    game: { kind: "perspective", eye: [23, 27, 21], target: [3, 0.5, 2.5], fovY: 0.6 },
    three: { kind: "perspective", eye: [27, 12, 9], target: [1.5, 2, 5.5], fovY: 0.6 },
    // The face, from the front and above (the claws, folded at rest, cover the mouth from lower down), the fort behind.
    close: { kind: "perspective", eye: [15.5, 9, 3], target: [4.2, 1.4, 0.2], fovY: 0.55 },
    top: { kind: "orthographic", eye: [1, 50, 0.001], target: [1, 0, 1], height: 34 },
    // The Grab from low and to the side: the claw closing across the beam, the lift and the squeeze.
    grab: { kind: "perspective", eye: [21, 5.5, -3], target: [8.5, 2.2, -6.5], fovY: 0.6 },
    // For comparing variants (`loft variants`): a far step back (the whole silhouette, the ship and the crush disc), the
    // face head on, the legs from the right side at water level, the back from above and behind.
    far: at({ eye: [33, 21, 21], target: [3, 1.5, -1], fovY: 0.6 }, 0, rise * 0.35),
    // `close`'s eye with a longer lens: from lower or nearer, the resting claws cover the mouth.
    face: at({ eye: [15.5, 9, 3], target: [4.5, 2.8, 0], fovY: 0.46 }, front, rise),
    legs: at({ eye: [2, 0.9, 26], target: [1.5, 1.4, 0], fovY: 0.6 }, 0, rise * 0.85),
    back: at({ eye: [-15, 19, 9], target: [-1.5, 2.5, 0], fovY: 0.6 }, 0, rise * 0.5),
  };
};

export const design = makeDesign({
  name: "reefback_titan",
  numbers: S as Numbers,
  slots: {
    carapace: { base: carapaceBase, plated, thorny } satisfies Record<string, CarapaceOption>,
    legs: { base: legsBase, stilt } satisfies Record<string, LegsOption>,
    claws: { base: clawsBase, fiddler, tongs } satisfies Record<string, ClawsOption>,
    face: { base: faceBase, mask, patient } satisfies Record<string, FaceOption>,
    back: { base: backBase, spires, shrine } satisfies Record<string, BackOption>,
  },
  variants: {
    // The spider crab (docs/art_direction/crab_round2/stilt_*.webp): a small thorny body high on bamboo stilts, tongs, a
    // patient sad face, a red shrine on its back. The ship is taken further round to the front (-0.55, not -0.75): at
    // -0.75 it lay 2.8 from the front-left foot's foam.
    stilt: {
      about: "a colossal spider crab: thorny body on bamboo stilts, tongs, a shrine",
      pick: { carapace: "thorny", legs: "stilt", claws: "tongs", face: "patient", back: "shrine" }, numbers: { grabBearing: -0.55 },
    },
    fiddler: { about: "one huge crusher, one small cutter", pick: { claws: "fiddler" } },
    ironclad: { about: "plated shell, stone spires, a masked stare", pick: { carapace: "plated", back: "spires", face: "mask" } },
    // stiltFiddler (fiddler claws on the stilt legs) was removed 2026-11-11, when the stilt legs became the spider crab's
    // (body 6.8 up). The fiddler's arms (2.5 + 2.4) cannot reach the water from there (grip 2.6 off, palm 3.7% outside).
    // Arms long enough for its 0.8 cutter (5.15 + 4.95: grip 0.058 off, measured) make its 2.4 crusher span 16.5 to reach
    // 6.8; folded that far its 7-long palm lies along a forearm aimed away from the disc (38.5% outside, measured). One
    // armLengths scales both claws by size; this pair needs an arm length per claw, a field the claws' numbers lack.
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
        cameras: camerasFor(n),
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
