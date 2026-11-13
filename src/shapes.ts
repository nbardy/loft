// Shapes layer: everything that builds a shape or a place (strands, sheets, fans, chains, coils, seas, soft media).
// Geometry only; how it moves is in motion.ts.
export { strand, Strand, type StrandSpec, type TipSpec, type PaintSpec, type Rgb } from "./geom/strand.ts";
export { sea, Sea, type SeaSpec, type SwellSpec } from "./geom/sea.ts";
export { type SeaDeform } from "./geom/sea.ts";
export { fan, rayOf, catenary, type FanSpec } from "./geom/fan.ts";
export { spiralTube, spiralLine, whorl, ridgePhase, type SpiralTubeSpec } from "./geom/spiral.ts";
export { mirror, array, reflect } from "./geom/symmetry.ts";
export { whirlpool, Whirlpool, PoolChart, type WhirlpoolSpec } from "./geom/whirlpool.ts";
export { afloat, afloatFrame, type AfloatSpec, type Waters } from "./geom/afloat.ts";
export { rigid } from "./geom/rig.ts";
export * from "./soft/index.ts";

export { coil, coilAt, coilPlan, COIL_TOPOLOGY, type CoilLaw, type CoilSection, type CoilSpec } from "./geom/spiral.ts";
export { sheetOn, sheetOnEdges, type SheetOnSpec } from "./geom/crest.ts";
export { chain, polyline, unit, type Chain, type ChainSpec } from "./geom/limb.ts";
export { ropes, type Rope, type RopesSpec } from "./geom/lines.ts";
export { billows, type BillowSpec, type PuffPlace } from "./soft/cloud.ts";
