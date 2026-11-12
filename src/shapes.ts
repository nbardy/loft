// Shapes layer: everything that builds a shape or a place (strands, crests, fans, limbs, spirals, hulls, seas, soft media).
// Geometry only; how it moves is in motion.ts.
export { strand, Strand, type StrandSpec, type TipSpec, type PaintSpec, type Rgb } from "./geom/strand.ts";
export { crest, type CrestSpec } from "./geom/crest.ts";
export { sea, Sea, type SeaSpec, type SwellSpec } from "./geom/sea.ts";
export { type SeaDeform } from "./geom/sea.ts";
export { limb, polyline, unit, type Limb, type LimbSpec } from "./geom/limb.ts";
export { fan, rayOf, catenary, type FanSpec } from "./geom/fan.ts";
export { shell, spiralTube, spiralLine, whorl, ridgePhase, type ShellSpec, type SpiralTubeSpec } from "./geom/spiral.ts";
export { mirror, array, reflect } from "./geom/symmetry.ts";
export { whirlpool, Whirlpool, PoolChart, type WhirlpoolSpec } from "./geom/whirlpool.ts";
export { afloat, afloatFrame, type AfloatSpec, type Waters } from "./geom/afloat.ts";
export { rigid } from "./geom/rig.ts";
export { hull, Hull, type HullSpec, type HullLook, type Lines, type Station } from "./geom/hull.ts";
export { ship, Ship, type ShipSpec, type ShipColours, type Placement } from "./geom/ship.ts";
export { lines, type Rope, type LinesSpec } from "./geom/lines.ts";
export * from "./soft/index.ts";
