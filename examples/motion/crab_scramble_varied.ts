// The crab's scramble with swings shaped by need (panel c of agent_notes/2026-11-19_motion.md, "Stepping from weight"):
// the stepper of crab_scramble_steps.ts, but each swing's duration and lift are set at lift-off from its length and
// its urgency (lib/stepper.ts, `Swing` varied): urgent steps short and low, relaxed ones longer and higher.
import { plane, scene, toon } from "../../src/index.ts";
import { CAMERAS, crab } from "./lib/crab.ts";
import { body } from "./lib/scramble.ts";
import { readout, stepper } from "./lib/stepper.ts";

export const feet = stepper("feetVaried", { kind: "varied" });
export default scene({ ground: plane, things: crab(body, (k) => feet.at(k)[0]!.xyz), style: toon({ bands: 3, outline: 0.04 }), sun: [-0.45, 0.75, 0.5], cameras: CAMERAS });
export const checks = readout(feet);
