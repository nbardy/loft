// Tables whose values are bounded shading data and can be stored as binary16.
import type { TableSource } from "./expr.ts";

export const shadingTables = new WeakSet<TableSource>();
