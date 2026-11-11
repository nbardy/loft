// The crab's slots: what an option of each receives and must return (docs/variants.md, "Adding an option").
//
// Every option is `about` (one line for the sheet), optional `numbers` (overrides it needs on the whole crab: stilt
// legs raise the body), and a `draw` from the rig. The claws also *offer* the rig two measurements before the rig is
// built (the Grab places the ship by them), because the attacks' geometry depends on the claws' shape.

import type { Drawable, Measured, Strand, Strike } from "../../../src/index.ts";
import type { Option } from "../../../src/design.ts";
import type { Numbers } from "./numbers.ts";
import type { Rig } from "./rig.ts";

type Opt = Option<Numbers>;

/** The body shell. `hull` is the strand the face's beard rides in its own (m, v): m from tail (0) to front (1), v round
 * the section with sin(2 pi v) its height (v = 0.25 on top; v = 0 is a flank). */
export type Carapace = { readonly hull: Strand; readonly things: readonly Drawable[] };
export type CarapaceOption = Opt & { readonly draw: (rig: Rig) => Carapace };

/** Eight legs from their roots on the body to the walk's feet (`rig.walk.foot(k)`). */
export type LegsOption = Opt & { readonly draw: (rig: Rig) => readonly Drawable[] };

/**
 * A two-fingered gripper as the Grab needs to know it, in plain numbers (DERIVATION in claws/base.ts):
 * - closedDepth(w): how far beyond the wrist, along the palm, the grip's middle sits when the fingers close on a thing w
 *   wide (the ship is placed at wrist reach plus this);
 * - restGap: the gap between the fingers' surfaces at the claw's own rest (below zero when they cross).
 */
export type Gripper = { readonly closedDepth: (width: number) => number; readonly restGap: number };
/** What the claws tell the rig before it exists: how far short of the disc's centre the crusher's wrist lands
 * (half its palm, so the palm covers the centre), and the cutter as a gripper. */
export type ClawsOffer = { readonly crushLead: number; readonly cutter: Gripper };
/** The claws drawn: the crusher bound to Claw Crush (`rig.crush`, part "crusherPalm"), and the cutter's two fingers
 * that the grip check measures. */
export type Claws = { readonly things: readonly Drawable[]; readonly crush: Strike; readonly cutterFingers: readonly [Measured, Measured] };
export type ClawsOption = Opt & { readonly offer: (n: Numbers) => ClawsOffer; readonly draw: (rig: Rig, offer: ClawsOffer) => Claws };

/** The face on the front of the carapace (crab coordinates: the base face's brow V at x = 4.93 for the base carapace; a
 * face may place itself from `n.carapace` instead, as face/mask.ts does). */
export type FaceOption = Opt & { readonly draw: (rig: Rig, carapace: Carapace) => readonly Drawable[] };

/** What rides the shell frame (`rig.shell`): the base is the atoll, its reef and the ruined fort. */
export type BackOption = Opt & { readonly draw: (rig: Rig) => readonly Drawable[] };
