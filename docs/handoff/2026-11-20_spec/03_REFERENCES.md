# References: papers, systems and APIs worth reading

Cited from memory; titles and years should be checked before quoting. Grouped by the design question each one bears on.

## Language, staging and compilation
- Ragan-Kelley et al., *Halide: a language and compiler for optimizing parallelism, locality, and recomputation in image processing pipelines* (PLDI 2013): algorithm separated from schedule; the model for `schedule` blocks.
- He, Fatahalian, Foley, *Slang: language mechanisms for extensible real-time shading systems* (SIGGRAPH 2018): modules, interfaces and generics for shaders without permutation explosion.
- Hu et al., *Taichi* (SIGGRAPH Asia 2019) and *DiffTaichi* (ICLR 2020): a staged kernel language with sparse data and differentiation.
- JAX (Bradbury et al.) and its tracing and staging model; `torch.fx`: what goes wrong when the host traces silently.
- Willsey et al., *egg: fast and extensible equality saturation* (POPL 2021) and *egglog* (PLDI 2023).
- Terra (DeVito et al., PLDI 2013): multi-stage programming with a low-level target.
- naga (wgpu's shader IR): building `naga::Module` directly and validating it.

## Implicit surfaces, tracing, bounds
- Hart, *Sphere tracing: a geometric method for the antialiased ray tracing of implicit surfaces* (1996): the Lipschitz condition.
- Duff, *Interval arithmetic and recursive subdivision for implicit functions and constructive solid geometry* (SIGGRAPH 1992).
- Keeter, *Massively parallel rendering of complex closed-form implicit surfaces* (SIGGRAPH 2020) and the `fidget` project: tape evaluation, interval pruning, the Tier 0 idea.
- Inigo Quilez's articles on distance functions, smooth minimum, domain repetition and its limits, normals of SDFs.
- Media Molecule, *Dreams* rendering talks (Evans, SIGGRAPH 2015 Advances): SDF sculpting at scale.
- Galin et al., *Segment tracing using local Lipschitz bounds* (Eurographics 2020).

## Visibility and GPU-driven rendering
- Burns and Hunt, *The visibility buffer: a cache-friendly approach to deferred shading* (JCGT 2013).
- Karis et al., *Nanite* (SIGGRAPH 2021 Advances): GPU-driven culling, clusters, software raster.
- Wihlidal, *Optimizing the graphics pipeline with compute* (GDC 2016).
- WebGPU spec: `drawIndirect`, `RenderBundle`, no `multi_draw_indirect` in core; wgpu native features for multi-draw.

## Materials and light transport
- Burley, *Physically-based shading at Disney* (SIGGRAPH 2012 course).
- Kulla and Conty, *Revisiting physically based shading at Imageworks* (SIGGRAPH 2017 course): multiple-scattering energy compensation.
- Heitz et al., *Real-time polygonal-light shading with linearly transformed cosines* (SIGGRAPH 2016).
- Belcour and Barla, *A practical extension to microfacet theory for the modeling of varying iridescence* (2017): thin film.
- Bitterli et al., *Spatiotemporal reservoir resampling (ReSTIR)* (SIGGRAPH 2020); Ouyang et al., *ReSTIR GI* (2021).
- Majercik et al., *Dynamic diffuse global illumination with ray-traced irradiance fields (DDGI)* (JCGT 2019).
- Pharr, Jakob, Humphreys, *Physically Based Rendering* (4th ed.); Mitsuba 3 (Jakob et al. 2022) for the reference renderer and differentiable rendering.
- Jensen et al., *A practical model for subsurface light transport* (2001); Christensen and Burley, *Approximate reflectance profiles* (2015).
- Novák et al., *Monte Carlo methods for volumetric light transport simulation* (2018 STAR): joint integration of overlapping media.

## Non-photorealistic rendering
- Gooch et al., *A non-photorealistic lighting model for automatic technical illustration* (1998).
- Bénard and Hertzmann, *Line drawings from 3D models: a tutorial* (2019).
- Arc System Works, *Guilty Gear Xrd's art style* (GDC 2015): authored normals, outlines, stepped light.

## Motion: procedural, physical, learned
- Hecker et al., *Real-time motion retargeting to highly varied user-created morphologies* (SIGGRAPH 2008): Spore.
- Witkin and Kass, *Spacetime constraints* (SIGGRAPH 1988).
- Pratt et al., *Capture point: a step toward humanoid push recovery* (2006).
- Yin, Loken, van de Panne, *SIMBICON* (SIGGRAPH 2007); Coros, Beaudoin, van de Panne, *Generalized biped walking control* (SIGGRAPH 2010); Coros et al., *Locomotion skills for simulated quadrupeds* (SIGGRAPH 2011).
- Müller et al., *Position based dynamics* (2007); Macklin, Müller, Chentanez, *XPBD* (2016); Macklin et al., *Small steps in physics simulation* (2019).
- Jakobsen, *Advanced character physics* (GDC 2001).
- Clavet, *Motion matching and the road to next-gen animation* (GDC 2016); Holden et al., *Learned motion matching* (SIGGRAPH 2020); Zhang et al., *Mode-adaptive neural networks for quadruped motion control* (SIGGRAPH 2018).
- Peng et al., *DeepMimic* (SIGGRAPH 2018), *AMP* (2021), *ASE* (2022).
- Rosen, *An indie approach to procedural animation* (GDC 2014, Overgrowth).
- NaturalMotion Euphoria (dynamic motion synthesis), Rain World's procedural creatures (Joar Jakobsson, talks).
- NVIDIA MotionBricks (SIGGRAPH 2026): in-betweening transformer plus VQ-VAE, trained per rig.
- Thomas and Johnston, *The Illusion of Life* (1981): the twelve principles.
- Bird head stabilisation: Frost, *The optokinetic basis of head-bobbing in the pigeon* (J. Exp. Biol. 1978); the head-bob hold and thrust phases.
- Featherstone, *Rigid Body Dynamics Algorithms* (2008): articulated bodies.
