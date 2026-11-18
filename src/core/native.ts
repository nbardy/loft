// Registers the Rust kernels with src/core/switch.ts. Imported only when `LOFT_CORE=rust` (src/cli/main.ts).
import { rustKernels } from "./switch.ts";
import { shareRust } from "./share_rust.ts";
import { solveRust } from "./solve_rust.ts";

rustKernels.share = shareRust;
rustKernels.solve = solveRust;
