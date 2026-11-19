// The heron's hop over the rail, keyed by hand (panel a of agent_notes/2026-11-19_motion.md): the same bird, deck,
// rail, camera and timing as heron_hop_solve.ts (planted to 0.6 s, airborne 0.6 to 1.2, on the deck at 1.2, still at
// 1.8), but each curve is a few keys an animator would set, joined by `keys`' monotone cubic (no overshoot between
// keys). Nothing checks the flight is a parabola or that the feet clear the rail: the keys were placed to.
import { div, keys, pose, poseFrame, time, vec3, type P3 } from "../../src/index.ts";
import { heron, HOMES } from "../second/lib/heron.ts";
import { NUMBERS, stage } from "../second/heron_leap.ts";

const END = 1.8, { land, top } = NUMBERS.deck;
/** A keyed curve of time over the hop: [seconds, value] pairs. */
const keyed = (k: readonly (readonly [number, number])[]) => keys(k.map(([t, v]) => [t / END, v] as const))(div(time, END)).x;

// Anticipation, push, arc, contact, absorb, settle: the classic jump keys at the leap's beats.
const x = keyed([[0, 0], [0.3, -0.12], [0.6, 0.55], [0.9, 1.85], [1.2, land - 0.1], [1.45, land - 0.02], [END, land]]);
const y = keyed([[0, 1.9], [0.35, 1.3], [0.6, 2.0], [0.9, 2.6], [1.2, top + 1.85], [1.4, top + 1.45], [END, top + 1.9]]);
const pitch = keyed([[0, 0], [0.35, -0.22], [0.6, 0.18], [0.9, 0], [1.2, -0.12], [1.4, -0.2], [END, 0]]);
// Feet: planted, then tucked up behind the body over the rail, then down on the deck.
const fx = keyed([[0, 0], [0.6, 0], [0.9, 1.6], [1.2, land], [END, land]]);
const fy = keyed([[0, 0], [0.6, 0], [0.9, 1.55], [1.2, top], [END, top]]);

const body = poseFrame(pose({ at: vec3(x, y, 0), heading: 0, pitch, heel: 0 }));
const foot = (i: number): P3 => vec3(fx, fy, HOMES[i]![2]);
export default stage(NUMBERS, heron(body, foot, { x: vec3(1, 0, 0), z: vec3(0, 0, 1) }).things);
