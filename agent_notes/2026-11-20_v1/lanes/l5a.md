# Phase 5 L1 glTF import, retarget and live pose

## Status

Stopped at the shared contract on the Phase 5 freeze revision `b7471e8`. No implementation landed. The freeze supplies `GltfScene::mesh`, the 56-byte `MeshRow`, the compiler's Mesh refusal and an acceptance fixture. Live pose input needs a shared runner seam before this lane can implement the documented path.

## Evidence

- The Earley parser rejected the frozen `tests/phase5/anchor.loft` at line 2, column 25, on the first quote of `import_gltf("anchor.glb")`. The normative grammar has `STRING` for waivers and omits it from primary expressions. The Rust parser also omits string expressions and explicitly refuses a top-level `retarget` with E405. The shared diagnostic enum lacks E601, which the language spec assigns to missing retarget chains.
- The §02.1 host API specifies `Runner::push_input` and `InputValue::PosePacket`. The freeze has no `InputId`, host input entry point or bundle slot metadata. `loft-runner/src/encode.rs` binds a fixed 16-byte placeholder at group 0, binding 1. One frozen `PoseJoint` alone is 32 bytes. The live and replay lanes need to agree on slot ranges, packet payload layout and tick consumption at this boundary.
- The §06.18 Mesh path needs shared dispatch for bounds, triangle hits and material queries. The Phase 5 base has a populated-Mesh E405 refusal, a Phase 4 BVH stub, and an ignored CPU proof. The Phase 4 geometry branch provides `Bvh2`; the Phase 5 bundle lane owns serialization. Mesh assets can use generic tables, with their placement in the ASSETS section agreed at the bundle boundary.

## Proof and integration

The CPU Mesh and walk proofs remain pending. No Rust build, Naga validation or GPU work ran. Once the shared seams are available, the frozen anchor fixture should yield 20 rows sharing one asset, with 20 CPU hits at 1 m within the fixture's existing 1e-5 m tolerance. Compare one and twenty instances for byte-identical Mesh WGSL. Measure the maximum world-space drift during planted intervals after retargeting and forward kinematics, against 0.1 mm. Use the same packet trace to exercise live input and replay at the same ticks.

Before resuming L5a, land the Stage 0 string/member-call and retarget parser hooks, assign the runtime input seam across L5a and L5g, and pin the packet-to-slot and asset table handoffs. The existing Mesh, motion and BVH wire records can be reused.

## Files

Only this lane note changed.
