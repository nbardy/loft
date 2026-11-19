# Loft

A scene imports from `../src/index.ts` and default-exports `scene({ ground, things, style, sun, cameras })`; `ground`
is `plane` (y = 0), `nothing` or `sea`. y is up. Write the maths of a function (a spine, a girth, paint, a field) as
WGSL in `wgsl` text and the structure (which things, where, cameras, checks) in TypeScript: both build one expression
graph that draws on the GPU and evaluates on the CPU, so checks read what is drawn. `time` is an argument, in seconds,
so any frame draws alone; `period` says when every motion repeats. A swept body's m runs 0 (root) to 1 (tip) and v 0
to 1 round it; a surface's u and v run 0 to 1; paint is a function of them. Give every scene two cameras: the whole
with room round it, and the focus. `strand`, `spiralTube`, `sheetOn`, `ropes` and `impostors` return things: put them
in `things`. `coil`, `fan`, `sheet`, `lathe`, `loft` and `extrude` return a bare surface: draw it,
`draw.surface(name, surface, { paint, grid })`.
Light is the sun (`sun: { toward, amount }` dims or colours it; amount 0 is a moonless night) and up to seven `lights`,
each a function of a world point returning `{ toward, amount, distance?, size? }`, such as `lamp(at, { colour, reach })`;
a lamp casts a shadow given shadow: true, a glow lights nothing by itself. Paint may return a struct: `base` (the colour)
and words of your own, of which the styles know gloss (0 to 1), metal and emits (linear light), and `pbr` also rough
and normal (a bump: `gradient` of a height); a style notes the words it does not read. Its `scatter` answers light
from a direction, its `shade` changes the finished colour.
Your own triangles draw with `draw.mesh`, a solid you can intersect with `draw.traced`. `pass` is another camera's
picture (a mirror) for an expression to read. `--samples 64 --shutter 0.05 --aperture 0.1` makes a still with motion
blur and depth of field. Meshes come out (the bake command writes glTF), none go in. `loft` is `bun src/cli/main.ts`.

```ts run
// examples/reed.ts
import { plane, scene, strand, toon, wgsl } from "../src/index.ts";

const reed = wgsl`
fn spine(m: f32) -> vec3f { return vec3(m * m * sin(time) * 0.6, m * 4.0, 0.0); }  // a point per m; the tip sways
fn girth(m: f32) -> f32 { return 0.25 - 0.2 * m; }`;

export default scene({
  ground: plane,
  things: [strand({ name: "reed", spine: reed.spine, girth: reed.girth, tips: ["open", "point"], paint: [0.85, 0.55, 0.2] })],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
  cameras: {
    whole: { kind: "perspective", eye: [6, 4, 8], target: [0, 2, 0], fovY: 0.6 },
    tip: { kind: "perspective", eye: [2.5, 4.5, 3], target: [0, 3.6, 0], fovY: 0.5 },
  },
});
```

```sh
loft render examples/reed.ts --out out/reed.webp --views whole,tip --size 640x400
loft frames examples/reed.ts --out out/reed_t.webp --times 0,0.8,1.6 --view whole --size 480x300
```

# WGSL text

`wgsl` returns a function per fn, taking expressions or numbers; it fills any slot a TypeScript function fills. A slot
may pass more arguments than a fn declares (paint passes u, v, at); a returned struct fills a record slot by field
name. In scope: the module's fns, structs and consts, `time`, the names below, and holes: `${x}` brings in a `param`,
an `input`, a solved curve, a TypeScript function (called in the text) or a number. The subset that checks, `solve`
and `gradient` read: f32, vec2f..vec4f, bool, mat2x2f, mat3x3f, constant arrays, structs; let, var, assignment,
swizzle writes, if/else, for over a counter with break and continue, early return. As in WGSL, round ties to even, %
truncates (mod floors) and 1 is an integer (write 1.0). A fn using anything else (integers, bit operations, textures,
derivatives, while) is raw: it renders, and checks refuse it by name and line. Lower down, a module draws
(`draw.shader`: vs and fs entries) or computes (`compute`), reading camera_ray(px), frame and holes.
`loft show examples/reed.ts --thing reed` prints the WGSL the GPU runs, your lines marked. Call form (`add`, `mul`,
`vec3`) is for what TypeScript does: mapping over parts, helpers.
Builtins (WGSL's meaning): abs, sign, floor, ceil, fract, trunc, round, sqrt, inverseSqrt, exp, exp2, log, log2, sin, cos, tan, asin, acos, atan, sinh, cosh, tanh, saturate, normalize, min, max, pow, atan2, step, mix, clamp, smoothstep, length, distance, dot, cross, fma, radians, degrees, reflect, faceForward, refract, select, all, any; vec2..vec4 and f32() construct.
Prelude (the library's functions, as in the API map): mod, pick, hash2, hash3, hash22, hash33, noise2, noise3, cellular2, cellular3, remap, remapClamped, polar, cartesian, rotate2, fresnel, smin, smax, luminance, hsv, sdCircle, sdBox, sdSegment, sdCapsule, sdRing, sdTriangleIsosceles, repeatAngle, stroke, coverage, sdSphere, sdBox3, sdRoundBox, sdCapsule3, sdRoundCone, sdTorus, sdCylinder, sdPlane, sdEllipsoid, union, subtract, intersect, smoothUnion, smoothSubtract, smoothIntersect, shell, elongate, revolve, extrude, fold2, fold3; structs PaintAt { world, normal, copy }, Ray { o, d, t0, t1 }; gradient(h, p); time.

# Craft (long form: docs/craft.md)

**Range in shape: change the construction, not the number.** A viewer recognises a shape's law, not its numbers: ten
spiral tubes with different curls are one shape. Make three to five far-apart candidates before refining any, small on
one sheet; write the weakest thing about each, pick one, push it further than feels safe. Each must change at least one
of: the spine (`through`, `wave`, `spiralLine`, a heading written in m with `tabulate`); the section (`aspect`,
`squareness`, `twist` as functions of m); the topology (tube `strand`, sheet `sheet` or `fan`, turned `lathe`, folded
`draw.field` with `fold3`, copies `impostors`); the construction (`extrude`, `loft`, `grow`, `tree`, relief
`draw.relief`). Otherwise it is a retune. Read a component's limits (`loft api spiralTube`: its law, what it cannot
draw, its relatives), then go one level down and write the rest. Examples and presets pull: take how a part attaches
from them, never a proportion; make first the candidate you would not have made.

**Set the target in words first;** there are no reference pictures, and without a target the first render becomes it.
In the file's top comment: the silhouette in one sentence, the three largest forms and their proportions, what makes
it unlike the nearest example, two or three inks and one accent, the beats of its motion, and predictions a render can
be scored against (a part's pixels from a named camera, what is in front of what, a beat's length).

**Range in style: compose parts, write a part.** Presets (`toon`, `woodblock`, `biolume`, `papercraft`) are four looks,
not range. Paint in body coordinates (`palette.countershade`, `palette.bands`, `palette.spot`, `tile`), choose which
parts carry ink, make sky and ground part of the picture. A `Style` is a plain value: spread one and replace `shade`;
a `post` pass changes the whole frame. Say what a thing is with a word in its paint or a `scatter` before you wrap the
style; a `shade` that ignores the style's colour drops out of every restyle. Light a night by what is in it: a dim sun, and a `lamp`
at the same point as the glow that shows it; check the subject is lit (`litBy`).

**Shapes that read.** Large forms first, judged in flat paint; silhouette before detail; proportion and carriage are
identity; detail in clusters, not spread evenly; contrast with what is behind decides whether a feature exists.

**Motion that reads.** Three beats, wind-up several times the act (`beats`); the wrong way first and past the mark
after (`anticipated`, `overshot`, `response`); loose parts lag (`dragged`, `swing`); nothing still or in step (`alive`,
`uneven`); a pose struck and held (`struck`); arcs and weight (`arc`, `squash`, `impact`, `footing`, `carriage`,
`gait`); temperament is timing (`dart`, `pendulum`); a warning before an action (`attack`, `telegraph`); then push it
(`exaggerated`). State only for what truly steps: a kick is a `jolt`, a trail is the thing at earlier times; `recur`
a grid, a flock, contact.

**Predict, then look.** Expressions run on the CPU (`compileJs`), so predict sizes and positions first and keep them as
exported `checks` (`minScreenSize`, `staysInView`). Look at your render small, from its camera, as a silhouette and
across the motion (frames, above); change the largest thing first.

# API map (`loft api` for all of it; `loft api <name>` for one entry whole)

`name(args)`; `{a, b?}` a spec's fields (`?` optional), `f(m)` a field or argument that is a function. `~` marks a module with one user so far: cut it down when your subject fights it.

## Core
operators: abs, and, atan2, clamp, cos, cross, distance, div, dot, eq, exp, f, floor, fract, ge, gt, le, length, lift, log, lt, mix, mod, ne, neg, normalize, not, or, pow, round, saturate, select, sign, sin, smoothstep, sqrt, step, sub, tan, vec2, vec3, vec4
expr: TAU; add; compileJs(roots, rows?); evaluate(e, env?); fetch(table, index, lane); input(given, ty, def); iterate(count, init, step(state, i), until?(state)); lib(fn, ...args); max; min; mul; namespaceSeed(); param(name, def, range?); pick(i, values); row(r, x); swizzle(a, sel); time; variable(name, ty)
text: wgsl(strings, ...holes)
graph_fn: graphFn(name, params, body(...a))
grad: gradient(e, p)
curve: keys(points, options?); ramp(colours, label?); through(m, count, point(j)); bspline(t, over, values); wave(m, spec); retarded(t, m, speed) [signal delay]; response(t, spec); pulse(t, t0, t1, ease?); ease(x, a, b)
map/types: NO_SKELETON
chart: carry(chart, f); compose(outer, inner); on(host, c); place(shape, chart); points(count, spec(i)); posed(count, spec(i)); posedList(given, specs); shifted(chart, by); size.even(k); size.axes(k); sized(chart, k); sizeLengths(z); turn.about(axis, angle); turn.axes(x, y); turn.{none}; inFrame(g, d); world
compute: compute(name, spec)
recur: recur(spec)
fold: fold2.repeatPolar(count); fold2.logPolar(); fold2.spiral({arms, cells}); fold3.repeatPolar(count, axis?); fold3.screw(pitch, count, axis?); fold3.helix(angle, rise, period, axis?); fold3.logSphere(); fold3.twist(rate, radius); fold3.bend(rate, radius)
warp: warp.paint(paint(u, v, at), w(p)); warp.field(field(p), w(p)); warp.surface(s, w(p)); warp.curve(c, w(u)); warp.points(p, count, w(i)); tile(w, motif(cell))
table: tabulate(given, fn(x), n, when)
map/surface: extrude(curve, by); framed(s, at); framedPose(s, at); lathe(profile(u), ends); loft(curves); mapNormal(map(u, v)); sheet(root, edge); surface(map(u, v), topology); thicken(s, half)
sweep: sweep(name, spine(m), section, transport)
transport: rotateAbout(v, axis, angle)
draw: draw.surface(name, surface, {paint(u, v, at), grid, sides, outline}); draw.glazed(name, surface, {colour, grid, phase, sceneDepth, detail?}); draw.curve(name, curve, {width(u), paint(u, v, at), segments}); draw.posed(name, poses, thing); draw.field(name, field(p), {bounds, paint(hit), steps, outline, schedule, safety?}); draw.relief(name, surface, {height(at), hMax, fold(p), paint(at), steps, grid, outline}); draw.mesh(name, mesh, {at?(row, i), paint, sides, outline, skeletons?}); draw.traced(name, {bound, hit(ray, copy), paint, outline, skeletons?}); draw.copies(name, {kind, copies, paint(hit), outline, minPixels, keepLevels, inkBack, carry?}); draw.density(name, medium, {bounds, sun?, sky?, forward?, shadowing?, detail?, haze?}); draw.points(name, count, speck(index), o?); draw.volume(name, density(p), {bounds, steps, albedo?(p), emission?(p), g?, shadow?}); draw.shader(name, module, o)
field: fieldPixel
draw/surface: spacing.even(count); spacing.ends(count, crowd?); spacing.from(count, start)
scene: scene({ground, things, style, compute?, sun, lights?, cameras?, outline?, period?})
ground: plane; nothing
compile: compile(scene)
context: drawSpec(d)
bind: emitFn(slots, name, args, out, roots, ret); emitShaderFn(slots, f)
trace: bound.box(min, max); bound.sphere(centre, radius); bound.capsule(a, b, radius)
march: march(field(p), {within, steps, safety?})
cast: castRay(ray)
impostors: impostors(name, {cell, shape(cell), bounds, paint(hit), cells, outline})
height: offset(patch, u, v, a); segmentDistance(patch, u, v, a, b); pathDistance(patch, u, v, path); profile(p, r); bump(patch, {at, radius, height, profile}); ridge(patch, path, {width, height, profile}); crease(patch, path, {width, depth, sharpness}); weight(patch, r, u, v); flatten(patch, h(u, v), region, level); inflate(patch, h(u, v), region, amount); level(h); sum(...hs); smoothMax(k, ...hs); clampHeight(h(u, v), lo, hi); scaled(h(u, v), by(u, v))
sculpt: sculptLayer(name, samples, size, {wrap})
displace: displace(s, height(u, v))
light: lamp(at, {colour, reach, size?, shadow?}); movable(dir, name?)
pass: pass({name, size})
noise: noise2(p)

## Shapes
strand: strand({name, spine(m), stations?, girth, section?, twist?, tips?, paint, detail?, up?, level?, bulge?})
geom/sea: sea({swell, deform?, reach?, foam?})
fan: fan({rays, sag, scallop, slack?}); rayOf(body, v?); catenary(f, k)
spiral: spiralTube({name, length, curl, taper, girth, out, toward, twist?, paint, detail?}); spiralLine(spec); whorl(u, turns) [coil turn]; ridgePhase(u, turns, perTurn); coil(spec); coilAt(law); coilPlan(spec); COIL_TOPOLOGY
symmetry: mirror(part, name); array(part, count, placement(i)); reflect(p)
whirlpool: whirlpool({at, radius, spin, depth, drift, throat?, arms?})
afloat: afloat(w, {at, heading, length, beam, lift?, wind?}); afloatFrame(w, spec)
rig: rigid(g)
medium: bank(b); blob({centre, radii, density, colour}); layer({height, thickness, density, colour}); medium(name, {puffs?, banks?}); unionBox(boxes)
glow: flare(name, at, o); glow(name, curve(u), o); glows(name, s, {width, halo, colour, core, brightness(i), veil}); strokes(name, count, segments, point(u, i), taper?(u, i))
crest: sheetOn({on, from, to, height(x), v?, paint, sag?, rays?, rake?, flutter?, name, rows?}); sheetOnEdges(spec)
limb: chain({root, target, lengths, bend, foot?}); polyline(m, points, knots); unit(v)
lines: ropes(given, ropes, {width, colour, segments?})

## Motion
gait: gait({feet, phase, course(s), stride, pace, duty, lift, minStep?}); course.line(origin, heading, facing?); course.arc(origin, heading, radius, facing?)
motion: lookAt({from, target, yaw, pitch}); breathe({rate, depth, phase?, t?}); beat(r, {rate, amplitude, lag, bias?, down?, t?}); strike(s, {anticipation, follow, timing?})
lively: looping(seconds, build()); alive(t, amount, seed?, hz?, period?); anticipated(u, back?); arc(a, b, u, lift_, up?); beats(since, b); dragged(t, along, lag); echo(f(t)); exaggerated(x, rest, by); impact(after, depth?, hz?, damping?); overshot(u, back?); slowIn(u, power?); slowInOut(u, power?); slowOut(u, power?); squash(along); stretchOf(speed, per?, most?); swing(after, amount, hz?, damping?); uneven(part, spread?)
~stance: carriage(stances); footing({ground, homes, t?, replant?(t), stamps?, walk}); headingIn(ground, v); replants({homes, loop, busy(t), idleBelow, every, step, reach, seed}); towardIn(ground, p); carriedFrame(ground, c)
~placing: pose({at, heading, pitch, heel}); poseFrame(p); placedBy(p, {standIn, host}); blendFrames(a, b, w); turnedAbout(g, axis, angle); hosted(p); STAND_IN
~bend: bent({root, along, length, lean?, sharp?}); bentTip(o)
~pendulum: pendulum({length, damping}); sway(amplitude, period, phase?); jolt(after, speed)
~restless: dart(t, {every, period, seed, uneven?, past?}); chosen(hold, seed); tremor(t, hz, seed?)
~expression: cast(rest, named); score(cast_, layersAt(t)); show(name, weight); only(name, channels, weight); added(by, weight?); during(clock, {from, to?, rise, fall?})
~struck: struck(clock, {from, snap, hold, release, recoil?}); holdsStill(score, poses, {period, within, rate?})
~solve: solve(spec); spline(n, from?); equal(a, b, within?)

## Actions
hit: hit.capsule({a, b, radius}); hit.disc({centre, radius}); hit.arc({centre, inner, outer, from, to}); hitSdf(h, p); hitPatch(h, u, v, pad); hitContour(h, p, spacing); hitInradius(h)
attack: attack(spec); animate(rest(m), attacks, build(spine)); part(name, body, {from?, to?, tip}); once(at); every(period, first); hold(o); ring(o); INSTANT; STILL
telegraph: telegraph(attack, o?); DANGER
smear: smear(strike, {over, samples, speed?, opacity?})
wake: wake(body, {over, samples?, spread?, colour?})
design: design(spec); parseVariantAsk(text)
~emote: emote({name, arm, accents, lead, hold, fall, snap, offbeat, shuffle, settle})
~routine: routine({beat, length, centre, places, manner, size, score})
~ring: ringWave({centre, radius, since(t), speed, height, length, reach, hold, drag})
~lineup: lineup({members, water, loop})

## Checks
check: clearance(body, target); landsInside(strike, o?); gripReaches(attack, fingers, {middle, gap, rate?}); releasesAt(attack, {speed, place}); throwLands(attack, {least, speed, place, rate?}); fliesClear(attack, targets, {size, margin?, rate?}); letsGoClean(attack, fingers, thing, {seconds, depth, rate?}); shoves(ringing, place, {least, speed, place}); frontFirst(ringing, places, o?); warned(attack); tolerance.units(units); tolerance.share(share); minScreenSize(feature, camera, pixels, o?); litBy(light, feature, {least, from?, to?, at?}); budget({triangles}); runChecks(scene, checks, o?); formatChecks(vs); expressionsRead(score, marks, o); spansOnScreen(what, mark, camera, pixels, {at, size?}); staysInView(points, blockers, o); limbsReach(limbs, {period, slack, fold, rate?})
ranged: always(range, condition, o?); eventually(range, condition, o?)

## Styles
style: palette.countershade(v, {back, belly}); palette.bands(m, v, under, {colour, count, width, from?, to?}); palette.spot(m, v, under, {at, radius, colour, aspect?}); palette.clamp(c); dorsalness(v)
style: STYLES.toon(); STYLES.pbr(); STYLES.{woodblock, biolume, papercraft, sketch, neon}; STYLE_NAMES; styleNamed(name); withStyle(scene, style); withLight(scene, name)
style/light: light.core(edge?); light.highlight(o?); light.cast(o?); light.waterline(o?); light.overlap(o?); light.bokashi(o?); light.hatch(o?); light.rim(o?); light.block(o); light.recipe(r?); light.{key, edge}; LIGHT.{flat, twoTone, threeTone, bokashi, hatched, contact, rim, cel, sunset}; lit(r, ink, ctx); litBodies(recipe, own, ink(ctx), after?(c, ctx)); printedBlock(p, colour, tone); pullOf(b); markOf(p, tone); blockInk(b, own); keyOf(k, ctx)
sky: skyOf(sky, ctx)
style/sea: swellLines(ctx, {perWave, width})
paint: paint.hotter(o?); paint.mottle(o?); paint.swelling(o?); paint.wandering(o?); warmed(w, c); mottled(m, c, ctx); weighted(l, c, ink, ctx); standsOut(hue, c)
post: depthOfField({focus, focalLength, fStop}); filmGrain({amount}?); paperGrain(o?); bloom(o?); reflection(o?)
shared: edgeOf(x, edge, soft, widest?); pixelsInside(ctx); rgb(c); wgslRgb(c); POST_COMMON
gloss: glossed(colour, ctx, {gloss, metal?, seen?})
pbr_parts: ggx(n_h, rough); smithGgx(n_l, n_v, rough); schlick(v_h, f0); lambert(albedo, n_l, _n_v, _v_h, _rough); skyLight({normal, view, sun}, rough, sky(dir, sun)); daylight(dir, sun); aces(c)
toon: toon(spec?)
woodblock: woodblock(spec?); print.bands(o?); print.keys(keys); print.overprint({ink, amount, value, black}); seaPalette.swell(); seaPalette.deep(o?); LOOKS.{referencePalette, engraving, engravingSunset}; REFERENCE_PAINT; WOODBLOCK_PALETTE.{black, indigo, pale, cream, vermilion}; WOODBLOCK_SEA
biolume: biolume()
papercraft: papercraft()
sketch: sketch()
neon: neon()
pbr: pbr(parts?)
noise: noise3(p); hash2(p); hash22(p); cellular2(p)
util: luminance(rgb)

## Library (import from src/lib/index.ts)
noise: hash3(p); hash33(p); fbm2(p, spec); fbm3(p, spec); cellular3(p)
util: remap(x, inLo, inHi, outLo, outHi); remapClamped(x, inLo, inHi, outLo, outHi); polar(p); cartesian(pr); rotate2(p, angle); fresnel(normal, view, power); smin(a, b, k); smax(a, b, k); hsv(h, s, v)
sdf2: sdCircle(p, r); sdBox(p, halfSize); sdSegment(p, a, b); sdCapsule(p, a, b, r); sdRing(p, r, halfWidth); sdTriangleIsosceles(p, halfWidth, height); repeatAngle(p, count); stroke(d, halfWidth); coverage(d, pixelWidth)
fold: compose(first(p), second(p))
sdf3: sdSphere(p, r); sdBox3(p, halfSize); sdRoundBox(p, halfSize, r); sdCapsule3(p, a, b, r); sdRoundCone(p, a, b, r1, r2); sdTorus(p, major, minor); sdCylinder(p, r, halfHeight); sdPlane(p, normal, offset); sdEllipsoid(p, radii); union(a, b); subtract(a, b); intersect(a, b); smoothUnion(a, b, k); smoothSubtract(a, b, k); smoothIntersect(a, b, k); round(d, r); shell(d, thickness); elongate(sdf(p), h); revolve(sdf2(p), offset); extrude(sdf2(p), halfHeight); fieldOf(fold(p), sdf(p)); nearest(folds, sdf(p)); lipschitz(field(p), {samples, bounds, seed, separation?})
grow: kifs({count, fold, scale, offset}) [folded fractal]; grow({count, motif(q, level), step(q, level)}); levels(lo, hi, x)
similar: ask(chain); gen.shift(by); gen.turn(plane, angle); gen.mirror(axis); gen.shrink(ratio); list(chain, parent); partDistance(part, q); placePart(part, fr); toWorld(fr, q); tree({part, step}); WORLD

Domain modules, left out of this map (weather, sea and throwing): `loft api <name>` prints each: lightning, rain, cloud, whitewater, foam, grab.

Types you write: F, P3, In, Drawable, Style, Check, Frame, Ray, Traced, Branch, Chart, VolumeOptions, StrandSpec, Folded, Target, Grab.

# Wrong without an error

A one-sided surface wound inside out shows its far wall or nothing (try `sides: "two"`); a camera aimed past the
subject shows sky; a subject far too small or large for its camera; a `period` that is not every motion's own jumps at the loop.
`minScreenSize` and `staysInView` catch two. Exit 6 is the environment, not the scene.

# More

`loft api` (everything); `loft api spiralTube` (one entry whole, with its limits and relatives); docs/craft.md;
docs/guide.md.

