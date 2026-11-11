// The jitter: the fiddler's face (docs/art_direction/DIRECTION.md, "Round 4: the fiddler's and the spider crab's
// faces"). Temperament: manic, twitchy, bug-eyed; a show-off who cannot keep still.
//
// What it is, and what every feature is for:
// - two bone-white bug eyes with big black pupils and a glint, on very tall thin vermilion stalks ringed black (the
//   great claw's ink: the two things it shows off). No iris, almost no lid at rest: a lid is a thin vermilion line on
//   top of the ball until fury or a flinch brings it down;
// - the stalks act. Each is a bent spine (src/geom/bend.ts): it leans at the root, bows, and crooks at the tip, by the
//   expression, so the pose of the stalks is the expression before any lid moves: craned and crossed toward the ship
//   (wanting), ramrod straight and shaking (straining), shot up and apart (triumph), short and spread in a V (fury), one
//   thrown back in an S while the other ducks (the wince);
// - the eyes swivel separately. Each has its own restless clock (src/geom/restless.ts, `dart`): at uneven intervals
//   it jumps to something else (you, its own great claw, the other boat, the ship, nothing), overshoots, and its stalk
//   flicks after it; it blinks on some jumps, one eye at a time. The more an expression fixes it on one thing (`roam`
//   toward 0), the less it wanders;
// - nothing rests: the stalks tremble in bursts, harder the more the expression shakes; they whip after the slam and
//   pop at the throw;
// - a tiny two-plate mouth high on the pebble's front: two cream teeth that part, chatter, splay at the top (a grin)
//   or at the bottom (a sneer).
//
// The face is a score (src/geom/expression.ts), as the islander's is. Where it looks is part of each expression: the
// ship is `rig.shipNow`, the slam's place is the rig's `crush.centre` input, the other boat is `rig.boatTwoAt`.
//
// Inherited: the eyeball, sliding pupil and lune lids (the islander's, by way of lib/stalk_eyes.ts); the audience-facing
// socket; a score over the fight's clocks. Mine: stalks whose bending is the pose, eyes on separate restless clocks,
// a flinch that is not the same on both sides, the teeth.

import {
  abs, add, added, alive, cast, clamp, div, dot, expressionsRead, floor, fract, lt, max, minScreenSize, mix, mul, pick, score, select,
  show, sin, smoothstep, strand, sub, swing, time, tolerance, unit, vec3, during,
  type Expression, type F, type P3, type Posed, type Score,
} from "../../../../src/index.ts";
import { bent } from "../../../../src/geom/bend.ts";
import { chosen, dart, tremor } from "../../../../src/geom/restless.ts";
import { lookOf, stalkEye } from "../../lib/stalk_eyes.ts";
import { cut, perFrame, TAU, type V3 } from "../ink.ts";
import type { Rig } from "../rig.ts";
import type { FaceOption } from "../slots.ts";

/** Where the audience is: the game camera's eye (reefback_titan.ts, the `game` camera). */
export const AUDIENCE = [23, 27, 21] as const;
/** Where the sockets face when nothing turns them: the audience's bearing, 10 degrees lower (face/dandy.ts found that
 * squarely at a camera 40 degrees up the eyes showed the lower cameras their undersides). */
export const FACING = [23, 19, 16] as const;
/** How far the socket turns toward what it looks at (a share of the way), and how far the pupil slides when the look
 * is square to the socket (a share of the radius). SLIDE plus the largest pupil stays under 1. */
export const TURN = 0.2, SLIDE = 0.32;

// ---- the channels, and the cast ----

/**
 * What the face is drawn from. Angles are radians; a bend's side is toward the great claw (the crab's right).
 * - upper, lower: how far each lid has come over the eye: 0 out of the way, 1 at its middle line; shut when they sum to 2.
 * - slant: the lid line's tip, positive down toward the middle (a scowl).
 * - pupil: the pupil's radius as a share of the eye's. swell: the eyeball's size, times its own.
 * - flinch: how far the face has flinched from a blow on its great claw's side. It is not the same on both sides
 *   (`EyeSpec.flinch`): the near eye shuts and its stalk is thrown back; the far one ducks and peeks.
 * - tall: the stalks' length, times their own. lean: both tip forward at the root; aside: both toward the great claw;
 *   spread: apart.
 * - bow: both bend near the root toward the great claw's side (negative: toward the ship's). crane: the tips curl
 *   forward. cross: the tips curl toward each other.
 * - jaw: the teeth, 0 shut to 1 parted. sneer: their bottoms splay (1), or their tops (-1: a grin).
 * - ship, disc: how far its look goes from the audience to the ship, or to the slam's place. roam: how much each eye
 *   wanders off on its own restless clock.
 * - shake: how hard it trembles.
 */
export type Channel =
  | "upper" | "lower" | "slant" | "pupil" | "swell" | "flinch" | "tall" | "lean" | "aside" | "spread" | "bow" | "crane" | "cross"
  | "jaw" | "sneer" | "ship" | "disc" | "roam" | "shake";

export const CAST = cast<Channel, "want" | "strain" | "triumph" | "fury" | "wince">(
  // At rest it cannot rest: bug eyes wide open, wandering separately, the stalks a little apart and twitching.
  { upper: 0.1, lower: 0, slant: 0, pupil: 0.42, swell: 1, flinch: 0, tall: 1, lean: 0.06, aside: 0, spread: 0.1, bow: 0, crane: 0.12, cross: 0, jaw: 0.1, sneer: 0, ship: 0, disc: 0, roam: 1, shake: 0.4 },
  {
    // It wants the ship: both stalks bowed hard over toward it and craned, the tips crowding together, pupils swollen.
    want: { upper: 0, pupil: 0.46, swell: 1.12, tall: 1.12, lean: 0.2, aside: -0.25, spread: -0.05, bow: -0.55, crane: 0.5, cross: 0.1, jaw: 0.6, ship: 1, roam: 0.2, shake: 0.5 },
    // The squeeze: eyes screwed shut, stalks ramrod straight, short and thick, shaking; teeth clenched and bared.
    strain: { upper: 1, lower: 1, slant: 0.5, pupil: 0.3, swell: 0.95, tall: 0.76, lean: 0, spread: 0, crane: 0, jaw: 0, sneer: 0.4, ship: 1, roam: 0, shake: 1 },
    // Let go: the stalks shoot up and apart, the eyes pop, the teeth part in a grin; it looks half after the ship and
    // half at you (looking all the way after it put the pupils on the far edge of the eye: tools/faces_fs_theory.ts).
    triumph: { upper: 0, pupil: 0.3, swell: 1.3, tall: 1.28, lean: -0.2, spread: 0.3, crane: -0.15, jaw: 1, sneer: -1, ship: 0.5, roam: 0.25, shake: 0.2 },
    // Before the slam: stalks short and spread in a V, thrust at the disc; lids hard down to the middle over pinpoints.
    fury: { upper: 0.85, lower: 0.2, slant: 0.75, pupil: 0.18, tall: 0.6, lean: 0.5, aside: 0.25, spread: 0.5, crane: 0.3, jaw: 0.3, sneer: 1, disc: 1, roam: 0, shake: 0.6 },
    // The blow: the near eye shut on a stalk thrown back, the far one ducked and half-lidded, peeking at the damage.
    wince: { flinch: 1, upper: 0.55, lower: 0.35, slant: -0.35, pupil: 0.3, swell: 0.92, tall: 0.85, lean: -0.1, spread: 0.1, crane: 0, jaw: 0, sneer: 0.6, disc: 1, roam: 0, shake: 0.4 },
  },
);
export type Mood = keyof typeof CAST.named;

// ---- the two eyes ----

export type EyeSpec = {
  readonly tag: "L" | "R"; readonly side: 1 | -1;
  /** The stalk's root on the carapace (crab coordinates), its length, its radius at root and tip; the eye's radius. */
  readonly root: V3; readonly length: number; readonly girth: readonly [number, number]; readonly radius: number;
  /** What `flinch` does to this eye: its lids (added to both), its stalk's lean back and to the side, its tip's curl
   * forward, and the share of its length it ducks by. */
  readonly flinch: { readonly lid: number; readonly back: number; readonly aside: number; readonly curl: number; readonly duck: number };
  /** Seconds late it reads the score; its restless clock (seconds a hold, seconds a jump, a seed); how fast it trembles
   * (Hz) and how it whips after the slam (radians, Hz). */
  readonly lag: number; readonly restless: { readonly every: number; readonly snap: number; readonly seed: number };
  readonly tremble: number; readonly whip: readonly [number, number];
};
// Not twins: the great claw's side stands taller on a bigger eye and leads; each has its own clocks.
export const EYES: readonly EyeSpec[] = [
  { tag: "R", side: 1, root: [4.35, 0.9, 0.95], length: 3.3, girth: [0.2, 0.13], radius: 0.72,
    flinch: { lid: 1, back: -0.55, aside: -0.4, curl: 0.75, duck: 0 }, lag: 0, restless: { every: 0.85, snap: 0.09, seed: 3 }, tremble: 9.3, whip: [0.5, 2.3] },
  { tag: "L", side: -1, root: [4.3, 0.9, -0.95], length: 2.95, girth: [0.19, 0.12], radius: 0.66,
    flinch: { lid: 0, back: 0.1, aside: 0.15, curl: 0.2, duck: 0.32 }, lag: 0.09, restless: { every: 1.15, snap: 0.11, seed: 8 }, tremble: 11.1, whip: [0.65, 1.7] },
];
/** How far a bend trembles at full shake, radians. At rest (shake 0.4, in bursts) the tip travels about 0.15, 5 px from the
 * game camera; straining, about 0.28. (0.055 was planned: 2 px at rest, which nobody would see.) */
const TREMBLE = 0.12;
/** A lid's least angle, radians: fully raised it is a thin line on top of the ball. */
const LID_MIN = 0.06;

type Arith<T> = { readonly add: (a: T | number, b: T | number) => T; readonly mul: (a: T | number, b: T | number) => T; readonly clamp: (x: T, lo: number, hi: number) => T };
const JS: Arith<number> = { add: (a, b) => a + b, mul: (a, b) => a * b, clamp: (x, lo, hi) => Math.min(Math.max(x, lo), hi) };
const GPU: Arith<F> = { add: (a, b) => add(a, b), mul: (a, b) => mul(a, b), clamp: (x, lo, hi) => clamp(x, lo, hi) };

/** One eye's drawn quantities from the channels: the lids' angles, the slant, the stalk's length, its lean and its
 * crook (each: toward the great claw, and forward) and its bow (toward the great claw). One formula for the picture and for the theory (tools/faces_fs_theory.ts). */
function eyeFrom<T>(o: Arith<T>, c: Readonly<Record<Channel, T>>, e: EyeSpec) {
  const fl = e.flinch;
  return {
    upper: o.clamp(o.mul(o.add(c.upper, o.mul(c.flinch, fl.lid)), Math.PI / 2), LID_MIN, Math.PI),
    lower: o.clamp(o.mul(o.add(c.lower, o.mul(c.flinch, fl.lid)), Math.PI / 2), LID_MIN, Math.PI),
    slant: o.mul(c.slant, e.side),
    length: o.mul(o.mul(c.tall, e.length), o.add(1, o.mul(c.flinch, -fl.duck))),
    lean: [o.add(o.add(c.aside, o.mul(c.spread, e.side)), o.mul(c.flinch, fl.aside)), o.add(c.lean, o.mul(c.flinch, fl.back))] as const,
    bow: c.bow,
    crook: [o.mul(c.cross, -e.side), o.add(c.crane, o.mul(c.flinch, fl.curl))] as const,
  };
}
/** The same in plain numbers: what tools/faces_fs_theory.ts predicts from. */
export const poseOf = (c: Expression<Channel>, e: EyeSpec) => eyeFrom(JS, c, e);

// ---- the score: the cast laid over the fight ----

/** Seconds after the thrown ship lands for the look to leave it (the grab's `settle` beat). */
const LOOK_AWAY = 1.0;

export function scoreOf(rig: Rig): Score<Channel, Mood> {
  const { n, crush, grab } = rig;
  return score(CAST, (t) => {
    const cb = crush.beats(t, n.crushSettle), ctau = crush.envelope(t).tau;
    const gb = grab.beats(t, LOOK_AWAY), gtau = grab.envelope(t).tau;
    // Everything arrives fast: it has no patience. From the Grab's warning the ship has it, until the ship is down.
    const attending = mul(during(add(gtau, grab.warn), { from: 0, rise: 0.18 }), sub(1, smoothstep(0.2, 0.8, gb.settle)));
    // Each squeeze, 0 to 1 at its tightest: the strain eases to three quarters between them.
    const squeeze = clamp(div(sub(1, grab.crush(t)), n.grabSqueeze), 0, 1);
    const straining = mul(mul(smoothstep(0, 0.03, gb.hold), sub(1, smoothstep(0, 0.12, gb.release))), mix(0.75, 1, squeeze));
    const triumphant = mul(smoothstep(0.02, 0.14, gb.release), sub(1, smoothstep(0.3, 0.9, gb.settle)));
    const furious = mul(smoothstep(0, 0.15, cb.anticipate), sub(1, smoothstep(-0.05, 0.01, ctau)));
    const wincing = during(ctau, { from: -0.05, rise: 0.06, to: 0.45, fall: 0.3 });
    // It cackles: the stalks bob and the teeth chatter, six times a second; and they chatter when it strains.
    const laugh = sin(mul(t, TAU * 6));
    // The pop: at the release the stalks overshoot upward and ring; the eyes with them.
    const sinceThrow = sub(gtau, n.grabSeconds.hold);
    return [
      show("want", attending), show("strain", straining), show("triumph", triumphant), show("fury", furious), show("wince", wincing),
      added({ tall: mul(laugh, 0.04), jaw: mul(sub(laugh, 1), 0.22) }, triumphant),
      added({ jaw: mul(max(tremor(t, 8, 3), 0), 0.3) }, straining),
      added({ tall: swing(sinceThrow, 0.22, 2.6, 0.2), swell: swing(sinceThrow, 0.12, 3.1, 0.3) }),
    ];
  });
}

// ---- the face ----

/** The teeth's place on the pebble's front (crab coordinates): the hinge line's middle, and the tips' at rest. */
const MOUTH = { hinge: [4.74, 0.8, -0.1] as V3, tip: [4.99, 0.18, -0.1] as V3 };

export const jitter: FaceOption = {
  about: "manic and bug-eyed: two white eyes on very tall thin vermilion stalks that bend, cross and twitch, each wandering on its own; two teeth",
  // The face camera stands 1.1 higher for it: the stalks are 1.3 times the islander's and left that frame's top.
  numbers: { faceRise: 1.1 },
  draw: (rig) => {
    const { n, body, local, shipNow, crushAfter } = rig;
    const I = n.ink;
    const theScore = scoreOf(rig);
    const tabF = (name: string, e: F): F => perFrame(`jitter_${name}`, e);
    const tabV = (name: string, e: P3): P3 => perFrame(`jitter_${name}`, e);
    const audience = vec3(...AUDIENCE);
    const disc = vec3(rig.inputs.crushCentre.x, 0, rig.inputs.crushCentre.y);
    const otherBoat = vec3(rig.boatTwoAt.x, 0.6, rig.boatTwoAt.y);
    // What an eye may wander off to: you (most often), its own great claw at rest, the other boat, the ship, nothing.
    const claw = local([5.6, 0.4, 4.2]), nothing = local([3, 4, -16]);
    const haunts: readonly (readonly P3[])[] = [
      [audience, claw, audience, otherBoat, shipNow, audience],
      [audience, shipNow, nothing, audience, otherBoat, audience],
    ];

    const eye = (e: EyeSpec, i: number) => {
      const c: Posed<Channel> = theScore.at(sub(time, e.lag)), q = eyeFrom(GPU, c, e);
      const base = local(e.root);
      // Its restless clock: which hold it is in and the last one, and how far the jump has gone (past 1, then back).
      const d = dart(time, { every: e.restless.every, snap: e.restless.snap, period: n.crushPeriod, seed: e.restless.seed });
      const place = (hold: F): P3 => pick(floor(mul(chosen(hold, e.restless.seed), haunts[i]!.length)), haunts[i]!);
      const wander = mix(place(d.before), place(d.now), d.arrive);
      const roam = tabF(`${e.tag}_roam`, c.roam);
      const target = tabV(`${e.tag}_target`, mix(mix(mix(audience, shipNow, c.ship), disc, c.disc), wander, roam));
      // The stalk leans after its look (from the root: the tip's own place depends on this), so every jump flicks it.
      const toward = unit(sub(target, base));
      // It trembles in bursts, each stalk at its own rate; whips after the slam, the tip against the root.
      const shake = tabF(`${e.tag}_shake`, mul(c.shake, mix(0.3, 1, clamp(mul(tremor(time, 0.7, e.restless.seed), 1.6), 0, 1))));
      const tr = (k: number): F => mul(mul(tremor(time, e.tremble, k + 10 * i), TREMBLE), shake);
      const whip = swing(crushAfter, e.whip[0], e.whip[1], 0.22);
      const idle = (k: number): F => alive(time, 0.04, 31 + k + 10 * i, 0.31, n.crushPeriod);
      const stalk = bent({
        root: base, along: body.y, a: body.z, b: body.x,
        length: tabF(`${e.tag}_length`, q.length),
        lean: [tabF(`${e.tag}_leanA`, add(q.lean[0], idle(0))), tabF(`${e.tag}_leanB`, add(q.lean[1], idle(1)))],
        bow: [tabF(`${e.tag}_bowA`, add(q.bow, tr(0))), tabF(`${e.tag}_bowB`, add(tr(1), mul(whip, -0.5)))],
        crook: [
          tabF(`${e.tag}_crookA`, add(add(q.crook[0], mul(dot(toward, body.z), 0.3)), tr(2))),
          tabF(`${e.tag}_crookB`, add(add(add(q.crook[1], mul(dot(toward, body.x), 0.3)), tr(3)), whip)),
        ],
      });
      // It blinks on four jumps in ten, as the jump starts: one eye at a time, since each has its own clock.
      const lookAt = (at: P3) => (middle: P3) => lookOf({ from: middle, facing: vec3(...FACING), target: at, up: body.y, turn: TURN, slide: SLIDE });
      const blink = mul(mul(select(lt(chosen(d.now, e.restless.seed + 3), 0.4), 1, 0), sub(1, smoothstep(0.05, 0.14, d.since))), roam);
      const shut = Math.PI / 2 + 0.08;
      return stalkEye({
        tag: e.tag, prefix: "jitter",
        stalk: {
          bent: stalk, girth: e.girth,
          // Vermilion, a black ring at every seventh, a dark collar where it leaves the shell.
          paint: (m) => mix(mix(vec3(...I.accent), vec3(...I.black), sub(1, cut(abs(sub(fract(mul(m, 7)), 0.5)), 0.08))), vec3(...I.brickDark), sub(1, cut(m, 0.07))),
        },
        radius: e.radius, swell: c.swell, hang: 0, up: body.y,
        look: lookAt(target),
        pupil: c.pupil,
        ink: { white: I.cream, pupil: I.black, back: I.brickDark, glint: 0.34, ring: 0.06 },
        lids: {
          upper: mix(q.upper, shut, blink), lower: mix(q.lower, shut, blink), slant: q.slant,
          // Vermilion with a heavy black edge: a shut eye is a red ball with a lash line, bright on the dark shell.
          ink: (edge) => mix(vec3(...I.accent), vec3(...I.black), sub(1, cut(edge, 0.2))),
        },
        // The marks are the expression's own pose: no wandering, no tremble, no whip, no blink.
        posed: {
          bent: bent({ root: base, along: body.y, a: body.z, b: body.x, length: q.length, lean: q.lean, bow: [q.bow, 0], crook: q.crook }),
          look: lookAt(mix(mix(audience, shipNow, c.ship), disc, c.disc)), upper: q.upper, lower: q.lower,
        },
      });
    };
    const [right, left] = EYES.map(eye) as [ReturnType<typeof eye>, ReturnType<typeof eye>];

    // The teeth read the score on the right eye's clock.
    const face = theScore.at(time);
    const jaw = tabF("jaw", clamp(add(face.jaw, mul(max(sin(mul(time, 2.7)), 0), 0.1)), 0, 1.2));
    const sneer = tabF("sneer", face.sneer);
    const tooth = (s: 1 | -1) => {
      const grin = max(mul(sneer, -1), 0), frown = max(sneer, 0);
      const hinge = local(vec3(MOUTH.hinge[0], MOUTH.hinge[1], add(MOUTH.hinge[2], mul(add(0.23, mul(grin, 0.22)), s))));
      const tip = local(vec3(add(MOUTH.tip[0], mul(jaw, 0.28)), sub(MOUTH.tip[1], mul(jaw, 0.22)), add(MOUTH.tip[2], mul(add(0.23, mul(frown, 0.3)), s))));
      return strand({
        name: `tooth${s < 0 ? "L" : "R"}`, spine: (m) => mix(hinge, tip, m), stations: 4,
        girth: (m) => mul(0.21, sub(1, mul(m, 0.2))), section: { aspect: 0.4, squareness: 3 }, up: body.x,
        tips: [{ round: 0.12 }, { round: 0.3 }], paint: () => vec3(...I.cream), detail: { along: 8, around: 12 },
      });
    };

    return {
      things: [...right.things, ...left.things, tooth(1), tooth(-1)],
      checks: [
        // Each eye is big enough to read from the game camera (the periscope's measured 34 px).
        minScreenSize(right.ball, "game", 48), minScreenSize(left.ball, "game", 40),
        // Every named expression shows in a cycle, and any two differ on screen by a quarter of the right eye's width.
        expressionsRead(theScore, { ...right.marks, ...left.marks }, { camera: "game", period: n.crushPeriod, apart: tolerance.share(0.25), of: right.width }),
      ],
    };
  },
};
