// Ships at the game's scale (a ship is about 5 units long), from the specs of examples/brig.ts and examples/fleet.ts.
// Those are drawn at 7 to 17 units; `scaled` multiplies every length of a spec (hull lines, masts, bowsprit, flutter)
// and keeps the dimensionless ones (exponents, fractions, angles), so the same ship comes out smaller.
import { type ShipColours, type ShipSpec } from "../../recipes/ship.ts";
import { brigSpec } from "../../brig.ts";
import { galleonSpec, sloopSpec } from "../../fleet.ts";

export function scaled(spec: ShipSpec, length: number, name: string, colours?: Partial<ShipColours>): ShipSpec {
  const k = length / spec.length;
  const pair = (p: readonly [number, number] | undefined) => (p === undefined ? undefined : ([p[0] * k, p[1] * k] as const));
  return {
    ...spec, name, length, beam: spec.beam * k, depth: spec.depth * k, freeboard: spec.freeboard * k,
    sheer: pair(spec.sheer), castle: pair(spec.castle), mastHeight: spec.mastHeight * k,
    bowsprit: spec.bowsprit === undefined ? undefined : spec.bowsprit * k,
    flutter: spec.flutter === undefined ? undefined : spec.flutter * k,
    colours: { ...spec.colours, ...colours },
  };
}

/** The player's colours in the owner's concept sheet: a warm brown hull under blue sails. */
export const PLAYER: Partial<ShipColours> = {
  hull: [0.36, 0.2, 0.1], cloth: [0.18, 0.4, 0.86], team: [0.95, 0.85, 0.3], trim: [0.95, 0.78, 0.3],
};

export const brig = (name: string, length = 6, colours?: Partial<ShipColours>) => scaled(brigSpec, length, name, colours);
export const sloop = (name: string, length = 4.5, colours?: Partial<ShipColours>) => scaled(sloopSpec, length, name, colours);
export const galleon = (name: string, length = 8, colours?: Partial<ShipColours>) => scaled(galleonSpec, length, name, colours);
