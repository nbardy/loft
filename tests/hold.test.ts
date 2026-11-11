// An attack that holds (docs/attacks_that_hold.md): `gripReaches` is the check scenes rely on to say a drawn gripper
// really takes the thing its hold owns.
import { expect, test } from "bun:test";
import { add, attack, compileScene, gripReaches, hit, hold, lift, mul, Probe, scene, sea, strand, tolerance, toon, vec2, vec3, type F } from "../src/index.ts";

const R = 0.12; // the fingers' radius

/** Two straight vertical fingers either side of the attack's grip, their axes `gap / 2 + reach` from its middle, shifted `shift` across. */
function claw(reach: number, shift: number) {
  const grab = attack({
    name: "grab", arm: 1, warn: 1.2, target: { kind: "disc", centre: vec2(0, 0), radius: lift<"f32">(0.5) },
    motion: { kind: "grip", hit: hit.disc({ centre: [0, 0], radius: 1.5 }), close: 0.2, release: 0.5 },
    effect: hold({ seconds: 1, at: [0, 0.5, 0], radius: 0.5, carry: { kind: "arc", to: [1, 0.5, 0], lift: 1 }, squeeze: 0.3, pulses: 1 }),
    timing: { windup: 0.5, strike: 0.8 },
  });
  const g = grab.grip([0, 3, 0]);
  const finger = (side: number) => strand({
    name: side < 0 ? "fingerA" : "fingerB", stations: 4, girth: R, paint: [0.8, 0.2, 0.1],
    spine: (m: F) => add(g.palm, vec3(add(mul(add(mul(g.gap, 0.5), reach), side), shift), add(mul(m, -2), 1), 0)),
  });
  const fingers = [finger(-1), finger(1)] as const;
  const s = scene({ sea: sea({ swell: [] }), things: [...fingers], style: toon({}), sun: [0.3, 0.8, 0.5], cameras: { c: { kind: "perspective", eye: [0, 2, 9], target: [0, 1, 0], fovY: 0.6 } } });
  return gripReaches(grab, fingers, { middle: tolerance.share(0.1), gap: tolerance.share(0.1) }).run(new Probe(compileScene(s), {}));
}

// Catches: a check that measures the fingers' axes (or tips) instead of their surfaces. The crab's old scene check did,
// and passed a claw whose fingers sank 0.085 into the hull on each side. Axes a width apart must fail; surfaces a width
// apart (axes a radius further out each side) must pass, through the squeeze; a grip off the thing's middle must fail.
test("gripReaches passes surfaces a width apart and fails axes a width apart or a grip off the middle", () => {
  const good = claw(R, 0), axes = claw(0, 0), off = claw(R, 0.3);
  expect(good.pass).toBe(true);
  expect(good.value).toBeLessThan(0.1);
  expect(axes.pass).toBe(false);
  expect(axes.measured).toContain("0.76 apart"); // the 1.0 width less two radii, at closing
  expect(off.pass).toBe(false);
});
