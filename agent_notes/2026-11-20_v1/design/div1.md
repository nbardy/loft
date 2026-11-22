# Crab seed admission, 23 November

A crab should be identifiable at thumbnail size from the outline of its working parts. The spider's long knees should enclose empty water. The fiddler's single raised crusher should outweigh its body. A hermit's shelter should change its centre of mass. Keep at least two fifths of the frame quiet around the silhouette. Use a dark contact or joint group, a middle-value shell and ground with distinctly different values, and one light focal accent. Each cell has one main light. Reflections, markings and joint highlights follow the surface. Use at most six related pigments per animal. Inspect the selected beauty, reduced and grayscale, before review.

## Instrument and prediction, registered before scoring

Use the pinned DINOv2 ViT-S/14 CLS vector on the complete 224-pixel square beauty. Its self-supervised visual features can distinguish a close composition from a wide one even when both would receive the same caption. The 22.1M-parameter model fits on the M4 CPU and leaves the GPU available for rendering. Record 1 minus cosine similarity to the nearest accepted image. [Model](https://huggingface.co/facebook/dinov2-small/tree/ed25f3a31f01632728cabb09d1542f84ab7b0056)

Render an actor matte through the identical camera and geometry, replacing actor and set materials with white and black albedo. Keep occlusion. Compare both frame-aligned masks and isotropically fitted, centred masks at 96 square. Take the larger IoU. This rejects rescaling or translating the same silhouette. For a third signal convert the masked subject to grayscale, fit it into the same square, smooth and compare gradient magnitude maps with cosine distance. The contour remains present under uniform recolours. Require all three inequalities against every reference, allowing different neighbours to set each bound.

Before scoring, I predict median nearest DINO distance below .07 in both crab cohorts, median maximum IoU above .88 authored and .93 ported, and median nearest grayscale-edge distance below .18. Prediction for the 44 leave-one-out decisions is zero passes. For calibration take the pooled medians of the 44 hard negatives. DINO minimum = median + .04 rounded up to hundredths, bounded [.08, .15]. IoU maximum = median − .10 rounded down to hundredths, bounded [.74, .82]. Edge minimum = median + .04 rounded up to hundredths, bounded [.08, .20]. Freeze these once, before generating. Report failures and outliers. Expected new result is 16/16 passing against all 44 old crabs, 12 old meta scenes and earlier winners, also when compared to later winners. Rank passing candidates by minimum DINO distance. Break ties by lower maximum IoU, then higher minimum edge distance and proposal index.

## Coverage

The four camera families are low across the ground, eye-level portrait, oblique three-quarter and overhead. Use different distances, target offsets and headings within these families. The four looks use the existing woodblock style, direct GGX, a coated BSDF, and an emissive-plus-diffuse BSDF. Woodblock is the only file in stdlib/styles on this revision. Four environment families are ripple sand, pebble bank, tidal pool and intertidal rock ledge. Light families are side sun, back rim, overhead and low crosslight. Pose groups are extension, guard, forage and carry. The 16 rows use an orthogonal array over GF(4) for style, camera, environment, light and pose (four values each, every pair once); anatomy occurs once by design. An action is interpreted through that animal's apparatus.

| Slot | Existing ID | Morphology and focus |
|---|---|---|
| 1 | copper-engraving | Japanese spider crab, tiny triangular body suspended between very long thin articulated legs. |
| 2 | spring-fiddler | Fiddler, one oversized open crusher held clear of the small feeding claw. |
| 3 | paper-cutout | Boxer, flattened carapace with a radiating anemone in each fist. |
| 4 | coral-regent | Decorator, irregular attached leafy debris rising from the back. |
| 5 | black-glass | Coconut crab, barrel-like segmented hindbody and heavy armoured forelegs. |
| 6 | timid-porcelain | Porcelain crab, low round disc and paired fans of feathery mouthparts. |
| 7 | bubble-scatter | Sponge crab, rear legs holding a lobed porous sponge overhead. |
| 8 | amber-tidepool | Land hermit, round moon-snail whorl with a lip around the aperture. |
| 9 | exhausted-glow | Pea crab, rounded translucent body with small folding limbs. |
| 10 | ferocious-signal | King crab, long angular legs and thorn-covered triangular shield. |
| 11 | blue-reef | Swimming crab, pointed lateral shell and visibly flattened hind paddles. |
| 12 | sand-scatter | Long-clawed hermit, tall tapering tower shell and unequal projecting pincers. |
| 13 | shimmer-patina | Anemone hermit, flared whelk with a wide lip and attached anemone. |
| 14 | pebble-walker | Ghost crab, tall paired ocular peduncles on a broad square front. |
| 15 | windup-snap | Box crab, two high shield claws meeting across its face. |
| 16 | jaunty-sketch | Arrow crab, extended rostrum and narrow triangular body between stilt legs. |

For row i=0..15 put x=i div 4 and y=i mod 4. The five columns are x, y, x XOR y, x XOR 2y, x XOR 3y, with multiplication modulo t²+t+1. Arrays and the concrete per-row assignment accompany the generator. Best-of-three changes proportions, joint angles and arrangement within the assigned cell. All three calls take the current contact sheet and the descriptor of every accepted seed. Read the pixels for composition and descriptors for previously used structures. Generate three in parallel, check and measure serially, then append one winner and rebuild the context. Save the actual contexts and all three distances. A new slot starts only after the previous choice. Budget for 48 proposals, 56 baseline references, and four training capture times per winner. Use the installed release, GPU lock, strict lint, both recognizers and the native clip budget receipts (2,000 ms CPU, 50 ms GPU at 512 square). No Rust changes are required.

Meta expansion should cover the following 12 cells, including object relationships and motion. Assign row r=0..11 a subject r mod 4 (animal, mechanism, architecture, natural process), composition floor(r/4) (pair in contact, procession, foreground/background interaction), look (r+floor(r/4)) mod 4 from the four looks above, and motion (r+floor(r/4)) mod 3 (articulated cycle, transfer between objects, propagated wave). Every class occurs in all three compositions and has three different looks. These are planned replacement briefs, not measurements on the old meta programs.

Keep the 28 ports as motion/edit ancestors pending review. Sample action extrema and acceleration reversals across their existing clips, try lateral and overhead cameras, and admit a still only if it independently clears this gate. Preserve the original source group across splits. Retire remaining duplicates from still generation and retain their paired action supervision. Leave the ported pack intact in this lane.

## Assigned crab cells

| Animal | Look | Camera | Set | Light | Pose |
|---|---|---|---|---|---|
| spider | woodblock | low across ground | ripple sand | side sun | extension |
| fiddler | woodblock | eye-level portrait | pebble bank | overhead sun | carry |
| boxer | woodblock | three-quarter | tidal pool | low crosslight | guard |
| decorator | woodblock | overhead | rock ledge | back rim | forage |
| coconut | direct GGX | low across ground | pebble bank | back rim | guard |
| porcelain | direct GGX | eye-level portrait | ripple sand | low crosslight | forage |
| sponge | direct GGX | three-quarter | rock ledge | overhead sun | extension |
| moon hermit | direct GGX | overhead | tidal pool | side sun | carry |
| pea | coated shell | low across ground | tidal pool | overhead sun | forage |
| king | coated shell | eye-level portrait | rock ledge | side sun | guard |
| swimmer | coated shell | three-quarter | ripple sand | back rim | carry |
| tower hermit | coated shell | overhead | pebble bank | low crosslight | extension |
| whelk hermit | emissive markings | low across ground | rock ledge | low crosslight | carry |
| ghost | emissive markings | eye-level portrait | tidal pool | back rim | extension |
| box | emissive markings | three-quarter | pebble bank | side sun | forage |
| arrow | emissive markings | overhead | ripple sand | overhead sun | guard |

All ten pairs of four-valued axes cover 16/16 possible combinations. A unique species cannot cover four settings of one axis in a single image.

## Twelve future meta briefs

Each planned seed gets a second subject that explains the first subject's pose or motion. Direct and coated looks can share the realtime schedule. Use the woodblock schedule for ink, and a lighting callback over realtime for emissive markings. Test the matched or reference engine on a second render of the optical scenes before claiming that engine meets the training budget.

| Row | Class | Composition and motion | Look |
|---|---|---|---|
| 0 | Animal | A heron folds its neck over a fish in its bill, articulated cycle. | Woodblock |
| 1 | Mechanism | A geared scoop hands a brass bead to a waiting cup. | Direct GGX |
| 2 | Architecture | A hanging shrine bell sends a wave along its attached rope. | Coated |
| 3 | Natural process | A kelp frond folds around a resting pebble in the tide. | Emissive markings |
| 4 | Animal | A line of salps passes a contraction along the chain. | Direct GGX |
| 5 | Mechanism | Three clockwork beetles walk out of phase along a rail. | Coated |
| 6 | Architecture | A row of canal locks passes a float from gate to gate. | Emissive markings |
| 7 | Natural process | A line of tide pools carries a wave from pool to pool. | Woodblock |
| 8 | Animal | A foreground otter passes a shell to its pup behind it. | Coated |
| 9 | Mechanism | A foreground hammer sends a pulse through distant suspended bars. | Emissive markings |
| 10 | Architecture | A foreground drawbridge articulates as a distant boat approaches. | Woodblock |
| 11 | Natural process | A foreground vent feeds bubbles into a distant ice pocket. | Direct GGX |

These 12 slots cover all 12 subject/composition pairs. Stage the transfer or propagation visibly in the admission frame, then verify the specified interaction across all four training times.

## Calibration on the original 44

The subject mattes show nearly coincident shell domes and feet even where the beauty changes from copper to blue or ink. All 44 fail the silhouette gate. Authored medians were 0.0656 DINO, 0.9522 IoU and 0.0835 edge. Ported medians were 0.0305, 0.9601 and 0.0339. The pooled medians were 0.0455, 0.9588 and 0.0474. The preregistered rule gives **DINO ≥ 0.09, IoU ≤ 0.82 and edge ≥ 0.09**. One authored image was 0.2705 from its nearest embedding neighbour and still repeated the outline.

Each row excludes itself and compares frame zero against the other 43 crabs. “Nearest” identifies the embedding neighbour. The maximum silhouette and minimum edge can select different references. The machine-readable scores retain all three IDs.

| Old seed | Nearest | DINO | Max IoU | Edge |
|---|---|---:|---:|---:|
| amber-tidepool | r38retry-before__before-0__1 | 0.0483 | 0.9564 | 0.0566 |
| black-glass | blue-reef | 0.2705 | 0.9726 | 0.0958 |
| blue-reef | v1__train-15__1 | 0.1314 | 0.9751 | 0.0834 |
| bubble-scatter | v1__train-18__2 | 0.0538 | 0.9349 | 0.0545 |
| copper-engraving | v1__train-16__0 | 0.0201 | 0.9845 | 0.0312 |
| coral-regent | r38retry-before__before-3__0 | 0.0809 | 0.9541 | 0.0323 |
| exhausted-glow | v1__train-18__2 | 0.0689 | 0.9328 | 0.1030 |
| ferocious-signal | shimmer-patina | 0.0804 | 0.9321 | 0.1132 |
| jaunty-sketch | smoke__eval-before__group-0__1 | 0.1836 | 0.9597 | 0.0902 |
| paper-cutout | v1__train-0__1 | 0.0496 | 0.9504 | 0.0569 |
| pebble-walker | r38retry-before__before-2__0 | 0.0361 | 0.9726 | 0.0307 |
| sand-scatter | v1__train-1__1 | 0.0500 | 0.9723 | 0.0194 |
| shimmer-patina | ferocious-signal | 0.0804 | 0.9497 | 0.0916 |
| spring-fiddler | v1__train-11__1 | 0.0622 | 0.9108 | 0.0836 |
| timid-porcelain | smoke__eval-before__group-0__1 | 0.1039 | 0.9395 | 0.1183 |
| windup-snap | v1__train-11__0 | 0.0534 | 0.9378 | 0.0978 |
| r38retry-before__before-0__1 | r38retry-before__before-2__0 | 0.0249 | 0.9595 | 0.0315 |
| r38retry-before__before-0__3 | v1__train-16__0 | 0.0299 | 0.9668 | 0.0194 |
| r38retry-before__before-1__0 | v1__train-18__2 | 0.0511 | 0.9872 | 0.0113 |
| r38retry-before__before-2__0 | r38retry-before__before-0__1 | 0.0249 | 0.9595 | 0.0356 |
| r38retry-before__before-3__0 | v1__train-14__1 | 0.0266 | 0.9657 | 0.0356 |
| smoke__eval-before__group-0__1 | v1__train-18__3 | 0.1022 | 0.9504 | 0.0610 |
| v1__train-0__0 | r38retry-before__before-0__1 | 0.0329 | 0.9480 | 0.0315 |
| v1__train-0__1 | v1__train-16__0 | 0.0489 | 0.9748 | 0.0239 |
| v1__train-10__1 | r38retry-before__before-3__0 | 0.0467 | 0.9361 | 0.0654 |
| v1__train-10__2 | v1__train-14__0 | 0.0263 | 0.9607 | 0.0283 |
| v1__train-11__0 | v1__train-11__2 | 0.0302 | 0.9424 | 0.1077 |
| v1__train-11__1 | v1__train-11__2 | 0.0320 | 0.9199 | 0.0836 |
| v1__train-11__2 | v1__train-18__3 | 0.0295 | 0.9636 | 0.0151 |
| v1__train-12__0 | v1__train-14__0 | 0.0314 | 0.9296 | 0.0538 |
| v1__train-13__0 | r38retry-before__before-3__0 | 0.0406 | 0.9854 | 0.0240 |
| v1__train-14__0 | v1__train-10__2 | 0.0263 | 0.9607 | 0.0283 |
| v1__train-14__1 | r38retry-before__before-3__0 | 0.0266 | 0.9471 | 0.0445 |
| v1__train-15__0 | v1__train-15__1 | 0.0305 | 0.9541 | 0.0323 |
| v1__train-15__1 | v1__train-15__0 | 0.0305 | 0.9581 | 0.0567 |
| v1__train-16__0 | copper-engraving | 0.0201 | 0.9854 | 0.0312 |
| v1__train-17__2 | v1__train-5__3 | 0.0189 | 0.9434 | 0.0474 |
| v1__train-18__2 | r38retry-before__before-1__0 | 0.0511 | 0.9872 | 0.0113 |
| v1__train-18__3 | v1__train-11__2 | 0.0295 | 0.9636 | 0.0151 |
| v1__train-1__0 | v1__train-15__1 | 0.0443 | 0.9684 | 0.0240 |
| v1__train-1__1 | sand-scatter | 0.0500 | 0.9751 | 0.0578 |
| v1__train-2__2 | r38retry-before__before-2__0 | 0.0505 | 0.9716 | 0.0548 |
| v1__train-5__3 | v1__train-17__2 | 0.0189 | 0.9399 | 0.0474 |
| v1__train-6__3 | v1__train-18__3 | 0.0865 | 0.9256 | 0.0610 |

## Runtime and provenance

The admission instrument uses the existing native clip, albedo, depth, check, strict lint, Earley, llguidance and Metal timestamp paths. DINOv2, cosine distance, intersection-over-union and the GF(4) orthogonal array are inherited methods. This lane adds the actor-only matte with stage occlusion, the frame/crop maximum, the calibrated three-way decision and sequential winner selection to the seed pack. A small procedural generator reads each reference tile's matte and luminance distribution, its descriptor, and the assigned cell to choose the least-repeated framing among sight lines for its anatomical study. The three parallel calls lengthen or flatten the shell, vary leg reach, knee angle, claw gape and dominance, and choose between different camera headings. No training service participates.

The parameterised Loft module emits the body, ocular stalks and tapered articulated leg pairs. The pincers and species-specific attachments are added in the authored programs. Ray intersections reuse the stdlib round-cone implementation. Each seed carries the module source inline because the trainer pins the stdlib tree. A separately importable copy lives on the Loft anatomy branch. Each successful selection retains four lossless beauty/depth pairs in the evidence directory. The training pack carries the four pinned q95 beauty frames plus the lossless frame-zero beauty, depth and matte. Its portable comparison index contains all 72 references.

For each slot, first capture three candidates at frame zero. Record all their distances, check strict lint and both grammars on candidates meeting the initial numeric gates, then visit the survivors in descending DINO order. Stop at the first with passing beauty/depth receipts at 0, 0.37, 0.91 and 1.53 seconds and a 60-frame profile (10 warmups, nearest-rank p95). This short circuit cannot exclude a stronger eligible candidate. Each winning profile records the adapter and triangle control. All captures use the shared GPU lock; other CPU workloads may be active.

## Scored replacements

The three hermits read through their round whorl, tall tower and flared lip. The paired boxer fans and rear swimming paddles remain visible in grayscale. In the low spider view the left toe reaches the frame edge, and the fiddler’s two pincer tips overlap from its portrait camera. Those are specific framing decisions for visual review. [Old and new, side by side](../../../out/meta/gallery/div1_sheet.webp)

All 16 winners passed the native checker, strict lint, both grammar recognizers, both runtime budgets and all three diversity thresholds. Against the complete 72-image reference collection, excluding self, the 16 winners have median DINO 0.3228, IoU 0.5297 and edge distance 0.4791. The closest DINO neighbour is 0.2474 away, the highest IoU is 0.6288 and the smallest edge distance is 0.4089. All 48 proposals have measured distances. Three exceeded the frame-zero timing budget. The first woodblock pass exposed a missing lighting prelude; the import was repaired before any selections.

| Training ID | Before DINO / IoU | After DINO / IoU | Final DINO neighbour | Final edge | Selected | Three DINO distances at admission (1 / 2 / 3) | GPU p95 ms |
|---|---:|---:|---|---:|---:|---|---:|
| copper-engraving | 0.0201 / 0.9845 | 0.3178 / 0.2607 | new/pebble-walker | 0.4089 | 1 | 0.4540 / 0.5685† / 0.5095† | 3.768 |
| spring-fiddler | 0.0622 / 0.9108 | 0.3264 / 0.5796 | new/windup-snap | 0.5714 | 2 | 0.2725 / 0.3340 / 0.2615 | 4.844 |
| paper-cutout | 0.0496 / 0.9504 | 0.3206 / 0.5249 | new/blue-reef | 0.4331 | 2 | 0.3243 / 0.3591 / 0.3334 | 2.987 |
| coral-regent | 0.0809 / 0.9541 | 0.3714 / 0.4977 | new/ferocious-signal | 0.5327 | 2 | 0.3242† / 0.3950 / 0.3614 | 6.319 |
| black-glass | 0.2705 / 0.9726 | 0.2922 / 0.5869 | new/ferocious-signal | 0.4300 | 2 | 0.2401 / 0.3165 / 0.2657 | 4.001 |
| timid-porcelain | 0.1039 / 0.9395 | 0.3327 / 0.5346 | new/blue-reef | 0.4331 | 1 | 0.3609 / 0.2780 / 0.2792 | 4.710 |
| bubble-scatter | 0.0538 / 0.9349 | 0.3591 / 0.6288 | new/windup-snap | 0.4651 | 1 | 0.4293 / 0.3481 / 0.3966 | 6.554 |
| amber-tidepool | 0.0483 / 0.9564 | 0.3383 / 0.6108 | new/shimmer-patina | 0.5374 | 1 | 0.3748 / 0.3103 / 0.3038 | 4.155 |
| exhausted-glow | 0.0689 / 0.9328 | 0.3249 / 0.6169 | old/v1__train-12__0 | 0.5150 | 1 | 0.3249 / 0.2975 / 0.2747 | 3.159 |
| ferocious-signal | 0.0804 / 0.9321 | 0.2733 / 0.4831 | new/pebble-walker | 0.4480 | 2 | 0.2713 / 0.2922 / 0.2752 | 6.385 |
| blue-reef | 0.1314 / 0.9751 | 0.2474 / 0.5249 | old/r38retry-before__before-0__1 | 0.4399 | 2 | 0.2467 / 0.2474 / 0.2265 | 4.384 |
| sand-scatter | 0.0500 / 0.9723 | 0.2915 / 0.5120 | new/windup-snap | 0.5871 | 3 | 0.3442 / 0.3680 / 0.4173 | 4.041 |
| shimmer-patina | 0.0804 / 0.9497 | 0.3383 / 0.6108 | new/amber-tidepool | 0.5374 | 3 | 0.3129 / 0.2960 / 0.3383 | 6.870 |
| pebble-walker | 0.0361 / 0.9726 | 0.2733 / 0.4817 | new/ferocious-signal | 0.4395 | 3 | 0.2554 / 0.2472 / 0.2733 | 4.405 |
| windup-snap | 0.0534 / 0.9378 | 0.2915 / 0.6169 | new/sand-scatter | 0.5457 | 3 | 0.2705 / 0.2591 / 0.2915 | 4.283 |
| jaunty-sketch | 0.1836 / 0.9597 | 0.4802 / 0.3150 | new/pebble-walker | 0.4931 | 2 | 0.3803 / 0.4802 / 0.4689 | 4.206 |

† Exceeded the frame-zero budget. Across 128 selected beauty and depth captures the highest CPU build was 1,636.249 ms and the highest single-frame GPU time was 13.165 ms. The slowest 60-frame beauty profile on Apple M4 Metal was 6.870 ms p95. Triangle p95 ranged from 0.042 to 0.611 ms. The 28 ported rows, their frames and their splits are preserved. [All scores, colour-independent neighbours, runner-ups and timing receipts](../../../out/meta/div1/scores.json)
