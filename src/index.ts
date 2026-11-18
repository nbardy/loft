// The one entry (package.json exports only "."): every public name. The layer files below are how `loft api` groups the
// library, not import paths an author has to learn.
export * from "./core.ts";
export * from "./shapes.ts";
export * from "./motion.ts";
export * from "./actions.ts";
export * from "./checks.ts";
export * from "./styles.ts";
export * from "./lib/index.ts";
// Posed: the expression layer's type wins over the map algebra's, as it did when this file was flat.
export type { Posed } from "./motion.ts";
// Names two layers both use: the core layer's meaning is the entry's (`compose` places charts; a chain of folds is a list
// given to `tile`). The sdf meanings of `extrude` and `round`, the fold `compose` and the list-tree types stay importable
// from src/lib/index.ts.
export { compose, extrude, round, type Field, type Frame, type Key, type Placed, type Plane } from "./core.ts";
export { type Part } from "./actions.ts";
