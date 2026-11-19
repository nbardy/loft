# Use cases that validate the meta engine (owner notes, 20 Nov 2026)

## 1. Spectral caustic still life (from an agent-built ISPC study, "Neon")
A thick green glass tumbler with water, trapped bubbles and droplets on wet paper with cursive ink, a dark wood table,
thin-lens depth of field and AgX. The vanilla `engines/reference.loft` is not enough; the scene exercises v1.2:
- **Refractive caustics (L S+ D E paths).** A ~60-line caustic photon or light-tracing pass, using `kernel ...
  allow(atomics, dynamic_loops)`: shoot sun photons through the glass and droplet bounds and refract them, then
  `atomic_add` fixed-point energy into a paper-UV caustic map that the path tracer reads. Manifold NEE is the
  alternative.
- **Hero-wavelength spectral transport.** Rebind `type Radiance = v4` (4 wavelengths, converted to XYZ and then to
  linear Rec.709 before agx). Use Cauchy/Sellmeier IOR(λ), per-wavelength Beer–Lambert absorption, and a 4-slot nested
  dielectric medium stack (air → glass → water → bubble → glass) with no fake air gaps.
- **Wet paper and feathered ink.** Bake the cursive Bézier stroke SDF to a texture at build time. In `Paper.material`,
  derive wetness from the droplet and puddle SDFs, blur the ink by wetness, darken the wet albedo, and blend with
  `mix_bsdf(dry_paper, wet_paper, wetness)`.
- **Point:** an agent keeps Loft's runner, BVH, primitives, AD normals, DoF camera and AgX for free, and writes only
  ~120 lines (the caustic pass plus the paper shader).
- **Target:** the scene converges at 2,048 samples on the M4 in seconds.

Status: Phase 4 stretch acceptance scene (`scenes/spectral_caustic_study.loft`), after the Phase 4 gates pass.
