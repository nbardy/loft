# Legacy algorithm references

These examples remain readable at the immutable tag `legacy/2026-11-19` (commit `acb7e8b9`). To read a file, run `git show legacy/2026-11-19:src/style/sheet.ts` with the desired path. The [old library audit](../agent_notes/2026-11-20_v1/design/std1.md) records the costs and the reason for each decision.

| Legacy source | What to look for | Example or route |
|---|---|---|
| [src/style/sheet.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/style/sheet.ts) | CLI builds a style-by-camera comparison sheet. Tool, no shader contract. | Serpent, Shell, Serpent Scales |
| [src/light/history.wgsl](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/light/history.wgsl) | Accumulates radiance when current/prior IDs match at the same pixel. | Slice realtime option, default off |
| [src/draw/volume.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/draw/volume.ts) | Jittered bounded medium march with depth clip, optional shadow march per view step. | Volume Dust |
| [src/motion/wgsl/seams.wgsl](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/motion/wgsl/seams.wgsl) | Fixed-capacity Plan/State/Pose ABI, quaternion frame helpers. | Crab Live |
| [src/motion/wgsl/share_range.wgsl](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/motion/wgsl/share_range.wgsl) | Extremal feasible foot loads by enumerating singleton, pair and triple supports. | Crab Live |
| [src/lib/grow.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/lib/grow.ts) | KIFS level, nearest motif and accumulated stretch already use GPU iteration. | Coral |
| [src/grad.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/grad.ts) | Symbolic derivative rules including analytic noise gradients and normals. | Shell Relief, Spheres, Heron Leap |
| [src/geom/afloat.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/afloat.ts) | Samples sea height/slope to place a hull. | Boss scale sloop |
| [src/geom/bend.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/bend.ts) | Bends a constant-length stalk to its pose and returns tip frame. | Titan stalks, open lantern variants |
| [src/geom/crest.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/crest.ts) | Builds a fin sheet between authored edges of a body. | Wyrm, Thundermaw, Serpent |
| [src/geom/expression.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/expression.ts) | Composes poses from named channel sets, weighted layers and scores. | Titan faces and Lineup |
| [src/geom/fan.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/fan.ts) | Membrane between rays with catenary sag. | Stormwing, Wyrm, Thundermaw |
| [src/geom/gait.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/gait.ts) | Closed periodic course and stance/swing foot targets. | Titan, Islandback, Heron rig |
| [src/geom/grab.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/grab.ts) | Closed-form catch, carry, squeeze, release and ballistic pose. | Grab, Titan crane |
| [src/geom/lines.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/lines.ts) | Stitches many sagging ropes into one parametrized curve. | Tithe Fleet, sloop rigging |
| [src/geom/restless.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/restless.ts) | Uneven hold clock, darts and tremor. | Titan Jitter/Mourner |
| [src/geom/ring.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/ring.ts) | Damped crest/trough wave with outward push and lag. | Titan crash, crane |
| [src/geom/sea.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/sea.ts) | Projected grid and animated height chart, depth-based contact. | Wyrm, Titan, Wrecker |
| [src/geom/skin_distance.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/skin_distance.ts) | CPU nearest-triangle signed distance to a capped sampled skin. | Attack checks against skin targets |
| [src/geom/stance.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/stance.ts) | Steps/footing and body carriage pegged to fixed footholds. | Titan feet, Heron |
| [src/geom/struck.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/struck.ts) | Strike-and-hold pose; samples stillness. | Titan Mask, Owl |
| [src/geom/whirlpool.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/geom/whirlpool.ts) | Inward swirl, funnel and moving pool chart. | Maelstrom Wyrm |
| [src/map/brick.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/map/brick.ts) | Samples a sparse distance brick with conservative coarse rejection. | Static draw.field route |
| [src/map/nets.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/map/nets.ts) | Surface-nets extraction on a baked brick. | Static draw.field mesh route |
| [src/relief/displace.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/relief/displace.ts) | Derives shell frame and vertex displacement from differentiated surface. | Shell Relief |
| [src/relief/height.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/relief/height.ts) | UV patch stamps, ridges and creases around wrapped paths. | Serpent Relief, Shell Relief, Titan face |
| [src/relief/sculpt.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/relief/sculpt.ts) | Bicubic height grid from baked values and slopes. | Sculpt-layer fixture, no named flagship found |
| [src/relief/trace.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/relief/trace.ts) | Second-order local shell, bounded march or analytic root. | Shell Relief, Serpent Relief via draw.relief |
| [src/draw/field.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/draw/field.ts) | Bounds, meshes or marches a field, couples execution choice to draw helper. | Coral |
| [src/draw/trace.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/draw/trace.ts) | Bound proxy, ray interval and shared hit/ink coverage. | Field/traced examples |
| [examples/bosses/lib/scale_ship.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/examples/bosses/lib/scale_ship.ts) | Places a named sloop for scale and picks inks to suit each print rule. | Titan, Wyrm, Wrecker, Tithe, Thundermaw |
| [src/render/world.wgsl](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/render/world.wgsl) | Bounds and local-space hits over world-tagged rows, linear trace/occlusion loops. | Slice, Furnace |
| [src/draw/cast.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/draw/cast.ts) | Conservative per-chain sphere then rounded-cone segments. Legacy generates query code per scene. | Skeleton-stand-in query for ray-casting scenes |
| [src/draw/relief.ts](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/src/draw/relief.ts) | Draws the outer shell proxy, then invokes the surface-coordinate ray model. | Shell Relief, Serpent Relief |
| [rust/crates/loft-core/wgsl/hit.wgsl](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/rust/crates/loft-core/wgsl/hit.wgsl) | Clips a ray to its proxy then calls the authored hit and applies visibility/depth. | Native ray cover for traced solids |
| [rust/crates/loft-core/wgsl/march.wgsl](https://github.com/nbardy/loft/blob/acb7e8b947afe9544c2bbf00833883e7539b6029/rust/crates/loft-core/wgsl/march.wgsl) | Generates one medium density/transmittance/scatter body per K. This template marches a volume. | Native medium template corresponding to Volume Dust |
