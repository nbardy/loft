# Which foot can the crab lift?

*Motion design, Step 2 · 19 November 2026 · main at `80b2d8e`, look experiments at `88e2fc5`*

Give the animator a way to ask the crab to take its weight on one foot. Give the controller a way to answer how much weight that foot can take. Let the artist spend the answer on a change of silhouette, the stepper use it to free another foot, and the viewer see where the weight went. All three should be working on the same animal. This is the first capability I would build. [Step 1 critique](motion_astra_step1.md) · [Motion goal](../../docs/design/motion.md)

## What the consumer writes

These are proposed TypeScript call sites. Each produces a program for Rust to compile. The imports distinguish purchased work from work the author does in the shot. A body asset owns its anatomy and drawing geometry. The deck supplies a surface and its transform. Player velocity, fish position and hits are timestamped host inputs. A time in an author's shot is in seconds.

**A crab changes direction on a deck.** The player can reverse after the crab has lifted a foot. To give a tired leg a rest, the author changes the ranking of feet requesting a step. Each foot's fatigue here is a consumer-supplied value between zero and one. The weight the remaining legs can carry still decides whether the chosen foot can lift.

```ts
import { walk } from "loft/motion"
import { crab, deck, player } from "./cast"

const pose = walk(crab, deck, player.velocity)
crab.draw(pose)
```

```ts
const pose = walk(crab, deck, player.velocity, {
  rank: foot => foot.strain.add(foot.fatigue)
})
```

**A heron watches a fish while hopping a rail.** The director owns takeoff, touchdown and the crouch. The fish can turn during flight. At the neck limit this director would let it escape the bird's gaze. Another director can supply a function which requests a turn of the body at the next supported instant.

```ts
import { hop, track } from "loft/motion"
import { heron, fish, deck, rail } from "./cast"

const leap = hop(heron, {
  over: rail, onto: deck, takeoff: 0.6, land: 1.2, prefer: heron.crouch
})
heron.draw(track(leap, {
  eyes: heron.eyes, point: fish.position, skullUp: deck.up, atLimit: "release"
}))
```

**A boss takes two hits while holding a rail.** The left palm has already closed on the rail when the shot begins. The guard contains the facial hold. The hits arrive from the game with their application point and impulse. The antenna follows the head that actually emerged from recovery. The grip is an input to recovery, so the shoulder can yield before the hand would have to let go.

```ts
import { pin, recover, follow } from "loft/motion"
import { titan, rail, arena } from "./cast"

const guard = pin(titan.guard, titan.leftPalm, rail.socket)
const struck = recover(guard, arena.hits)
titan.draw(follow(struck, titan.antennae, { hz: 3, damping: 0.45 }))
```

**A six-legged animal with short front legs.** This author has no motion asset. The two-link leg constructor obtains its rest joint frames from the hip, home, lengths and bend direction. The trunk is level at rest. The shared drawing function takes its link geometry from this description. Changing the length in the first row on the negative side requires no gait edit.

```ts
import { body, leg, walk, drawLinks } from "loft/motion"
import { deck, player } from "./cast"

const beetle = body({
  trunk: { mass: 2.4, height: 0.24 },
  legs: [-1, 1].flatMap(side =>
    [0.28, 0, -0.28].map(x => leg({
      hip: [x, 0, side * 0.16],
      home: [x, -0.24, side * 0.42],
      lengths: [x > 0 && side < 0 ? 0.19 : 0.25, 0.24],
      bend: [0, 1, side]
    }))
  )
})
drawLinks(beetle, walk(beetle, deck, player.velocity))
```

**An external performance comes in and posed joints go out.** A recorded or generated take has local joint transforms, a rest pose and a sample clock. The artist supplies correspondences and marks the interval of the actual grasp. The handle can move. The exported frames include the corrected palm orientation, important for a handle that twists in the actor's hand.

```ts
import { retarget, pin, writeSkeleton } from "loft/motion"
import { titan, take, jointMap, handle } from "./cast"

const acting = retarget(take, titan, jointMap)
const grasp = pin(acting, titan.rightPalm, handle.socket, [0.4, 1.2])
writeSkeleton(grasp, { rate: 60, file: "grasp.glb" })
```

**Replace the entire locomotion engine.** A ribbon eel swims by passing a wave of curvature down its spine. Its joints carry distance along the spine in metres. The local yaw amplitude below is for equally spaced joints. For unequal spacing an author would multiply curvature by the rest distance to the next joint. A row map produces local joint frames, and the same drawing function that accepts a performance accepts these frames.

```ts
import { time, sin } from "loft"
import { eel } from "./cast"

eel.draw(eel.joints.map(joint =>
  joint.rest.yaw(sin(time.mul(5).sub(joint.arc.mul(9))).mul(0.22))
))
```

| Author's work | TypeScript tokens | Model tokens (`o200k_base`) |
|---|---:|---:|
| Crab, complete call site | 35 | 41 |
| Replacement crab assignment, including the entire call | 30 | 29 |
| Heron, complete call site | 84 | 104 |
| Boss, complete call site | 71 | 79 |
| Beetle, including anatomy and drawing call | 140 | 179 |
| Import, edit and export | 68 | 89 |
| Eel, including its entire motion law | 61 | 56 |

The TypeScript 5.9.3 scanner counts imports, punctuation and literals, skipping trivia and EOF. The model count uses the exact fenced contents, including the final newline. Step 1 counted 125 source tokens for the whole varied-scramble scene, 1,371 for its stepper before diagnostics and 1,211 for the gaze scene through its declaration. The scopes differ. The new crab buys a rig and public controller. The heron buys a directed crouch and obstacle geometry. The boss buys the guard, a marked grip surface, antenna pivots and physical properties for the driven parts. The import buys the correspondence map per pair of skeletons. The beetle authors its anatomy; the eel authors its law. Count new asset fields, custom law source and call-site tokens separately in implementation reviews. A rig file that secretly scripts liftoff fails the beetle example. [Counts and fixture boundaries in Step 1](motion_astra_step1.md)

## What lasts from one instant to the next

There are two different crabs at the same time, position, heading and velocity. Yesterday one placed a foot 20 cm farther up the deck to miss a stone. It stands there still. Tomorrow's program must be able to draw either crab. The least state that distinguishes them is the identity of the support and the point the foot took on it. By contrast, a rail hop with a known takeoff velocity can be drawn at its apex with one quadratic. Running 108 simulation ticks to find that height at 0.9 seconds would be a strange price for knowing the answer. The unit of temporal choice should be a quantity, such as the body trajectory or the left contact, and its dependencies. [Stance](../../docs/PHILOSOPHY.md) · [Existing gait](../../src/geom/gait.ts)

I would expose a body, ordinary functions over it, and two operations Loft nearly has. One advances a typed state from a previous value and the inputs delivered at an integer tick. The other draws or observes geometry from the resulting pose at the presentation time. In notation, for tick boundary `t[k]`,

```text
left limit at t[k] = evaluate the span carried by S[k−1] at its end
S[k]               = law(left limit, earlier state, inputs[k], events[k], body)
pose(t)            = present(S[k], shot(t), surfaces(t), t − t[k])
marker(t)          = place(pose(t), local feature)
```

A span is simply the function valid until its next boundary. A stationary point on a deck, a flight parabola and an underdamped response have different functions. Each can be written directly. At a liftoff the swing inherits the actual pad position and velocity. At landing the point takes deck coordinates and its new relative velocity follows the named impact law. Half-open intervals put an event at the start of the new law. Only an explicit impulse can jump a velocity in a law that promised continuous velocity. The author can supply an impact that absorbs over three frames instead. A join gets checked for the particular continuity it requests.

An author can supply a closed expression, a finite recording, or a live initial value and transition. A pure expression can use the clock and any inputs declared available at that time. Baking requires known inputs over the requested interval. A live transition can read history and the current packet. A future player command is unavailable. A predicted landing point calculated from today's velocity is available, and a swing can revise that prediction at the next tick. Give the host an integer tick and fractional presentation offset with an explicit origin and rate. Explicit reset starts a new history. A paused display with a different camera evaluates the same state. A stalled 60 Hz display advancing 50 ms consumes six 120 Hz packets in order.

A body need only buy the mechanics its laws read. A point-mass model needs mass and a centre. A joint-driven torso needs inertia and local pivots. A drawn plume can have neither. The following are library types in mathematical notation. IDs are stable within an asset. `Local` and `World` below describe frames at authoring boundaries. They erase to numbers or rows after validation.

| Value | Fields and meaning |
|---|---|
| Body | Root and parent-ordered segments with stable IDs, local rest rigid transforms, named features and draw bindings. Separate optional mass, local centre and inertia table keyed by segment. |
| Joint motion | Fixed, hinge with local axis and interval, ball with local swing cone and twist interval, or consumer function returning a local frame. A prismatic joint is already expressible by that function. |
| Feature | Owner segment and local point, directed ray, or rigid frame. A sole pad uses the point, an eye uses the ray, a palm uses the full frame. A procedural surface may supply a parameterised feature and the same evaluator it draws. |
| Surface | Stable instance and generation, coordinate map, world frame/velocity at a time, query of the permitted patch. A rigid deck needs an inverse transform. A deforming triangle can use primitive ID and barycentric coordinates. |
| Contact | Attached(surface, local feature, since tick), Swing(start pose and velocity, goal, start and end ticks), or Free. The fraction of weight is a separate scalar. An attached toe can carry zero. |
| Performance | A body, local frame rows, named commitments and any expressions/state they read. A producer may also expose its available degrees of freedom to a caller. A bare row producer suffices for drawing. |
| Tick input | Per-stream timestamped level or ordered edge with stable sequence and payload. Positions and directions name a frame. An impulse is in N·s about a point in metres. Angular data use radians. |
| Constraint | A predicate or signed margin over named final features, a domain of times/inputs and tolerance with units. A law can construct a solution, a build can optimise for it, or an observer can check it. |
| Evidence | Existing verdict, scope, witness and bounds, tied to the resolved feature expression, performance identity and domain. A build result can also carry an unproved obligation. |

Validate parent cycles, unique IDs, finite data, unit axes, valid quaternion frames, ordered limits, reachable rest constructions and positive masses wherever the selected law needs them. Report the offending segment and source site. Scale imported lengths once into metres and change handedness and joint bases at import. Arrange quaternion signs continuously between adjacent keys. Preserve twist around a bone even when its endpoints are stationary. Copying the positions and radii of today's swept `Skeleton` would lose a quarter turn of the palm. A segment can drive many sweep stations. One extra station to round the wrist should change neither a bone index in an export nor the point the hand holds. [Current skeleton and frame types](../../src/map/types.ts) · [Export path](../../src/bake/gltf.ts)

Use a single writer for each final local degree of freedom over a span. A function can take ownership of a subset from its input performance and return the reconstructed pose. Two independent writers to the same axis require an explicit function of the two values or disjoint intervals. A library can offer additive twist on a free axis. A grasp specifies the admissible palm frames. Changing shoulder pose with the grasp active means reconstructing the arm against the same frame, or reporting the instant it becomes unreachable. The caller chooses whether to release, move the trunk, or refuse the shot. Passing the commitments as ordinary data lets `recover(pin(...), hits)` spend the unlocked coordinates. Changing the order asks a different question, with a fresh check at the output. A weight of 0.9 has no implied contact semantics.

## An editable law for bearing weight

Start with what the eight-legged crab has in excess. Its pads can share an identical total force in many ways. On a level, rigid deck use a reduced body of mass `m`, constant centre height `h`, level deck coordinates `(x,z)`, and constant angular momentum. Let `c` be the horizontal centre of mass, `a` its requested horizontal acceleration and `g` the positive magnitude of gravity. For pad positions `p[i]` in that plane, the vertical shares `w[i]` can support this request when

```text
q = c − (h/g) a
sum w[i] = 1          sum w[i] p[i] = q          w[i] ≥ 0
F[i].up = m g w[i]    F[i].horizontal = m a w[i]
```

Force and moment balance give the first line. The stated force distribution also satisfies each isotropic friction cone when the common coefficient obeys `|a| ≤ μg`. Parallel horizontal forces give no net yaw moment about the centre because `q − c` is parallel to `a`. This construction assumes coplanar supports and a lumped body. Motor torque limits, inclined walls, a pushing partner and a twisting contact require a richer wrench model. An artist can still pin a toe kinematically on any of those surfaces. The contact mechanism and the force law have different domains. Robotics already knows how to reason about feasible wrenches and distribute contact force. [Caron, Pham and Nakamura](https://scaron.info/papers/journal/caron-tro-2016.pdf) · [Force distribution in multilegged vehicles](https://www.cambridge.org/core/journals/robotica/article/abs/optimal-force-distribution-in-multilegged-vehicles/6DBA4A8471D54CB848B745234298D063)

Now ask for the least and greatest share a *particular* pad can take while preserving those three equalities. Each feasible load vector is a point in a bounded convex polytope. At an extreme at most three independent columns `(1,x,z)` carry positive weight. With four positive columns there is a linear dependence and a small move along it in both directions keeps the load feasible. Thus enumerate triples, pairs and singletons. A triple uses the signed-area barycentric coordinates of `q`, a pair uses a segment test, a singleton a point test. Keep the two viable candidates minimizing and maximizing the chosen foot's share with their complete load vectors. Interpolating those vectors realizes every requested share between the endpoints and keeps the resultant fixed. This is elementary linear programming. Our proposed use is to hand that *interval and its two poses' worth of force data* to the author as the control for a performance.

Four pads make the claim directly testable. Place them at `(±0.30, ±0.20)` m. Ask for `q=(0.15,0)`. Exactly 0.75 of the weight belongs on the positive-x side and 0.50 on the positive-z side. In the order `(++,+−,−+,−−)` all answers are `(u, 0.75−u, 0.50−u, u−0.25)` with `0.25 ≤ u ≤ 0.50`. The director can double the first foot's load with the same root, acceleration and four contact positions. For 10 kg the requested release of that foot is short by at least 24.525 N. A generic stepper that just marks it airborne would have to change the root dynamics to justify the shot. At `q=(−0.03,0)` zero is attainable with positive margin inside the other three pads. The same function answers a question of acting and a question of locomotion.

In the crab recipe a controller requests acceleration from the velocity error, bounded by available friction. A foot's strain is the distance between its retained anchor and a candidate foothold, divided by usable reach. Candidate footholds start from the rest footprint translated by locally estimated velocity times swing duration. Intersect that target with the actual deck patch and reachable region; reject a missing intersection. The most urgent eligible foot asks to unload. An attached foot may leave when a witness using the other contacts places `q` safely inside their support, the swing has a reachable destination, and the previous swing has landed in the simple one-at-a-time recipe. The safety inset is a length in metres. At a marginal edge one can direct a balanced pause or choose a dynamic recovery law. A different ranking, concurrent lifts or costlier foothold search is an edit to this function.

A request outside the current support has a consequence. For the planar model choose the nearest feasible pressure point on the convex hull, then calculate the acceleration the contacts can deliver, `a'=(g/h)(c−q')`. For an exterior query the nearest point occurs on a segment or at a pad, already available during enumeration. Recheck friction after projection. If the hull and this simple friction policy cannot produce a witness, emit a loss of support with the residual, and invoke the caller's slip, fall or stop-and-report law. The default on a frictionless released body integrates momentum and gravity until its next contact query. Never invent a force from an absent pad. At each tick the planar integrator advances position by `v dt + a' dt²/2` and velocity by `a' dt`. The pressure witness describes that tick's decision. Continuous-time stability over an arbitrary trajectory would require an additional check.

Take care with numerical and artistic ties. Use deck-local coordinates recentered near the body. Test nonnegative weights, sum and position residuals after constructing a candidate. Marginal determinants or signs that could change the extremum produce an uncertainty flag or an offline higher-precision check. Collinear and coincident sets still have their pair and singleton cases. An empty set gets the flight or unsupported variant. Resolve equivalent extrema by proximity to the previous distribution and then stable pad IDs. On moving through a basis change the other loads can still jump. Ramp the *requested* share across a transfer, filter the visible response, and measure support-switch pops. A director needing continuous forces can substitute minimum-change constrained fitting of the full vector at each tick. Smoothing a previous load vector after a change of support could assign weight to a lifted foot.

Spend the load on a degree of freedom the eye can read. A three-link crab leg has a choice of terminal segment direction and a choice of bend. In the first recipe, a loaded leg brings its ankle toward the body and folds at the knee while its tip stays on the same point. Aim the terminal link a further 32° per unit of normalised pad load toward its hip-side tangent, within the rig's angular limit. Solve the remaining two links analytically. At the two extremes of the rectangle the same pad's terminal orientation differs by 8°. With a 1.2 m link the ankle endpoints are about 0.167 m apart. A shot can mute or amplify this response. The shell can lean about the model centre in response to braking, and the heavy claw can complete the turn later. A rigid tilt derived solely from the first moment of the load would erase the distinction between the two performances because both have exactly the same first moment. The leg bend is where this particular choice becomes visible. [Three-link construction](../../src/geom/limb.ts) · [Scramble sheet at the game angle](../../../_wt/loft-motion/agent_notes/2026-11-19_motion/crab_scramble_3.webp)

The parent builds the load interval on the *attainable* set of contacts. Each proposed stance is realised through the allowed FK/IK, including the terminal direction, and checked on the drawn pad. If the extra bend consumes the last millimetre of reach, shorten the expressive bend first. If the neutral chain also misses, remove that support and recompute once. A second failure at this tick yields explicit unsupported state and a visible recovery, with the failed pad and residual. This bounded policy handles the circular dependency between the load and the pose which makes that load available. It also makes the worst-case work countable. On very rough ground or bodies for which the planar reduction fails, replace the whole support law. A general contact solver is an attractive consumer of this interface.

## Directing the bird and the boss

For the heron, split the known performance at takeoff and landing. With up positive, flight of duration `T` from `y0` to `y1` fixes `vy=(y1−y0)/T+gT/2`. At equal heights and `T=0.6` s this gives 2.943 m/s at takeoff and an apex 0.44145 m higher. Eliminate those unknowns before fitting anything. On land choose curves close to the keyed crouch and absorb, and enforce the velocities at the joins specified by the impact model. The director can change the landing force or leave the arrival velocity discontinuous at a hard contact. Solve only the remaining free coefficients. Check the side of the rail as well as the separation of each required piece of the *final* bird. An infeasible deck edit reports a beat, feature and margin when available. It leaves the artist able to change the landing time, the takeoff or the tuck. [Heron problem](../../examples/second/heron_leap.ts) · [Rust solve measurements](../2026-11-19_lean_rust_solve2.md) · [Witkin and Kass](https://www.cs.cmu.edu/~cga/dynopt/witkin/spacetime.pdf)

Three independent quantities direct the head. The eye ray can pursue the fish. The skull can resist roll or deliberately pitch for one glance. The head position can stay still while the shoulder advances, then thrust to regain neck range. Independently measure angular eye error, skull attitude, and position relative to the reachable neck. Evaluate neck length on the actual centreline. A desired chord longer than the neck's arc length is an immediate witness of infeasibility. A pair of arcs of total length 1.55 m can realise a suitable shorter chord; solve the scalar half-angle relation with a stable straight-limit case, then verify joint and end-tangent limits. A head controller stores its previous turn direction or unwrapped angle around the rear blind spot. A fish crossing the branch cut of `atan2` cannot flip a saturated head from one stop to the other in one tick. After release, reacquire from the present ray when the fish re-enters range. If the body accepts a turn request, spend it only in the body's available coordinates. Pigeon experiments motivate separate translational and rotational responses. The director supplies the timing and range for this heron. [Head stabilisation experiments](https://www.frontiersin.org/journals/neuroscience/articles/10.3389/fnins.2017.00551/full) · [Heron's revised glance](../../../_wt/loft-motion/agent_notes/2026-11-19_motion/heron_hop_gaze.webp)

The bird makes the time split worthwhile. A forward seek in a known hop indexes the body span directly. The fish-driven eye on that same bird requires its history if release has hysteresis. A check about a foot landing at 1.2 s can survive an edit to eye colour. A rail-clearance check that reads the bill must follow the new head. Build the dependency from the bill attachment and the geometry that draws it, through the final neck law. If a live target can enter any position, a finite fish trace gives a measured bill clearance for that trace. To obtain a continuous assertion, bind a domain for the target and enclose the actual final function over that domain. A goal line for the bill in a different pose buys no evidence.

For the boss, start with as few driven masses as the second hit can distinguish. For example, with effective moment of inertia 2 kg·m² about a supported axis, a transverse impulse of 3 N·s at a 0.4 m lever arm adds 0.6 rad/s when its moment points along that axis. Compute the lever and effective inertia again for the next impact using the new support. Carry position and velocity. A slipping foot can remove a pivot and change the second reaction. A producer that uses a joint solver can instead apply the impulse and satisfy the grip through the remaining joint coordinates within their torque and angular limits. The face holds its guard expression; time since impact drives the body's recovery and the antenna's relative rotation. A parent rotating by 10° and a follower desired at an absolute 7° imply a local correction of −3°. Reapplying the 7° as a local rotation makes the antenna lead the parent. Known shot impulses can buy a sum of closed responses as long as the law and pivot stay fixed. [Existing driven pendulum](../../src/geom/pendulum.ts) · [Titan's brow](../../examples/bosses/titan/face/brow.ts)

For retargeting, map source rest-relative rotation into the target joint's rest basis, then compose with the target rest frame. Match planted *features* in their surface frames. A long forearm and a short forearm place very different palms under the same angular motion. Solve only the chain needed by the grasp and measure the rendered palm frame afterwards. Missing correspondences are explicit omissions or import errors according to the caller's requested tracks. Export local translations and rotations, parent links, inverse bind transforms when skinning, and the sample clock. Split keys at releases and impacts. If the destination player interpolates baked joint keys, check the exported playback at intermediate times too. A held palm at the keys may drift between them. Adaptive subdivision can meet a declared sampled tolerance; continuous assurance requires a bound for the interpolation. Save a source clip or generator version, rig mapping and correction graph with the take's identity. The gameplay host can ignore export altogether.

## The compiler and runtime this takes

Put the mechanics in public source an animator or model can copy. In the stable core keep program modules, typed resources and their read/write dependencies, pass order and execution on the caller's device. The only new general runtime capability needed for the first experiment is delivery of historical packets to state transitions owned by an instance. Each transition has a previous state and one distinct next state. Anonymous WGSL kernels and a consumer's complete pass list get the same facility. No crab, contact verb, skeleton solver or gait phase belongs in the loader. The types above can disappear into packed rows when the compiler has consumed their meaning. [Loft philosophy](../../docs/PHILOSOPHY.md) · [Bundle types](../../rust/crates/loft-ir/src/bundle.rs)

Record public functions over row indices, their data, live inputs, and references from skinning or procedural geometry to those rows. Bind anatomy, units and feature lookups once. Distinguish a value known at build, one evaluated at load, one depending on an integer tick and one depending on presentation time. Classify from dependencies and declared access to the future. Fold a fixed rest frame, share a lookup in two legs, inline a cheap formula used once, keep a repeated leg law as one loop or function, solve known shot coefficients at build, and place a needed history transition on the GPU. Eliminate unused evidence computation from a normal draw. A frame cache for live poses needs the input history identity. Hashing only time and the newest input will return the wrong crab.

One group can keep a small contact calculation local. Use a 64-thread group per independent animal for the eight-pad recipe. It loads anchors, selects the permitted release candidate, tests subsets in parallel, reduces feasible extrema in a stable tree, broadcasts the winning load vectors and writes the state. All threads reach the same workgroup barriers, including lanes with no candidate. A contact between two independently simulated creatures requires a joint group or a materialized intermediate and a subsequent dispatch. An inter-workgroup write followed by an unsynchronised read would introduce a timing-dependent footfall. Frame placement reads the final state, evaluates the fractional span, applies reach limits and writes the joint frames once. Sweeps, claws, shadows and an optional observer read these same frames. Never run the stepper per vertex.

A useful compiler optimisation here is deleting history for an entire *output slice*. The authored heron's body and the reactive heron's bill share a neck joint. Mark the outputs demanded by a particular draw or check, trace back their time dependencies, and keep a tick buffer only where a previous value can affect one of those outputs. Removing gaze from a body-only shadow can then remove the read of tracking state from that shadow. It cannot remove a tracker whose state a later visible frame needs. Visibility by itself never suspends a live animal. A closed, seekable span can skip frames. A live animal can skip only with an explicit state advance that is equivalent for the skipped inputs. Publish the dependency cut, scheduled kernels, source sites, resources and generated WGSL through the existing inspection path. That is also how an author finds the one expression turning a direct shot into a recurrence. [Compiler scheduling](../../rust/crates/loft-core/src/schedule.rs) · [Recorder declarations](../../src/record/decls.ts)

There is a concrete port issue before any fusion. The Rust compiler orders Load, Steps and Frame. The single-instance GPU host currently encodes frame kernels before live steps, and fills catch-up step uniforms with current inputs. Its shared-instance entry point refuses `Keep::Latest`. The present frame key omits history. Replaying a reversal or reading the newly landed ankle from a frame kernel needs all three boundaries fixed. Start with one finite 480-tick instance and its immutable tick packets. Encode steps in time order, then any dependent camera/pose kernels, then draws. Preserve independent pre-step work only if dependencies permit it. Make ping-pong parity follow the actual state across multiple submissions. Two views of one tick advance the same instance once. Bind previous and next resources in distinct read/write scopes. The host owns device, queue submission and targets. Keel keeps authoritative hit decisions and needs zero per-frame motion readbacks. [GPU host](../../rust/crates/loft-gpu/src/lib.rs) · [Frame identity](../../rust/crates/loft-ir/src/identity.rs) · [Keel contract](../../docs/keel.md)

A session appends input changes and ordered events, addressed by instance, tick and sequence. Latch a level until replaced; consume each edge once. Duplicate IDs with different payloads are invalid. A late edge in a recording forks history from a retained checkpoint, or gets a named rejection if the prefix is unavailable. Checkpoints contain the complete law state, last level values, stream cursors, rig/law identity, tick origin and input-prefix hash. History identity includes inputs that influence surfaces and targets. A backward seek restores the nearest compatible state and replays its suffix. A host choosing to retain only the latest state can still run indefinitely; asking for a past state without its inputs returns the missing range. Use wide integer time on the host and local bounded tick offsets in shader spans. Dispatches consume non-overlapping packet ranges even after a presentation gap. Ring overflow is backpressure or an explicit host reset with new identity. These choices make same-device replay testable. Cross-backend f32 arithmetic near a decision boundary can change which foot wins.

Analysis follows the final feature. For a rigid pad let the margin be `ε − length(inverse(deck(t)) * drawnPad(t) − localAnchor)`. Specify the feature, units, interval, support generation and which inputs varied. A free pad exits the interval with a named release; record an unplanned release as a failed *requested duration*, even if its shortened stance looks perfect. Report peak residual and time on the named trace. An offline enclosure can yield `Holds`, `Fails` or `Undecided` with the existing scope. A sampled check prints its count and worst sample; preserve the distinction in the UI. A solver stores `Unproved` until checked. A raw WGSL law without a suitable twin can still return a GPU measurement. An interval request identifies the unsupported operation. A GPU skin displacement that moves the actual pad must supply its final attachment evaluator or forgo the claim about the skin. A stale proof cannot survive a different dependency hash. [Rust verdict and scope](../../rust/crates/loft-core/src/check.rs) · [Build versus check](../../CORE.md)

The replacement path has four useful stopping places. Edit the one rank expression. Replace the contact law with a function receiving pad features, commands and previous state and returning next state and forces. Replace the recipe with an entire different mapping to joint frames, as with the eel. Finally, emit your own compute and draw modules, resource declarations and ordered passes and use the same wgpu host. The consumer can supply an external pose buffer plus its binding and synchronisation contract on a shared device, or own the surface deformation and draw too. There is no requirement to fabricate legs for a ribbon or to run a CPU interpreter for a raw GPU pass. Mark the latter's checks by the evidence it actually supplies. The runtime must be small enough that bypassing the entire default motion library still leaves an understandable program.

## The bill, before running it

Numbers in this section are design predictions on the project's M4 Metal target at f32. The existing 512² scramble measured 0.59 ms whole-frame best of 60 for playback and 3.1 ms GPU for a 480-tick bake, with 15.7 ms bake wall time and 309 ms pipeline setup separately. Its stores and algorithms differ. Those numbers justify measuring setup and dispatch latency separately. They supply no live p95. CORE records 232/295 Rust corpus loads and zero corpus draws from the scene library at this revision. [Experiment at `88e2fc5`](../../../_wt/loft-motion/agent_notes/2026-11-19_motion.md) · [Port status](../../CORE.md)

**Work.** Eight attached pads have 56 triples, 28 pairs and eight singletons, or 92 candidate tests per tick. Carrying both extrema for one nominated foot costs one enumeration. A 60 Hz picture covering two 120 Hz ticks evaluates at most 184 candidates on the ordinary fully supported path. A projected exterior pressure and a one-time retry after failed IK can each require another evaluation. Allow an exterior projection on each of the two attempts, at most four enumerations per tick, 736 candidates per two-tick picture. Bound the loop at that point. At 35–65 scalar operations per candidate, 184 candidates cost about 6.4k–12.0k scalar operations. Budget another 3k–7k for reductions, eight-leg controller and three-link placement including terminal directions, plus roughly 40–100 square roots, reciprocals or trig evaluations across the ordinary frame. These are budgets for emitted code and dynamic work. Instrument active subset counts and early exits. An f32 multiply-add counts as two scalar operations. A square root and a workgroup barrier have separate costs.

The reusable idea is the load query. The eight-pad enumeration is a small GPU implementation. At 32 pads the very same search costs 5,488 candidates per tick, almost sixty times the eight-pad case. A crowd of caterpillars justifies a specialised linear program, spatial contact grouping with declared error, or a different law. Publish that choice at the call site or in a recipe with an explicit supported size. Swapping in a faster implementation requires agreement on feasible examples, marginal classifications, pictures and costs. Silent truncation to eight would give a novel creature the wrong body.

**Bytes.** Use the experimental crab's three-segment legs. Store one root and four frames per leg including the hip and distal frame, 33 frames total. A posed rigid frame takes 32 B, a padded translation and quaternion. The claws and face in this fixture are drawn relative to the root. Here is the working layout for one active animal. Sizes are logical payload before device allocation and dynamic-offset alignment.

| Allocation | Derivation | Bytes |
|---|---|---:|
| Shared rig and candidate table | 64 B header, 33 × 64 B rest records, 8 × 32 B pad descriptors, 92 × 4 B subset indices | 2,800 |
| One live state | Four 16 B root lanes and five 16 B lanes per foot | 704 |
| Two writable states and presented frames | 2 × 704 + 33 × 32 | 2,464 |
| Five independently posed animals | 5 × 2,464 plus one shared rig | 15,120 |
| Full four-second state bake | 481 × 704 including tick zero | 338,624 |
| One minute of dense velocity/deck input | 7,200 × 32 B | 230,400 |
| Checkpoint state payload at half-second spacing | 121 × 704 including tick zero | 85,184 |

The root lanes hold planar centre and velocity, acceleration and shell attitude, their presentation controls, and mode/selected pad/tick metadata. A foot's five lanes hold the attached point and surface slot, swing start and tick, incoming velocity and end tick, destination and surface slot, and mode/load/filtered bend/bend rate. The instance's surface table owns generation mappings. The small nominal crab takes one level packet per tick; hits use extra variable-length event records. Identity hashes, cursor and level snapshots, GPU packet rings, padded allocations, per-view uniforms, mesh, framebuffer and driver pipeline memory are separate. Eight aligned 32 B packets per live crab cost 256 B in a storage ring. Sliding the ring must respect completion of in-flight submissions. Step state read/write twice plus two packets and one pose read/write give 4,640 logical B per ordinary frame per crab before shared rig loads and vertex fetches. A 64-thread group can hold old/new state, anchors and two 16-byte extrema records per lane in under 4 KiB of workgroup memory with a packed implementation. Record actual padding, spill and storage sizes.

**Time.** Start with two step dispatches and one pose dispatch in the normal frame. For the ordinary fully supported path, budget 18 µs dispatch latency, 16 µs per concurrently resident set of up to eight animal groups for each tick, and 13 µs for placement. This deliberately crude occupancy hypothesis gives `3 × 18 + (2 × 16 + 13) × ceil(N/8)` µs, or 99, 99 and 414 µs for one, five and 64 animals. For six ticks on one animal it gives 235 µs. The 99 µs estimate is below two timer ticks. Score upper envelopes of 0.197 ms p95 for one, 0.262 ms for five and 0.655 ms for 64 on the ordinary path. The six-tick one-animal envelope is 0.393 ms. Five and 64 assume batched dispatch over independent state; phase 2 tests that assumption. Use 256 repeated independent groups to resolve a slope and timed empty dispatches to estimate the intercept. Dividing a batch time by 256 estimates throughput. A player waiting on one creature experiences the intercept as well. For close-ups compare the entire frame's first-to-last timestamp with a recorded-pose control drawing exactly the same geometry.

The first crab needs two motion pipelines (step and placement) per layout/precision, shared by all creatures of that kind. Predict ≤1 s cold Rust build for the slice, ≤2 s for a training build including validation, 0.1–0.45 s cold motion pipeline setup on Metal and ≤30 ms incremental warm load with a compatible cache. Record total cold scene load beside the incremental motion cost; predict ≤1.5 s for the small complete slice. Five instances add four payload initialisations and zero motion shader modules. The immutable rig upload is 2,800 B per distinct rig, so content bytes still cost bandwidth. No live bake is necessary. For optional dense offline replay, recording 480 transitions with one separate dispatch per tick gives a rough GPU estimate of 480 × 34 µs = 16.32 ms plus history writes, with a 25 ms GPU and 80 ms warm wall target. A later locally batched bake can amortise submission overhead. With half-second checkpoints an arbitrary integral seek replays at most 59 ticks. Include restore/upload and replay time in seek latency.

The other cases buy different work. Four cubic channels across twelve fixed spans need 768 B of scalar float coefficients and 16 coefficient reads at one sample. One ballistic flight needs its initial conditions and gravity. Budget 0.25–0.8 s CPU to solve the free heron channels on a cold build, a 2 s cap, and one placement dispatch plus at most two gaze ticks per 60 Hz presentation with motion p95 ≤0.197 ms. This needs measurement given the branch's 7.6 s six-channel solve. For a boss that actually needs a 24-link local XPBD solver with 48 scalar rows, eight sweeps and two ticks, budget 768 row projections, roughly 60k–160k scalar operations, 3 KiB dual state at 64 B/link, plus 2–5 KiB for drives, contacts and multipliers. Predict motion p95 ≤0.393 ms for one on simple terrain. For a 60 Hz imported ten-second take on 33 joints, 601 dense poses at 32 B/joint take 634,656 B before compression; sample adjacent keys and correct only marked chains. The equal-spacing eel pays a sine per posed joint and zero recurrent storage. These are different bills for different deliverables.

## What we inherit and what we have to invent

Spore supplies late binding to anatomy. Overgrowth gives the animator an extreme worth preserving. Rain World shows how little simulated structure can support a convincing creature. Disney timing makes a delayed ankle fold or a completely still face intelligible. Keep all four as choices in public recipes. Let a limb graduate from drawn decoration to a contact that changes the body's acceleration by editing its feature binding and support law. [Spore](https://enriquevelasco.com/papers/hecker-realtime-retargeting-2008) · [Overgrowth](https://github.com/WolfireGames/overgrowth/blob/main/Data/Scripts/aschar.as) · [Rain World](https://unity.com/blog/exploring-procedural-design-rain-world) · [Lasseter on animation principles](https://graphics.cs.cmu.edu/nsp/course/15-464/Fall05/papers/lasseter.pdf)

Euphoria contributes state-dependent reactions, Control Rig editable IK, MuJoCo explicit contact dynamics, and Jakobsen, PBD and XPBD inexpensive constraint projection. Their rightful place is wherever that particular solver improves the picture at its measured cost. For Honor's search, DeepMimic imitation, AMP's style prior, ASE's repertoire, SuperTrack's physical tracker and MotionBricks' generated phrases can all supply performance or drives. Preserve the correlations in a good take. Account for capture, correspondence, inference and training when changing bodies. A generated hand closing on a partner needs the partner's geometry and interaction to survive the edit. [Prior-art analysis and primary references](motion_astra_step1.md) · [Epic](https://dev.epicgames.com/documentation/unreal-engine/control-rig-full-body-ik-in-unreal-engine) · [MuJoCo](https://mujoco.readthedocs.io/en/stable/computation/) · [XPBD](https://matthias-research.github.io/pages/publications/XPBD.pdf) · [DeepMimic](https://xbpeng.github.io/projects/DeepMimic/) · [MotionBricks](https://github.com/NVlabs/motionbricks)

Our bet is the combination of three capabilities. Expose the feasible freedom in a contact as a directable quantity with a constructive answer. Compile that choice, a keyed phrase and a causal controller independently down to the few transitions their outputs require. Follow a commitment through all replacements to the same feature the audience sees. The novelty claim is about that authoring and execution contract. Its mathematics has predecessors. The state-of-the-art ambition is falsifiable on the same task: match a handwritten GPU kernel within one timing quantum at five animals, change a law or body without engine edits, and deliver two recognisably different weight transfers with identical footprints and root. Compare price, edit time and pictures together before claiming a win.

## Landing sequence

**Phase 1. Ship one live weight transfer from a TypeScript scene to a Rust-drawn crab.** Bring over only the look fixture's geometry, camera and rest data. Use the explicit draw/pass/camera recording interface for this scene while general scene recording catches up. In the public motion library implement one planar support function, a retained contact, one Hermite swing with inherited starting velocity, and load-driven ankle placement. Use `src/record` for recording, `loft-ir` for validation/layout declarations, `loft-core` for lowering, and `loft-gpu` for per-tick packets and dependency order. A finite single-instance history suffices. Produce a real colour/depth draw through `loft-draw`. Use existing V1 raw kernels and rows where their binary semantics suffice. If extending persisted timing/bindings is necessary, version the format and lower old bundles with their original input semantics. Keep new mechanics out of the frozen TS engine. [Rust draw lowering](../../rust/crates/loft-core/src/draw/mod.rs) · [Rust port plan](../../docs/proposals/2026-11-19_lean_rust_loft.md)

| Score before promotion | Prediction and pass condition |
|---|---|
| Picture | Two four-second loops with the same crab, root, camera, feet, timing and materials, varying the loaded leg. Also compare against neutral bend with identical inputs. In blinded, randomised order at game distance, at least 4/5 viewers identify the chosen side and prefer the weighted reversal to neutral in each of two views. Heat maps appear only afterwards. |
| Force and release | Four-pad calibration above, 10 kg, height 0.30 m, μ=0.7. Error ≤0.0001 per share and ≤0.1 mm in reconstructed pressure point. Request lift at tick 120; stationary centre (0.15,0) requires refusal through tick 131. At tick 132 request +5.886 m/s² along x. Then q moves to −0.03 m, about 16.6 mm inside the exclusion triangle's diagonal. With a 5 mm inset and immediate unloading in this fixture, first legal lift is tick 132. Zero earlier releases. |
| Pose and history | Two legal earlier landings 0.20 m apart still differ by 0.20 ±0.001 m during shared stance. Move the deck uniformly at 0.5 m/s for one second. The held pad travels 0.50 ±0.001 m in world coordinates. Peak deck-relative error ≤1 mm at 120 Hz plus three fractional samples per interval. A target 0.20 m beyond extension yields zero reported successful holds. |
| Execution | Four-second replay at 30, 60 and 120 Hz plus a six-tick stall yields identical state bits on the reference backend at every common tick, zero duplicate edges and no stale first-frame toe. Changed history produces a different frame identity. Predict two motion modules, three ordinary dispatches, 2,464 B writable state/pose per crab, motion p95 ≤0.197 ms, complete 512² × 16 draw ≤50 ms per frame, CPU build ≤2 s. |
| Instrument | A/B interleaved over 240 presentations after 60 warmups, same input trace; compare against direct WGSL and cached-pose draw. Time live steps through placement and the whole frame with separate spans resolved after completion. Report p50/p95, timestamp quantum, cold and cached setup, adapter, res/memory and actual draw count. Score samples at catches and switches separately. |

**Phase 2. Earn the default walk.** Add acceleration-limited recovery, timestamped streaming, checkpoint seeking and shared pipelines for five independent crabs, then 64. Change only the six-legged rig data and run the unannounced reversal with one short front leg. Require zero engine or species-specific controller edits, two different histories preserved, and a fresh feasible swing within 0.20 s on a prepared reversal with room to step. Give a second author fifteen minutes to change the release ranking and rerun the shot. Publish the edited source and the timing against a handwritten equivalent.

**Phase 3. Earn hybrid performance and interchange.** Ship the heron's directed ballistic hop, live fish and imported grasp, then the boss's interrupted guard. Gate the bird on touchdown at tick 144, sampled neck length within 1.6 mm, reachable eye error below 2°, and zero rear-target flips. Gate the boss on two distinct impulses 15 ticks apart, ≤1 mm palm error through the required hold, and recovery that changes when the first support slips. Compare the real-time loops in both cameras. Promote only laws whose pictures win; preserve a rejected solver as a replaceable experiment. The outcome worth shipping is an animator who can see the wrong foot take the weight, open its law, and correct that one decision.
