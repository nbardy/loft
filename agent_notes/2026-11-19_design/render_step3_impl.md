# Render implementation, Step 3

Branch `feat/design-render` starts at 5d94531, which contains the draws merge 4e8ee7b. Scope is the vertical slice in §8 of the consolidated architecture. The motion lane owns BODY, STATE, POSE and its tick. Render consumes ordinary packed rows. Both lanes need to reconcile the recorder version, native handshake and bundle reader if both extend them.

## Predictions before implementation

| # | Instrument | Prediction and acceptance |
|---|---|---|
| 1 | Hash every module at N=10,100,1000 | Identical texts, at most 8 pipelines and 2,000 lines |
| 2 | Diff row sections and module hashes | Washer edit changes one row only. Adding a bead changes three modules across the two engines |
| 3 | Native profile after device creation | 30 ms warm, 280 ms cold, ceilings 60/350 ms and spread ≤10% |
| 4–5 | GPU record and shadow against integer pixel-centre oracle | 9,664 and 12,080 ring samples, same counts shifted 128 pixels left for shadow. Solid shadow 12,892. Zero bit errors |
| 6–7 | Pick, secondary and bead probes | Washer ID 9/9 then 9/7, secondary distances 2/2 then 2/1. Bead miss, 2.2, 0.35 inward, 0.4 outward |
| 8 | Record vs independent centre ray, two cameras | Interior IDs identical and relative depth error ≤1e-4, tolerance 0.01% pixels |
| 9–11 | Linear radiance probes | Closed box 1.50/1.75/1.999985 E within 1%. Rough white metal 0.50 within 2%. One bounce mean bias ≤1% |
| 12–13 | Pass timestamps and whole-frame p50/p95 | Shade 3.0/5.2 ms, frame 4.5/6.7 ms at N=10/30. Training 21.7 ms, p95 ≤50 ms |
| 14 | Isolated still, clip and static viewer | Same still bytes. Viewer vs 64-sample still ≤1% |
| 15 | 9 bead frames from R0's commit in flat colour and log depth | 9/9 under rule (a) |
| 16–17 | Added core identifiers and o200k_base over runnable consumer cases | Zero domain identifiers. Cases 263, 231, 171+69, 377, 107 tokens within 15% |
