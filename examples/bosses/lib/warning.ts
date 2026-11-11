// The warning marker in the print's palette, for every boss's `telegraph`.
//
// The telegraph's fill is translucent (its alpha throbs and grows toward arming: src/spacetime/telegraph.ts). A red or
// brick fill at partial alpha over the indigo sea printed as a muddy maroon, which is not an ink of this print (seen
// in the Wyrm, the Reefback, Thundermaw and the Tithe Fleet, 2026-11-11). Filled with the print's pale blue instead,
// any alpha over the indigo is still a blue of the print; the cream teeth and the key-black line round them are opaque.

import { WOODBLOCK_PALETTE } from "../../../src/index.ts";
import type { TelegraphStyle } from "../../../src/spacetime/index.ts";

export const PRINT_WARNING: Partial<TelegraphStyle> = {
  fill: WOODBLOCK_PALETTE.pale, fillFrom: 0.35, fillTo: 0.9,
  bone: WOODBLOCK_PALETTE.cream, ink: WOODBLOCK_PALETTE.black, flash: WOODBLOCK_PALETTE.cream,
};
