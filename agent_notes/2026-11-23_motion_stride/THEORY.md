# Stride: several feet aloft, one gait for any body plan

Lane `motion-stride`, from `motion-nfeet-2` (4b525fc7). Builds on `agent_notes/2026-11-23_motion_theory/THEORY.md`
§5 (duty factor, Wilson leader tree, capture test). Where this lane departs from it, it says so.

## 0. What is wrong today

`choose_foot` picks one foot per tick, only while no foot is aloft, by largest strain; ties go to the lower ID.
At eight sites all unstepped pins tie, so the crab sweeps 0→7, about 0.3 s per step, a 2.4 s cycle. At 0.31 m/s
the body travels v·T = 0.75 m relative to a foot that waits a whole cycle: the last legs trail 0.75 m. The
neighbour test `clear` is dead (it runs only when no foot is aloft).

## 1. State

Per site i (scalar columns, the only per-site storage a row has): anchor (x, z), age, and new **clock_i**,
**span_i**, **aim_x_i**, **aim_z_i**. `span_i > 0` means aloft, so the aloft set is derived, not stored. The single-swing
columns (`foot`, `swing`, `span`, `aim_x`, `aim_z`, `lift`, `rest_time`) and the unused neighbour pairs go.

Per plan, computed once in `walker_plan` (build):

- sweep `s`: min over sites of 2·min(d/2, 0.98·L − d, d − L_lo), d the hip-to-home distance (THEORY §5.2).
- swing time `T_sw = 0.25 s · sqrt(h / 0.45 m)`: today's 0.25 s at the stock height, scaled as a pendulum.
- wave rank and side offset per site, and a leader per site (§3).
- knee direction (body frame, mirrored per side) and sole radius: the two pieces of data the old plan lacked.

## 2. Duty factor and timing

THEORY §5.2: T(v) = T_sw + s/v and β(v) = 1/(1 + v·T_sw/s). It shortens the sweep at low speed to s = v·N·T_sw,
which caps β at N/(N+1). For eight legs that is β = 0.889: one foot at a time and a 2.25 s cycle, which is the
complaint, not a fix. **Departure:** cap β at 3/4 for every body.

    β(v) = min(3/4, 1/(1 + v·T_sw/s)),    T_st = T_sw·β/(1−β),    stride s_eff = max(v·T_st, 0.12 m)

At low speed every body cycles in T = 4·T_sw (1.0 s at the stock height): a quadruped walks one foot at a time
(β = 3/4 is its static floor), a hexapod or crab has one foot aloft per side, a biped has 50% double support.
The cycle no longer grows with leg count, so neither does the trail. As speed rises β falls with the full sweep
and reaches 1/2 at v* = s/T_sw: trot (4), tripod (6), alternating tetrapod (8), walk-to-run (2).

The 12 cm floor is the old 0.14 m strain threshold, slightly lowered: a standing body does not re-step for less.

## 3. Phase law

Stance progress is the stride the foot would take now, over the stride it should take:

    goal_i = c + R(yaw)·home_i + v·(τ + T_st/2),     u_i = |goal_i − anchor_i| / s_eff,    τ = sqrt(h/g)

goal_i is where the foot lands in steady travel (home, plus the capture lead τ·v, plus half a stride ahead),
moved to the present. It is 0 at touchdown and grows at |v|. Direction-free, so turning, reversing and stopping
need no case: at a stop goal_i is the home and u is the displacement over 12 cm.

Cycle phase θ_i = β·u_i in stance, β + (1−β)·clock_i/span_i in swing.

Target lift phases (Wilson): side A is socket z ≤ 0, side B z > 0; rank r counts same-side sites behind (smaller x).

    φ_i = fract(o_i + r_i·(1−β)),    o_i = 0 on side A, n_A/N on side B

n_A/N is 1/2 for symmetric bodies (Wilson's half cycle) and 2/3 for the tripod (evenly spaced, one foot at a time).
At β = 3/4: crab A 0, .25, .5, .75 and B .5, .75, 0, .25 (one foot per side aloft, sides alternate);
quadruped A 0, .25, B .5, .75 (lateral-sequence walk); hexapod A 0, .25, .5, B .5, .75, 0.
At β = 1/2: tetrapod, tripod, trot.

Leader tree: rank r on a side follows rank r−1 on that side; B's rank 0 follows A's rank 0; A's rank 0 is the
root. With Δ_i = fract(φ_i − φ_lead(i)), the error and threshold are THEORY's:

    e_i = wrap(θ_i − θ_lead(i) + Δ_i),    foot i may lift when u_i ≥ 1 + clamp(e_i, −0.30, 0.15) and age_i ≥ 0.12 s

Locally e contracts by (1 − β) per stride (P8). Among the feet that may lift, the one furthest past its threshold
lifts, if the support test (§4) passes; other candidates are tried in the same tick, so a veto never stalls a
foot that could go. Several feet are then aloft at once whenever their windows overlap.

## 4. Stability with k feet lifted; the veto

Let A be the aloft set and H' = hull(planted \ (A ∪ {i})) dilated by the sole radius ρ.

**Static path (H' is an area).** During the swing the pressure point travels from today's p to about the centre
of mass at touchdown, c + v·T_sw. Both are inside H' inset by r₀ = 2 mm (P5.2's rounding room) ⇒ the segment is,
by convexity. That is the condition: both points pass `contact` with margin ≥ r₀. Landings enlarge the hull, so
the test is conservative while other feet are aloft.

**Dynamic path (H' is a segment or a point).** No pressure point inside H' can hold the body; it will diverge.
The lift is admitted if the capture point at touchdown is reachable by this foot: with the spill
σ = q − Π_H'(q) (q the pressure point the controller wants), the divergence beyond steady travel over the swing is
σ·(e^{T_sw/τ} − 1), and the foot's goal plus that correction must lie within 0.98 of the horizontal reach of its
hip at touchdown. While aloft every foot's aim is re-targeted with the same correction against the current hull,
so it tracks the fall it is catching (capture-point stepping, P5.3 with p held at the projection).

The tripod needs this path: lifting one of three point feet always leaves a segment, so a three-legged body with
point feet cannot walk statically at all. The biped needs it for every step.

A biped also needs the sole: with two point feet the support between steps is a segment across the travel, and
the pressure point projected onto it has no component along the travel, so the body cannot start (a = (c−p)/τ²
has zero forward part). A sole of radius ρ lets p sit up to ρ behind c, a forward acceleration of up to ρ/τ².
ρ = 0 reproduces today's point contacts exactly.

A veto never forces a fall: a vetoed foot stays down, and if its pin runs out of reach the existing leash holds the
trunk back (a stall, which the audit counts).

## 5. Knee pole

Old pole: dir(0.28·(tip − hip)_xz + up). A crab foot left behind its hip pulls the knee backward, so legs read
as vertical sticks (motion-nfeet-2 §3). New pole: dir(0.28·(tip − hip)_xz + R(yaw)·[k_x, k_y, k_z·side]) with
the plan's knee vector k. k = [0, 1, 0] is the old pole bit for bit; crab-like plans use k = [0, 1, 1.2], so knees
rise sideways and outward; a heron uses a backward pole.

## 6. Predictions (to score)

Steady speed is |[0.82, 0.11]|·stick_scale. T = span + stance with span = T_sw·(1 + 0.16·s_eff).
"Trail" = the stance displacement at lift, |goal − anchor|, i.e. how far a planted foot has fallen back
relative to where it should be; today's crab reaches 0.75 m.

| body | N | sweep | T_sw | v | β | cycle T | trail at lift | feet aloft (mean) |
|---|---|---|---|---|---|---|---|---|
| crab (crab_live plan) | 8 | 0.640 | 0.250 | 0.314 | 0.750 | **1.01 s** | **0.236 m** (root), ≤ 0.271 m (followers) | 2.06 |
| hexapod | 6 | 0.581 | 0.236 | 0.372 | 0.750 | 0.95 s | 0.263–0.303 m | 1.55 |
| quadruped (upright) | 4 | 0.33 | 0.276 | 0.414 | 0.743 | 0.98 s | 0.29–0.33 m | 1.1 |
| stock walker (crossing) | 4 | 0.711 | 0.250 | 0.314 | 0.750 | 1.01 s | 0.236 m | 1.03 |
| tripod | 3 | 0.584 | 0.242 | 0.248 | 0.750 | 0.97 s | 0.180–0.207 m | 0.77 |
| biped | 2 | 0.272 | 0.250 | 0.248 | 0.750 | 1.01 s | 0.186–0.214 m | 0.51 |
| heron | 2 | 0.454 | 0.344 | 0.290 | 0.750 | 1.39 s | 0.30–0.35 m | 0.52 |

Crab, predicted against today: cycle 2.4 → 1.01 s; trail 0.75 → 0.24–0.27 m; feet aloft one at a time → two
(one per side, each side a back-to-front wave, sides half a cycle apart); the foot lifts 5 cm behind its home
(it lands τv + s/2 = 0.185 m ahead of it).

Cycle values hold when the root foot is never vetoed; a veto lengthens that cycle. Expected vetoes: the quadruped
at β = 3/4 lifts one foot at a time with the centre near the diagonal of the remaining triangle, so some lifts
wait (the same marginal case today's walker has). The tripod and bipeds lift only on the dynamic path; whether
the biped stays upright is decided by the run, not by this note: the capture correction e^{T_sw/τ} − 1 is 2.2
(biped) to 2.8 (heron), so a lateral spill of 5 cm moves a landing 11–14 cm, inside the reach.

Cost: the step law tests the support only for feet past their threshold (0 to 2 per tick, one hull each); the
trace changes by one rotated vector per leg. Predicted crab trace cost unchanged within noise (11 ms class).
