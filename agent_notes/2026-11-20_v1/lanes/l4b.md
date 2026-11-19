# Phase 4 L2 material, media, and quantization

## Predictions before implementation

- Joint media samples total extinction and source per segment. If the coefficients are constant, the closed-form segment composes to the exact slab solution for every positive step count. A source-free unit dye slab with extinction `[ln 2, ln 4, 0]` over white gives `L = [0, 0, 0], T = [0.5, 0.25, 1]`. The two coincident media have total extinction `ln 4` and normalized source `[0.5, 0, 0.5]`, giving `L = [0.375, 0, 0.375], T = [0.25; 3]` in both orders at the frozen `1e-4` tolerance.
- Integrating the height-correlated Smith GGX lobe at roughness 1 and normal view gives single-scattering energy `1 - ln 2`. The cosine-normalized Kulla–Conty term restores the missing `ln 2` for a perfect reflector, for unit total directional albedo and `0.50` under the white-furnace sky, within the existing 2% gate. A lower layer's contribution is multiplied by the upper lobe's directional transmittance. Top emission adds independently.
- Signed byte dot for `[-128,-1,0,127] · [2,-3,-128,127]` is `15876`. The same words read as unsigned yield `80900`. All unsigned lanes at 255 give `260100`. The portable lowering uses eight extracts, four integer multiplies and three adds. The feature path emits one packed dot per call. BF16 widening preserves the bits of each halfword and nibble unpacking preserves low-to-high order.

## Implementation

- The existing Loft lobe fold now accounts for emission independently of the ordered scattering stack. GGX, coat and film use height-correlated Smith GGX and the Kulla–Conty compensation term. Each uses its outgoing Fresnel and directional albedo for lower-layer attenuation. Colored Fresnel uses luminance for the existing scalar lobe transmittance. Lambert is available as a diffuse layer.
- One 33 × 33 unit-Fresnel directional table and one cosine-average per roughness row feed the CPU twin and WGSL. The directional table uses a squared-cosine coordinate for grazing resolution. The generator uses 16,384 visible-normal samples per interior cell, with an analytic roughness-one row and an exact cosine integral of the directional interpolant for the average. The shader emits the lookup only in modules that call it. Sources are [Kulla and Conty, 2017](https://blog.selfshadow.com/publications/s2017-shading-course/imageworks/s2017_pbs_imageworks_slides_v2.pdf) and [Heitz, 2018](https://jcgt.org/published/0007/04/01/).
- The joint marcher uses one midpoint query per segment and integrates the summed coefficients with Beer–Lambert attenuation in each channel. The small-optical-depth series preserves the range where direct subtraction loses precision, including a nonzero source in vacuum.
- The typed `std::quant` intrinsics use the existing integer words and scalar or vector registers. BF16 packing rounds to nearest even. Signed nibble extraction, indexed u2 extraction, and E4M3/E5M2 widening cover the other packed storage views. Dequantization scale and zero point remain ordinary arithmetic at the call site.

## CPU and Naga proof

- `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-passes` passed. The first check flagged overlapping borrows in the new media signature check, corrected before the focused run.
- `CARGO_BUILD_JOBS=2 tools/dev.sh test loft-passes --test furnace --test phase4 -E 'test(bsdf_media::) or binary(phase4)'` passed 12 of 12 selected tests. Nextest excluded three tests outside the filter.
- The CPU twin matched the predicted dye slab and both coincident orders at 1, 24 and 64 steps, including `T = [0.25; 3]` for the coincident pair and the vacuum/thin source limit. The layered lobe tests matched the predicted order and additive emission and the white-furnace quadrature met the original 2% tolerance at normal and oblique view.
- The compiler and CPU twin passed all signed/unsigned dot, BF16, 4-bit, 2-bit and FP8 vectors. Naga validated all modules of the same scene under both feature selections and validated the joint marcher with a supplied sampler. The signed packed dot returned 15876 and the unsigned packed dot 80900. No GPU checks were run in this lane.

## Files and integration

- `lib/bsdf.loft` defines the material lobes. `crates/loft-passes/src/bsdf.rs` and `src/bsdf/{energy.in,energy.js,inscatter.wgsl}` contain the shared lookup and twin/WGSL marcher.
- `crates/loft-passes/src/quant.rs` defines the intrinsic signatures, CPU evaluation, and feature-specific WGSL. Existing `src/{check,emit,eval,lib}.rs` entry points attach these operations to normal checking, the twin, and module emission.
- `crates/loft-passes/tests/reference/bsdf_media.rs` fills the predeclared fixture and adds the other lane proofs. This note records the predicted and measured acceptance checks.
- The World-aware caller supplies the summed sampler to `bsdf::INSCATTER_WGSL` and a positive step count, default 24. It clips the ray to the opaque hit or ray limit and composes `L + T * background`. The author-facing call type-checks with the frozen World payloads. The base compiler still returns the existing Phase 4 pending-lowering diagnostic for World calls, so World dispatch and the lighting sampler remain integration work. The closed-furnace path tracer is the engines lane's gate.
- The default compiler uses portable dots. An attached host can pass its `HostCaps` to `quant::compile` to select packed dots. The packed table schema and buffer contents are the same in both cases.
