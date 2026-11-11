// Who places a thing, and a thing's transform as a game gives it (docs/throw_and_shove.md, "The contract").
//
// A scene that shows an attack on a boat needs the boat somewhere when the monster does not hold it. Either Loft's
// stand-in places it (a closed form: afloat, a ballistic flight, a drift), so the scene plays on its own; or the host
// does: the game passes the boat's location and orientation every frame and Loft only draws it there. That choice is a
// sum type, `Placer`, classified once where the scene is assembled, and `placedBy` is its one dispatcher: every use
// gives one handler per variant and never asks the kind.

import { add, cos, cross, lift, mix, mul, normalize, sin, sub, vec3, type Expr, type In } from "../expr.ts";
import type { Frame } from "../map/types.ts";

type F = Expr<"f32">;
type P3 = Expr<"vec3">;

/**
 * A thing's transform as a game states it: where its origin is (world), its heading (radians from +x toward +z), its
 * pitch (radians, bow up) and its heel (radians, to starboard). Each is an expression, so a game drives them through
 * host inputs.
 */
export type Pose = { readonly at: P3; readonly heading: F; readonly pitch: F; readonly heel: F };

/** A pose from its four parts; none is defaulted (a level thing says pitch 0 and heel 0). */
export function pose(o: { readonly at: In<"vec3">; readonly heading: In<"f32">; readonly pitch: In<"f32">; readonly heel: In<"f32"> }): Pose {
  return { at: lift<"vec3">(o.at), heading: lift<"f32">(o.heading), pitch: lift<"f32">(o.pitch), heel: lift<"f32">(o.heel) };
}

/**
 * The frame a pose gives: x toward the bow, y up through the mast, z to starboard (= x cross y), origin at `at`. The
 * heading turns about world up, the pitch then lifts the bow, the heel then leans the mast to starboard: the same axes
 * `afloat` gives (src/geom/afloat.ts), so a thing drawn afloat can be drawn by a pose instead.
 */
export function poseFrame(p: Pose): Frame {
  const fwd = vec3(cos(p.heading), 0, sin(p.heading)), side = vec3(mul(sin(p.heading), -1), 0, cos(p.heading)), up = vec3(0, 1, 0);
  const x = add(mul(fwd, cos(p.pitch)), mul(up, sin(p.pitch))), y0 = sub(mul(up, cos(p.pitch)), mul(fwd, sin(p.pitch)));
  return { o: p.at, x, y: add(mul(y0, cos(p.heel)), mul(side, sin(p.heel))), z: sub(mul(side, cos(p.heel)), mul(y0, sin(p.heel))) };
}

/** Loft's stand-in places the thing: a closed form of the clock, so the scene plays with no game. */
export type StandIn = { readonly kind: "standIn" };
/** The host places the thing: it gives the pose every frame and Loft draws the thing there. */
export type Hosted = { readonly kind: "host"; readonly pose: Pose };
export type Placer = StandIn | Hosted;

export const STAND_IN: StandIn = { kind: "standIn" };
export const hosted = (p: Pose): Hosted => ({ kind: "host", pose: p });

/** The one dispatcher on who places a thing: a handler per variant. */
export function placedBy<T>(p: Placer, by: { readonly standIn: () => T; readonly host: (pose: Pose) => T }): T {
  switch (p.kind) {
    case "standIn": return by.standIn();
    case "host": return by.host(p.pose);
  }
}

/**
 * Frame a at w = 0, frame b at w = 1, between them for w between: the origin mixed, the axes mixed and made orthonormal
 * again (x kept, y made perpendicular to it). For handing a thing from one owner's frame to another's over a moment
 * (a ship taken off the water into a grip); the two frames should be within a right angle of each other.
 */
export function blendFrames(a: Frame, b: Frame, w: In<"f32">): Frame {
  const x = normalize(mix(a.x, b.x, w)), z = normalize(cross(x, mix(a.y, b.y, w)));
  return { o: mix(a.o, b.o, w), x, y: cross(z, x), z };
}
