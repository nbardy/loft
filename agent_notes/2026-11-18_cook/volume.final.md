Implemented `draw.volume` for animated density, albedo, and emission. The march uses deterministic jitter, clips at opaque depth, and exits at 99.9% opacity. Sun and lamps use Henyey–Greenstein scattering with optional shadow marches. Toon and woodblock use the per-light scattering terms.

Measured dust at **1280×720 on Apple M4**:

| View steps | Shadows off | Six shadow steps |
|---:|---:|---:|
| 64 | 19.92 ms | 61.01 ms |
| 128 | 41.55 ms | 145.36 ms |

The volume adds zero dedicated textures or tables. First use of the shared depth path costs a derived 6.45 MB.

Typecheck, both analytic GPU tests, and the example builds and stills passed. The loaded dust artefact matched the compiled frame byte for byte. All 264 suite tests passed across resumed batches and an isolated pipeline retry.

Transparent layers require back-to-front ordering. Interpenetrating media remain approximate, and Rust execution remains unverified. A concurrent test-lock edit appeared in `tests/setup.ts`.

[Example](/Users/nicholasbardy/git/_wt/loft-volume/examples/volume_dust.ts) · [Notes and stills](/Users/nicholasbardy/git/_wt/loft-volume/agent_notes/2026-11-18_volume/README.md)