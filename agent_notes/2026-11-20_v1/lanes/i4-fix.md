# Phase 4 repair

## Predictions before compilation

- Lipschitz diagnostic. Six samples give the local gradient direction. A separate secant along that direction supplies the violation evidence. For a valid L-Lipschitz field its exact slope is at most L, including at a kink. Account for endpoint rounding separately from the threshold. On the scored sphere fixture expect 0/12,916 red at scales 0.5 and 1, and 12,916/12,916 at scale 2.
- Material controls. At the same camera and pose, the Phase 1.5 material control gives 299 pixels above 2/255 for the single hermit. The Phase 4 layer and Kulla–Conty changes are expected to fail the old picture gate even after correctness and performance fixes. Compare the fixed candidate to the scored candidate as well as the manifest frames.
- Shrine. The 1,024² shadow map adds a second analytic branch traversal via proxy rasterization. For emissive kinds, the sun/sky quadrature has exactly one nonzero term, at zero light direction. Removing the remaining 13 iterations from those kinds should remove the normal, AO and shadow dependency for emit-only materials that do not use the normal. Measure the pass split before and after, with the scored preview as a control.
- Hermit-20. The shader repeats a dynamic lookup into a 1,122-element WGSL constant array at every direction. Retain all LUT samples and interpolation arithmetic, reading the data from the existing shared read-only table. Predict a material GPU saving of at least 0.335 ms, which is enough for the 5.6 ms gate if the other timings hold.
- Media. For two coincident media with total extinction log(4) per metre and equal monochromatic emission, 24 piecewise-exact substeps should yield T=0.25 and L=(0.375, 0, 0.375) in both authored row orders. A march with status -2 terminates the path before environment contribution.

## Findings

1. **OWNER APPROVAL NEEDED — layered materials.** The fixed build still misses all 18 approved frames. In the same compiler at t=0, replacing only the Phase 4 BSDF shelf with the scored base shelf changes the counts above 2/255 as follows. The locked allowance is 1,024 pixels.

   | Scene | Layered shelf | Base shelf |
   | --- | ---: | ---: |
   | Capsules, 1 | 8,418 | 0 |
   | Capsules, 20 | 30,648 | 0 |
   | Blobs, 1 | 5,280 | 0 |
   | Blobs, 20 | 27,870 | 0 |
   | Hermit, 1 | 599,245 | 299 |
   | Hermit, 20 | 511,267 | 994 |

   The legacy shelf control holds for all six t=0 frames. All 18 reference RGBA hashes match the manifest. Both shelves use linear HDR and one sRGB conversion at the target. The genuine path bug was providing the face-forward scattering normal as the material's local geometric normal on exit hits. Material evaluation now gets the outward geometric normal. A zero gradient produces an unresolved hit. Twin holds 36/36, and white furnace centre/rim remain 0.4999761/0.5000629 after moving the exact energy LUT values into storage. No threshold or reference changed.

   `out/meta/i4-fix/approval/` contains the 18 approved-frame pairs and the Heron and Shrine diagnostic pairs, all at WebP q95. The old Heron and Shrine are the integrator's same-camera main renders. Shrine's new woodblock appearance also needs an owner decision. `approval.json` records lossless pixel counts and approved hashes.

2. **Two-world Shrine wired. ESCALATE timing and shadow query scope.** `PrimaryWorld` draws the carved lantern, floor and 3,069 analytic branches. `SolidShadow` binds a separate closed lantern proxy and the shared floor and branches as shadow casters. The probe with two independent declarations compiles. The reference path builds independent BVH roots for named worlds. The Shrine woodblock path uses the mapped raster shadow pass and still needs per-hit World visibility.

   At 1,280×800 with 30 warmups and 100 timed close frames, the final same-machine scored preview was 5.980/8.942 ms p50/p95 and the repair was 6.387/9.429 ms. With the first repair and the last analytic cone early-out, the fixed p95 was 8.679 and 9.429 ms respectively. The original scored preview was 7.713 ms. The final visible-pass p95 was 5.132 ms by itself, beyond the full 4.0 ms budget. The final shade-pass median fell from 1.779 to 1.076 ms. The cone early-out did not shorten the visibility tail. The Naga counters and individual pass stamps are in `shrine-{scored,fixed}.json`.

3. **Authored RGB media fixed. ESCALATE remaining material operations.** An independently named path function calls typed `inscatter_joint` and `visible` against GPU-bound authored Worlds. Coincident slabs in both row orders produce L=(0.375,0,0.375), T=(0.25,0.25,0.25), RGB visibility=(0.25,0.25,0.25), and unit visibility in the separate vacuum World. The reference path composes L and T on camera and continuation rays, and its direct-light visibility includes RGB extinction. The synthetic marcher oracle still passes. The Charlie sheen shelf is present. Dielectric transmission, dipole/random-walk SSS and Gaussian media are open. The field marcher handles homogeneous boxes and an approximate directional single-scattering source.

4. **Lipschitz AOV fixed.** A directional secant independently checks the gradient estimate against the original threshold with an endpoint roundoff allowance. All 12,916 foreground pixels are sampled at each scale. Red counts are 0, 0 and 12,916 at scales 0.5, 1 and 2. Red background pixels are zero. All eight still/clip pairs match on frame zero and every clip published both frames. `aov.json` contains the readback counts.

5. **OWNER APPROVAL NEEDED — frozen bundle texture contract. ESCALATE native bake timing.** Shared execution needs an approved serial freeze for texture dimensions, formats and bake clocks. The frozen Bundle and goldens remain untouched. The native-only mip code is beside its sole caller, reducing Runner from 1,206 to 1,000 physical source lines under the review's exclusions. The native fixture encoded 20 load dispatches and 5/0/0/5/0 dirty/clean/unrelated/changed/repeated bake dispatches. It took 2.659 and 2.429 ms over the two repair measurements against the 2.0 ms gate. Cube/cull validation repairs hold. Multi-component dirty parameters remain open.

6. **Unresolved path status fixed. ESCALATE history, post and spectral device loading.** The authored RGB and spectral paths only add environment for status −1; −2 terminates transport. Closed furnace returns 1.499512/1.749268/1.998413, and the thousand-light mean is 545.625194 against 546.25 (−0.1144%). These fixtures do not exercise temporal reservoirs. Temporal reuse, reprojection and a scheduled bloom mip chain remain unimplemented. The spectral program validates in Naga, but its device load requests nine storage buffers against the negotiated eight after adding World resources. The existing world-word arena is the place to pack the new descriptors while preserving the binding count. Typed non-RGB `medium` also regressed at front-check; its extinction projection remains meaningful with rebound radiance. Both issues were found after the final permitted build.

7. **ESCALATE twenty-crab p95.** The full gate scored 11.252 ms against 5.6 ms. The unmodified scored binary rerun under the same GPU lock measured 11.246 ms, versus the integrator's 5.935 ms. All six p95 gates also miss in that same-machine scored rerun. The three submitted controls for the final crab timing were below 0.35 ms. Naga reports the shell module fell from 740 to 430 reconstructed/emitted WGSL lines with the lookup change, but shade median was 4.968 ms scored and 4.989 ms fixed. The first storage-LUT test also missed and the reflective-horizon early-out did not reduce cost. The result does not establish a speed benefit. Timing limits stay fixed.

## Verification

- Merged local and remote main at `5d98414` into the repair branch. The Phase 2/3 scored tree was still outside main.
- Build 1 stopped on a duplicate telemetry module declaration. Builds 2 and 3 completed with two Cargo jobs. The final full workspace test run recorded 98 passed, two failed and one skipped. The Shrine BVH test's local prelude omits `paper_grain`, required by the woodblock schedule. The existing Phase 4 typed contract test hits the Radiance issue above. The final spectral GPU smoke fails with the storage-buffer limit above.
- Fixed baseline gate exits 3. Pictures 0/18, Twin 36/36, Speed 6/12, Load 33/33, Truth 34/34. The matched scored rerun also holds 6/12 Speed. Earley and llguidance agree on 61/61 files after admitting the Phase 5 import fixture's quoted asset names. Core-word check passes.
- The instrument and raw timings, matched baseline records, readbacks and approval artifacts are under `out/meta/i4-fix/`. The branch is withheld from main.
