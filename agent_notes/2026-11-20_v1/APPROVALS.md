# Picture approvals

**Policy (owner, 20 Nov 2026).** Approvals no longer wait for the owner. The lead (the Claude worker manager) approves
look changes after looking at the old/new pairs. Codex workers still never approve their own changes: they render
pairs and queue them. An integrator re-pins a reference only when the change is listed as approved here.

## Approved

| Date | Change | Evidence | Verdict |
|---|---|---|---|
| 2026-11-20 | Phase 4 layered material, all 18 gate frames (capsules, blobs and hermit; 1 and 20; t = 0/4/9) | out/meta/i4-fix/approval/*_{old,new}.webp; out/meta/i6/approval_needed.md | **Approved.** Gradient sky, richer shell and coat highlights; geometry and pose unchanged; no artefacts. Re-pin tests/fixtures/gate-refs to the current main renders. |
| 2026-11-20 | Coral Shrine Lipschitz fix (loft/s1-lip 871ea94) | out/meta/s1_lip/shrine-review.webp | **Approved.** The old lantern was a truncated march; the new one is solid, with far fewer steps. |
| 2026-11-20 | Bead look (d1d, examples/bead.ts at 1.37 s) | out/meta/d1d/approval/bead-whole-{old,new}.webp | **Approved.** Visually identical toon bands, bore, rim and cast shadow; fixed band widths instead of derivative-dependent are not noticeable. |
| 2026-11-20 | Coral Shrine per-hit shadows (d1b) | out/meta/d1b/approval/coral-{old,new}.webp | **Rejected.** Cost Shrine 19.2 ms against a 4.0 ms budget; shadow maps (pf1a) stay. |
| 2026-11-23 | Seed diversity div1: 16 authored crabs replaced by species-hinted, sequential best-of-3 seeds (Creative loft/seed-diversity 6997e461) | out/meta/gallery/div1_sheet.webp; agent_notes/2026-11-20_v1/design/div1.md | **Approved for training seeds.** Distinct body plans, poses, cameras and scenes (median DINO 0.066→0.323, silhouette IoU 0.952→0.530). Weakest: fiddler (big claw reads as a blob, not a claw) and porcelain (low contrast); regenerate those two in the next pass. The 28 ported seeds still fail the gate and must be replaced the same way before they enter training. |

## Pending lead review
- Phase 5 corpus beauty pairs (out/meta/i5-r2/approval_needed.md). Approval alone certifies nothing; defects first.
- Sky bake look change (x2c), when it lands.
