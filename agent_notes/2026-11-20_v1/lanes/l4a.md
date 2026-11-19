# Phase 4 L4a · Geometry queries

## Predictions before building

- A deterministic four-instance median leaf split gives 2,047 nodes and 3,071 leaf references for the 3,069 branches, floor and primary lantern. At the frozen strides that is 90,072 bytes. Reverse node order refits every bound once, with unchanged topology and IDs.
- At 512² pixel centres over [−4, 4]², the ring IDs and the shadow mask shifted 128 columns will have zero differences from the washer oracle. The two hole sizes give 9,664 and 12,080 hits. Filling the shadow-world hole produces 12,892 blocked samples.
- The traced bead gives an axial miss and distances 2.2, 0.35 and 0.4 m, including an inside exit. The bore and outside walls give −X and +X normals. The distance tolerance is 1e-5 m.
- Naga validates the traversal and the scene's emitted modules without opening a device.

## Implementation and measured proof

- `Bvh2` splits on the widest bound axis at the centroid median, with four rows per leaf and a stable (kind, row) tie break. The wire records use the frozen `BvhNode` and `LeafRef`. Refit validates membership and bounds before updating, and preserves node indices and leaf addresses.
- The CPU world dispatches each compiled kind once by its draw form. It calls the analytic `visible` query or marches the signed `trace` field with the derived Lipschitz bound. Marching uses the absolute distance to find the exit from inside. Nearest-hit and any-hit share the traversal. Unoccluded segments use the caller's RGB transmittance.
- The oracle compared every ID, depth and shadow bit over both 512² plates. It measured **9,664** and **12,080** washer hits and matching shifted annular shadows, with **12,892** blocked samples in the solid shadow world. The legacy object IDs 7 and 9 map to compiled `(kind_id, row_id)` pairs `(0, 0)` and `(1, 0)`.
- The bead's axial ray missed. The measured distances were **2.1999993**, **0.34999952** and **0.39999968 m**. The bore and inside-exit normals were exactly **−X** and **+X**. The largest distance error was below 1e-6 m.
- Coral Shrine built **3,069** analytic branch rows around the traced four-fold lantern and floor. The CPU BVH contained **2,047** nodes and **3,071** references, or **90,072 B** of wire data. The carved lantern transmitted the axial test segment and its solid proxy blocked it. The two snapshots emitted byte-identical shaders.
- `tools/dev.sh check loft-passes` passed. `tools/dev.sh test loft-passes --test bvh --no-fail-fast --no-capture` passed **5/5**, including Naga validation of the traversal and emitted scene modules. Earley and llguidance accepted all three new Loft sources. The first focused run exposed an invalid integer suffix in the scene; the retry passed with the same acceptance thresholds.

## Integration

Use `engines/coral_shrine.loft` with the default `shrine_distance=6.4` for the close camera and `shrine_distance=13` for the far camera. Build a second snapshot with `shrine_solid=1` for the shadow world. The current raster schedule is a preview with branch shadows. The woodblock/reference pass needs to bind both snapshots and link the per-kind intersection and field functions to `bvh::WGSL`. That pass should use the emitted query bodies, clip the leaf interval, and multiply clear visibility by media transmittance. The shader module reports an unresolved march separately from its miss sentinel. We did not measure GPU frame time, validate picture parity, or exercise device bindings in this lane.

`CpuWorld` uses drawn geometry in a matching `Program` and `Compiled` snapshot. It refuses grid-only kinds, which would need their own intersector. `Bvh2::refit` takes updated instance bounds for the same IDs.

## Files

- `crates/loft-passes/src/bvh.rs`, `crates/loft-passes/src/bvh/twin.rs`, and `crates/loft-passes/src/bvh.wgsl`
- `crates/loft-passes/tests/bvh.rs` and its `bvh/{washer,bead}.loft` fixtures
- `engines/coral_shrine.loft` and this lane note
