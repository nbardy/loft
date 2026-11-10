// A gait: foot targets for many legs from one cycle, as a pure function of time. The body travels a `course` (its
// pose after walking a distance s, any closed-form function); each foot is planted, for the `duty` share of its
// cycle, where the body's frame put its home at the middle of that stance, and swings to the next such place on an
// arc. So a planted foot is fixed in the world while the body moves over it, with no state: the plant is a function
// of which cycle it is, `floor(t / period + phase)`.

import { add, cos, floor, lift, mix, mul, sin, smoothstep, step, sub, saturate, div, vec3, time, type In } from "../expr.ts";
import { rotate } from "../map/chart.ts";
import type { F, Frame, P3 } from "../map/types.ts";

export type Course = (s: F) => Frame;

export type GaitSpec = {
  /** Each foot's home in the body's course frame (x the body's forward, y up), on the ground. */
  readonly feet: readonly In<"vec3">[];
  /** Each foot's phase offset, as a share of the cycle (one per foot). */
  readonly phase: readonly number[];
  /** The body's pose after travelling a distance s along the ground. */
  readonly course: Course;
  /** Distance the body travels in one full cycle. */
  readonly stride: In<"f32">;
  /** Seconds per cycle. */
  readonly period: In<"f32">;
  /** Share of the cycle each foot is planted (above 0.5 keeps more feet down than up). */
  readonly duty: In<"f32">;
  /** Peak height of a swinging foot above the line between its plants. */
  readonly lift: In<"f32">;
  /** The clock. Default `time`. */
  readonly t?: In<"f32">;
};

export type Gait = {
  /** Distance travelled. */
  readonly travelled: F;
  /** The body's course frame now. */
  readonly body: Frame;
  /** Foot i's target in the world. */
  foot(i: number): P3;
  /** 1 while foot i is planted, 0 while it swings. */
  planted(i: number): F;
};

/** A point given in a frame's coordinates, in the world. */
const inFrame = (g: Frame, p: In<"vec3">): P3 => add(g.o, rotate(g, lift<"vec3">(p)));

export function gait(spec: GaitSpec): Gait {
  if (spec.phase.length !== spec.feet.length) throw new Error(`gait: ${spec.feet.length} feet need as many phases, got ${spec.phase.length}`);
  const t = lift<"f32">(spec.t ?? time), period = lift<"f32">(spec.period), duty = lift<"f32">(spec.duty);
  const speed = div(spec.stride, period);
  const cycle = (i: number) => add(div(t, period), spec.phase[i]);
  return {
    travelled: mul(speed, t),
    body: spec.course(mul(speed, t)),
    planted: (i) => sub(1, step(duty, sub(cycle(i), floor(cycle(i))))),
    foot: (i) => {
      const c = cycle(i), k = floor(c), s = sub(c, k);
      // The middle of this cycle's stance, and the next one's: where the body's frame puts the home then.
      const tMid = mul(period, sub(add(k, mul(duty, 0.5)), spec.phase[i]));
      const here = spec.course(mul(speed, tMid)), next = spec.course(mul(speed, add(tMid, period)));
      const swing = saturate(div(sub(s, duty), sub(1, duty)));
      // In stance swing is 0, so this is the plant itself: no branch between stance and swing.
      const arc = mul(here.y, mul(spec.lift, sin(mul(swing, Math.PI))));
      return add(mix(inFrame(here, spec.feet[i]), inFrame(next, spec.feet[i]), smoothstep(0, 1, swing)), arc);
    },
  };
}

/** Courses: a straight line, and an arc of constant turning. Ground at y = origin.y; `facing` turns the body (radians, about up) from its direction of travel (a crab walking sideways faces a quarter turn off it). */
export const course = {
  line(origin: In<"vec3">, heading: number, facing = 0): Course {
    return (s) => frameAt(add(lift<"vec3">(origin), mul(vec3(Math.cos(heading), 0, -Math.sin(heading)), s)), lift<"f32">(heading + facing));
  },
  /** Turning left (counter-clockwise seen from above) at `radius`; a negative radius turns right. */
  arc(origin: In<"vec3">, heading: number, radius: number, facing = 0): Course {
    const dir = vec3(Math.cos(heading), 0, -Math.sin(heading)), left = vec3(-Math.sin(heading), 0, -Math.cos(heading));
    return (s) => {
      const phi = div(s, radius);
      const o = add(lift<"vec3">(origin), mul(add(mul(dir, sin(phi)), mul(left, sub(1, cos(phi)))), radius));
      return frameAt(o, add(phi, heading + facing));
    };
  },
};

/** A frame at o whose x is the horizontal direction at `yaw` (radians counter-clockwise from +x seen from above). */
function frameAt(o: P3, yaw: F): Frame {
  const x = vec3(cos(yaw), 0, mul(sin(yaw), -1));
  return { o, x, y: lift<"vec3">([0, 1, 0]), z: vec3(sin(yaw), 0, cos(yaw)) };
}
