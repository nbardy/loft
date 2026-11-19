Pushed `loft/l4b` at `1739a0f` with layered Kulla–Conty GGX, joint RGB media integration, and `std::quant`. [Lane notes](https://github.com/nbardy/loft/blob/loft/l4b/agent_notes/2026-11-20_v1/lanes/l4b.md)

All 12 focused tests passed without GPU use. The CPU twin matched the analytic dye slab and returned radiance `(0.375, 0, 0.375)` with transmittance `0.25` per channel in both media orders. Naga validated the marcher and both packed-dot and portable quant paths.

Colored lobes use luminance for the existing scalar transmittance. Scene-level `inscatter_joint` still needs the World sampler wired during integration.