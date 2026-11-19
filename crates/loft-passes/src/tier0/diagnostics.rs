//! Full rendered diagnostics for every implemented v1 code in E101–E503.
use crate::{Code, Sources, compile};
use std::path::Path;

#[test]
fn authored_diagnostic_snapshots() {
    let mut src = Sources::default();
    let lib = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib");
    let mut files: Vec<_> = std::fs::read_dir(lib)
        .unwrap()
        .map(|f| f.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "loft"))
        .collect();
    files.sort();
    for file in files {
        src.add(
            file.file_name().unwrap().to_str().unwrap(),
            std::fs::read_to_string(&file).unwrap(),
        );
    }
    let file = src.files.len();
    src.add("case.loft", String::new());
    for &(code, source, expected) in CASES {
        src.files[file].1 = source.into();
        let error = compile(&src, &[]).err().expect("negative fixture compiled");
        assert_eq!(error.code, code);
        assert_eq!(error.render(&src), expected, "{code}");
    }
}

const CASES: &[(Code, &str, &str)] = &[
    (
        Code::E101,
        r###"space Body
kind K @Body (pose: xf@Body>World) {
  trace(q) { let wrong = q + q; wrong.x }
  material(q,n) { ggx(rgb(0.5), 0.5, 0) }
}
build { emit K(pose: place([0,0,0]@World)) }
"###,
        r###"error[E101]: `Pt + Pt` has no meaning: points do not add
  --> case.loft:3:26
   |
  3 |   trace(q) { let wrong = q + q; wrong.x }
   |                          ^^^^^
   = fix: subtract an origin from one side (`Pt - Pt` is a Vec), or use `mix(p, q, t)`
"###,
    ),
    (
        Code::E104,
        r###"space World Local
kind K @Local (pose: xf@Local>World, r: f = 1) {
  trace(p) = sphere(p - [0,0,0]@World, 0.5)
  material(p,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit K(pose: place([0,0,0]@World)) }
"###,
        r###"error[E104]: space mismatch: `p3@Local` - `p3@World`
  --> case.loft:3:21
   |
  3 |   trace(p) = sphere(p - [0,0,0]@World, 0.5)
   |                     ^^^^^
   = fix: bring both into one space; `Local` and `World` differ
"###,
    ),
    (
        Code::E105,
        r###"space World Local
kind K @Local (pose: xf@Local>World, r: f = 1) {
  trace(p) = sphere(p - [0,0,0]@Local, 0.5)
  material(p,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit K(pose: place([0,0,0]@World)) }
fn goal(anchor: attach@World) -> target@World = anchor
"###,
        r###"error[E105]: expected target@World, found attach@World
  --> case.loft:7:49
   |
  7 | fn goal(anchor: attach@World) -> target@World = anchor
   |                                                 ^^^^^^
   = fix: supply an independent goal or an attached anchor of the declared type
"###,
    ),
    (
        Code::E106,
        r###"space Body
kind K @Body (pose: xf@Body>World) {
  trace(q) { sphere(sym - sym.pt(0,0,0),0.5) }
  map sym(point) = point |> mirror_x
  material(q,n) { ggx(rgb(0.5), 0.5, 0) }
}
build { emit K(pose: place([0,0,0]@World)) }
"###,
        r###"error[E106]: a map is not a point
  --> case.loft:3:21
   |
  3 |   trace(q) { sphere(sym - sym.pt(0,0,0),0.5) }
   |                     ^^^
   = fix: use its .p, .pt(...), .in(...) or .out(...) member
"###,
    ),
    (
        Code::E108,
        r###"space World Local
kind K @Local (pose: xf@Local>World, r: f = 1) {
  trace(p) = sphere(p - [0,0,0]@Local, 0.5)
  material(p,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit K(pose: place([0,0,0]@World)) }
law read(h: K, other: K) writes h.{r} { h.r = 1 }
"###,
        r###"error[E108]: read-only rows require a table parameter
  --> case.loft:7:16
   |
  7 | law read(h: K, other: K) writes h.{r} { h.r = 1 }
   |                ^^^^^
   = fix: write [Kind] instead of Kind
"###,
    ),
    (
        Code::E110,
        r###"kernel fill(x: m/s, dt: s) { let length: m = x + dt }
"###,
        r###"error[E110]: expected dimensions [1.0, -1.0, 0.0, 0.0], found [0.0, 1.0, 0.0, 0.0]
  --> case.loft:1:50
   |
  1 | kernel fill(x: m/s, dt: s) { let length: m = x + dt }
   |                                                  ^^
   = fix: use compatible length, time, mass and angle units
"###,
    ),
    (
        Code::E210,
        r###"space World Local
kind K @Local (pose: xf@Local>World, r: f = 1) {
  lipschitz(p) = 1.35
  trace(p) = sphere(p - [0,0,0]@Local, 0.5)
  material(p,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit K(pose: place([0,0,0]@World)) }
"###,
        r###"error[E210]: authored Lipschitz bound
  --> case.loft:3:3
   |
  3 |   lipschitz(p) = 1.35
   |   ^^^^^^^^^
   = fix: remove the bound and refine the row columns
"###,
    ),
    (
        Code::E211,
        r###"space World Local
kind K @Local (pose: xf@Local>World, r: f = 1) {
  trace(p) = sphere(p - [0,0,0]@Local, 0.5) / r
  material(p,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit K(pose: place([0,0,0]@World)) }
"###,
        r###"error[E211]: row divisor can reach zero over its declared range
  --> case.loft:3:47
   |
  3 |   trace(p) = sphere(p - [0,0,0]@Local, 0.5) / r
   |                                               ^
   = fix: declare a nonzero column range or clamp the divisor away from zero
"###,
    ),
    (
        Code::E212,
        r###"space Body
kind K @Body (pose: xf@Body>World) {
  trace(q) { 1.0 }
  material(q,n) { ggx(rgb(0.5), 0.5, 0) }
}
build { emit K(pose: place([0,0,0]@World)) }
"###,
        r###"error[E212]: row 0: the field has no surface within ±1024 m of the origin
  --> case.loft:2:6
   |
  2 | kind K @Body (pose: xf@Body>World) {
   |      ^
   = fix: check the row's values; a field that is positive everywhere draws nothing
"###,
    ),
    (
        Code::E307,
        r###"space World Local
kind K @Local (pose: xf@Local>World, r: f = 1) {
  trace(p) = sphere(p - [0,0,0]@Local, 0.5)
  material(p,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit K(pose: place([0,0,0]@World)) }
fn bump(t: f) -> f = abs(t - 0.5)
fn slope(t: f) -> f = bump'(t)
"###,
        r###"error[E307]: cannot differentiate a `C0` expression: `abs` has a kink
  --> case.loft:7:22
   |
  7 | fn bump(t: f) -> f = abs(t - 0.5)
   |                      ^^^
   = fix: keep `diff` on the smooth part (smoothstep/smootherstep, sin, exp, ...), or move the kink outside the differentiated fn
"###,
    ),
    (
        Code::E401,
        r###"space World Local
kind K[length: m] @Local (pose: xf@Local>World, r: f = 1) {
  trace(p) = sphere(p - [0,0,0]@Local, 0.5)
  material(p,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit K(pose: place([0,0,0]@World)) }
"###,
        r###"error[E401]: continuous value cannot parameterize kind identity: `length` is `f`
  --> case.loft:2:8
   |
  2 | kind K[length: m] @Local (pose: xf@Local>World, r: f = 1) {
   |        ^^^^^^
   = fix: move `length: f` into the kind’s column list so all instances share one compiled kind
"###,
    ),
    (
        Code::E402,
        r###"space World Local
kind K[a: bool, b: bool, c: bool] @Local (pose: xf@Local>World, r: f = 1) {
  trace(p) = sphere(p - [0,0,0]@Local, 0.5)
  material(p,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit K(pose: place([0,0,0]@World)) }
"###,
        r###"error[E402]: kind variant budget exceeded (8 > max 4)
  --> case.loft:2:26
   |
  2 | kind K[a: bool, b: bool, c: bool] @Local (pose: xf@Local>World, r: f = 1) {
   |                          ^
   = fix: move `c` into the row; each variant is a separate shader module
"###,
    ),
    (
        Code::E403,
        r###"space Body
kind K @Body (pose: xf@Body>World) {
  trace(q) { sphere(q - [0,0,0]@Body, 0.5) }
  material(q,n) { ggx(rgb(0.5), 0.5, 0) }
}
build { let t = time; emit K(pose: place([0,0,0]@World)) }
"###,
        r###"error[E403]: time read at build stage
  --> case.loft:6:17
   |
  6 | build { let t = time; emit K(pose: place([0,0,0]@World)) }
   |                 ^^^^
   = fix: read time in a query or a law
"###,
    ),
    (
        Code::E404,
        r###"space World Local
kind K @Local (pose: xf@Local>World, r: f = 1) {
  trace(p) { emit K(pose: pose); 1.0 }
  material(p,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit K(pose: place([0,0,0]@World)) }
"###,
        r###"error[E404]: `emit` appends a row at stage 0 and runs only inside a `comptime fn`
  --> case.loft:3:14
   |
  3 |   trace(p) { emit K(pose: pose); 1.0 }
   |              ^^^^
   = fix: call a `comptime fn` from the schedule's rows pass and emit there
"###,
    ),
    (
        Code::E405,
        r###"fn field(x: f) -> f = unproved(0.5) { x }
"###,
        r###"error[E405]: unproved outside a geometry query
  --> case.loft:1:23
   |
  1 | fn field(x: f) -> f = unproved(0.5) { x }
   |                       ^^^^^^^^
   = fix: place the block inside trace or surface
"###,
    ),
    (
        Code::E501,
        r###"space World Local
kind K @Local (pose: xf@Local>World, r: f = 1) {
  trace(p) = sphere(p - [0,0,0]@Local, 0.5)
  material(p,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit K(pose: place([0,0,0]@World)) }
law move_row(h: K, dt) writes h.{r} { h.pose = h.pose }
"###,
        r###"error[E501]: field assignment outside its declared writes set
  --> case.loft:7:41
   |
  7 | law move_row(h: K, dt) writes h.{r} { h.pose = h.pose }
   |                                         ^^^^
   = fix: declare the target field in the owning law’s writes clause
"###,
    ),
    (
        Code::E502,
        r###"space World Local
kind K @Local (pose: xf@Local>World, r: f = 1) {
  trace(p) = sphere(p - [0,0,0]@Local, 0.5)
  material(p,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit K(pose: place([0,0,0]@World)) }
law first(h: K, dt) writes h.{r} { h.r = dt }
law second(h: K, dt) writes h.{r} { h.r = dt }
schedule Main { run_laws(first,second) }
"###,
        r###"error[E502]: conflicting writes on K.r
  --> case.loft:8:32
   |
  8 | law second(h: K, dt) writes h.{r} { h.r = dt }
   |                                ^
  --> case.loft:7:31
   |
  7 | law first(h: K, dt) writes h.{r} { h.r = dt }
   |                               - first writer declared here
   = fix: give one scheduled law ownership of this field
"###,
    ),
    (
        Code::E503,
        r###"state Points { pos: array[p3@World, 64] }
solver Broken over Points writes Points.{pos} config { strategy: colored_gs(2) } {
  constrain edge(i, i) in 0..15 { pos[i] = pos[0] }
}
"###,
        r###"error[E503]: constraint endpoints have the same binder
  --> case.loft:3:21
   |
  3 |   constrain edge(i, i) in 0..15 { pos[i] = pos[0] }
   |                     ^
   = fix: use a separate name for each endpoint
"###,
    ),
];
