//! The language's promises, as the errors that keep them. Each case is a bug the checker must refuse: if one of
//! these compiles, the class of bug it names (the draft's gaze-frame bug, an unproven march, a per-instance module
//! explosion, a stage-0 leak into kind code) is silently back.

use loft_ir::Code;
use std::path::Path;

fn compile(scene: &str) -> Result<(), (Code, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = loft_passes::source::Graph::text(&root, Path::new("case.loft"), &format!(
        "{scene}\nuse stdlib::sdf::solid::*\nuse stdlib::materials::bsdf::*\nuse stdlib::geometry::analytic::*\nuse stdlib::frames::transport::*"
    ));
    loft_passes::compile(&src, &[])
        .map(|_| ())
        .map_err(|d| (d.code, d.render(&src)))
}

fn kind(params: &str, trace: &str, material: &str) -> String {
    format!(
        "space World Body
kind K{params} @Body (pose: xf@Body>World) {{
  trace(q) {{ {trace} }}
  material(q,n) {{ {material} }}
}}
build {{ emit K(pose: place([0,0,0]@World)) }}"
    )
}
const OK_TRACE: &str = "sphere(q - [0,0,0]@Body, 0.5)";
const OK_MATERIAL: &str = "ggx(rgb(0.5), 0.5, 0)";
fn expect(code: Code, src: &str) {
    match compile(src) {
        Ok(()) => panic!("expected {code}, but it compiled"),
        Err((c, text)) => assert_eq!(c, code, "expected {code}, got:\n{text}"),
    }
}

#[test]
fn the_baseline_compiles() {
    if let Err((_, text)) = compile(&kind("", OK_TRACE, OK_MATERIAL)) {
        panic!("{text}");
    }
}

/// The draft's gaze bug: a point in a mirrored map space used as if it were in `Body`.
#[test]
fn e104_point_from_a_map_space_without_to_parent() {
    let material = "map sym = q |> mirror_x |> move([-0.4, 0.5, 1.2]);
    let eye_c = sym.pt(0, 0.2, 0.1);
    let eye_world = pose * eye_c;
    return ggx(rgb(eye_world.x), 0.5, 0.0);";
    expect(Code::E104, &kind("", OK_TRACE, material));
    // The fix the error names compiles.
    let fixed = material.replace("pose * eye_c", "pose * sym.out(eye_c)");
    if let Err((_, text)) = compile(&kind("", OK_TRACE, &fixed)) {
        panic!("{text}");
    }
}

#[test]
fn e101_points_do_not_add() {
    expect(
        Code::E101,
        &kind("", "let r = q + q; return r.x;", OK_MATERIAL),
    );
}

/// A denominator whose interval contains 0 over the traced region: L is `Unbounded`, so the march is refused.
/// The box keeps the surface bounded, so the refusal is E211 (no L), not E212 (no bound).
#[test]
fn e211_unbounded_lipschitz() {
    let trace = "let flare = 0.04 / max(0.0, length(q.xz) - 0.3);
    let o = [0,0,0]@Body;
    return max(sphere(q - o, 0.5) - flare, box_sdf(q - o, rgb(0.6)));";
    expect(Code::E211, &kind("", trace, OK_MATERIAL));
    // Clamping the denominator away from zero, as the fix says, derives a finite L.
    let fixed = trace.replace("max(0.0,", "max(0.05,");
    if let Err((_, text)) = compile(&kind("", &fixed, OK_MATERIAL)) {
        panic!("{text}");
    }
}

#[test]
fn e210_lipschitz_is_never_authored() {
    expect(
        Code::E210,
        &kind("", OK_TRACE, OK_MATERIAL).replace("  trace(q)", "  lipschitz(q) = 1.35\n  trace(q)"),
    );
}

/// Twelve legs as twelve kinds would mint twelve modules: a continuous kind parameter is refused.
#[test]
fn e401_continuous_kind_parameter() {
    expect(Code::E401, &kind("[femur_len: m]", OK_TRACE, OK_MATERIAL));
}

#[test]
fn e402_kind_variant_budget() {
    expect(
        Code::E402,
        &kind("[leg: u in 0..12]", OK_TRACE, OK_MATERIAL),
    );
    expect(
        Code::E402,
        &kind("[a: bool, b: bool, c: bool, d: bool]", OK_TRACE, OK_MATERIAL),
    );
}

#[test]
fn e404_emit_outside_stage_0() {
    expect(
        Code::E404,
        &kind("", "emit K(pose: pose); return 1.0;", OK_MATERIAL),
    );
}

/// Build rows cannot depend on a frame that has not happened.
#[test]
fn e403_build_reads_time() {
    let src = kind("", OK_TRACE, OK_MATERIAL).replace("build { emit", "build { let t = time; emit");
    expect(Code::E403, &src);
}

#[test]
fn row_defaults_obey_strict_and_inclusive_bounds() {
    let src = kind("", OK_TRACE, OK_MATERIAL).replace(
        "pose: xf@Body>World)",
        "pose: xf@Body>World, size: f > 0 = 0.5)",
    );
    assert!(compile(&src).is_ok());
    expect(Code::E602, &src.replace("= 0.5)", "= 0)"));
    expect(
        Code::E602,
        &src.replace("emit K(pose:", "emit K(size: -1, pose:"),
    );
}

#[test]
fn write_ownership_is_checked_across_the_schedule() {
    let src = kind("", OK_TRACE, OK_MATERIAL)
        .replace("pose: xf@Body>World)", "pose: xf@Body>World, yaw: rad = 0)");
    let laws = "\nlaw first(h: K, dt) writes h.{yaw} { h.yaw = dt }\nlaw second(h: K, dt) writes h.{yaw} { h.yaw = dt }\nschedule Main { run_laws(first,second) }";
    expect(Code::E502, &format!("{src}{laws}"));
    expect(
        Code::E501,
        &format!("{src}\nlaw first(h: K, dt) writes h.{{yaw}} {{ h.pose = h.pose }}"),
    );
}

/// The draft's curvature bug: differentiating through a kink (here `abs`) has no derivative to give; `diff` refuses.
#[test]
fn e307_diff_of_a_c0_expression() {
    let src = format!(
        "{}\nfn bump(t: f) -> f {{ return abs(t - 0.5); }}\nfn slope(t: f) -> f {{ return diff(bump, t)(t); }}",
        kind("", OK_TRACE, OK_MATERIAL)
    );
    expect(Code::E307, &src);
    // A smooth bump differentiates.
    if let Err((_, text)) = compile(&src.replace("abs(t - 0.5)", "smootherstep(0.0, 1.0, t)")) {
        panic!("{text}");
    }
}

#[test]
fn row_division_needs_a_proof_before_specialization(){
    let src=kind("", "let divisor = scale; sphere(q - [0,0,0]@Body, 0.5) / divisor",OK_MATERIAL)
        .replace("pose: xf@Body>World)", "pose: xf@Body>World, scale: f = 1)");
    expect(Code::E211,&src);
    assert!(compile(&src.replace("scale: f =", "scale: f > 0 =")).is_ok());
}

#[test]
fn raw_shader_islands_cannot_certify_geometry_or_unmarked_motion(){
    let src=kind("", "wgsl (q: p3@Body) -> f { return length(q) - 0.5; }",OK_MATERIAL);
    expect(Code::E211,&src);
    let motion=kind("",OK_TRACE,OK_MATERIAL).replace("pose: xf@Body>World)", "pose: xf@Body>World, yaw: rad = 0)")
        +"\nfn drift(x: f) -> f = wgsl (x: f) -> f { return x; }\nlaw turn(h: K, dt) writes h.{yaw} { h.yaw = drift(dt) }";
    expect(Code::E504,&motion);
    assert!(compile(&motion.replace("law turn(h: K, dt) writes", "law turn(h: K, dt) nondet writes")).is_ok());
}

#[test]
fn forward_kind_map_and_bare_map_refusal(){
    let scene=kind("", "sphere(sym.p - sym.pt(0,0,0),0.5)",OK_MATERIAL)
        .replace("  material(q,n)", "  map sym(point) = point |> mirror_x\n  material(q,n)");
    assert!(compile(&scene).is_ok());
    expect(Code::E106,&scene.replace("sphere(sym.p","sphere(sym"));
}

#[test]
fn branch_cannot_hide_a_row_divisor() {
    let scene = kind("", "var d = 1.0; if scale < 1 { d = scale }; sphere(q - [0,0,0]@Body, 0.5) / d", OK_MATERIAL)
        .replace("pose: xf@Body>World)", "pose: xf@Body>World, scale: f = 1)");
    expect(Code::E211, &scene);
}

#[test]
fn row_declarations_resolve_outside_the_emit_scope() {
    let src = "const minimum = 1.0\n".to_owned() + &kind("", OK_TRACE, OK_MATERIAL)
        .replace("pose: xf@Body>World)", "pose: xf@Body>World, scale: f > minimum = minimum + 1)");
    let shadow = src.replace("build { emit", "build { let minimum = -1.0; emit");
    assert!(compile(&shadow).is_ok(), "the default uses the module minimum");
    expect(Code::E602, &shadow.replace("emit K(pose:", "emit K(scale: minimum + 1, pose:"));
}

#[test]
fn row_division_preserves_dependencies_across_constants() {
    let src = "const offset = 1.0\n".to_owned() + &kind("", "sphere(q - [0,0,0]@Body, 0.5) / (scale - offset)", OK_MATERIAL)
        .replace("pose: xf@Body>World)", "pose: xf@Body>World, scale: f = 2)");
    expect(Code::E211, &src);
    assert!(compile(&src.replace("scale: f =", "scale: f > offset =")).is_ok());
}

#[test]
fn checks_top_level_payloads_and_aliases() {
    let src = "struct Sample { value: Distance, count: u }\ntype Distance = f\nfn sample(s: Sample) -> Distance = s.value\n".to_owned()
        + &kind("", OK_TRACE, OK_MATERIAL).replace("pose: xf@Body>World)", "pose: xf@Body>World, x: Distance = 0.5)");
    assert!(compile(&src).is_ok());
    expect(Code::E011, &src.replace("type Distance = f", "type Distance = Sample"));
    expect(Code::E011, &src.replace("s.value", "s.missing"));
    expect(Code::E011, &src.replace("count: u", "value: u"));
}

#[test]
fn e403_stage_requirements_follow_pure_calls_and_constants() {
    let src = "fn clock_now() -> f = time\nconst stamp: f = clock_now()\nfn wrapped() -> f = stamp\n".to_owned()
        + &kind("", OK_TRACE, OK_MATERIAL).replace("build { emit", "build { let n = wrapped(); emit");
    expect(Code::E403, &src);
    assert!(compile(&src.replace("= time", "= 2.0")).is_ok());
}

#[test]
fn space_polymorphic_material_and_kind_local_calls_lower() {
    let first = kind("", "local(q)", "chitin(q, n)")
        .replace("  trace(q)", "  fn local(q: p3@Body) -> f = loft_rot_y(q)\n  trace(q)");
    let second = kind("", OK_TRACE, "chitin(q, n)").replace("space World Body", "space Other").replace("Body", "Other").replace("kind K ", "kind Second ").replace("emit K(", "emit Second(");
    let src = format!("fn chitin(p, n) = ggx(rgb(0.5 + p.x * n.x), 0.5, 0)\nfn loft_rot_y(q: p3@Body) -> f = sphere(q - [0,0,0]@Body, 0.5)\n{first}\n{second}");
    if let Err((_, text)) = compile(&src) { panic!("{text}"); }
}

#[test]
fn implied_schedule_ignores_unpopulated_kinds_and_resolves_imports() {
    let src = kind("", OK_TRACE, OK_MATERIAL);
    let unused = "\nkind Ball @World () { trace(q) = sphere(q - [0,0,0]@World, 0.2); material(q,n) = ggx(rgb(1), 0.5, 0) }";
    assert!(compile(&format!("{src}{unused}")).is_ok());
    expect(Code::E011, &format!("use fake::lighting::*\n{src}"));
    expect(Code::E505, &format!("{src}\nschedule Main {{ pass late reads [NeverWritten] writes [Other] {{ compute(entry: op) }} }}"));
}

#[test]
fn bracketed_if_expression_scopes_branch_locals() {
    let src = "fn choose(x: f) -> f = max(0, if x < 0 { let v = -x; v } else if x > 1 { let v = 2.0; v } else { x })\n".to_owned()
        + &kind("", OK_TRACE, "ggx(rgb(choose(q.x)), 0.5, 0)");
    if let Err((_, text)) = compile(&src) { panic!("{text}"); }
}

#[test]
fn frozen_diagnostic_fixtures() {
    let dir=Path::new(env!("CARGO_MANIFEST_DIR")).join("../loft-abi/tests/negative");
    for code in [Code::E104, Code::E105, Code::E108, Code::E210, Code::E211, Code::E307, Code::E401,
        Code::E402, Code::E404, Code::E501, Code::E502, Code::E504, Code::E505] {
        expect(code,&std::fs::read_to_string(dir.join(format!("{code}.loft"))).unwrap());
    }
}

#[test]
fn diff_preserves_inactive_helper_parameters() {
    compile(r#"
space World
fn powered(x: f, exponent: f) -> f = pow(x, exponent)
fn alias_power(x: f, exponent: f) -> f {
    let k = exponent
    powered(x, k)
}
fn test_derivative(x: f, exponent: f) -> f = diff(alias_power, x)(x, exponent)
kind Sphere @World (expo: f) {
    trace(p) = sphere(p - [0, 0, 0]@World, 1)
    material(p, n) = ggx([test_derivative(0.5, expo), 0.2, 0.1], 0.5, 0)
}
build { emit Sphere(expo: 2) }
"#).unwrap();
}

/// F1: `atan2`, `acos`, `log` and `fma` passed the checker and panicked the bound evaluator
/// (`unreachable!("checked builtin atan2")`); `asin`, `atan` and `tan` were unknown names (E011). Each now derives
/// a bound. `atan2` on a body round the axis, where its value jumps by 2π and its slope is 1/r, is E211, not a panic.
#[test]
fn inverse_trig_in_a_trace_derives_a_bound_or_e211() {
    let ok = "let o = [0,0,0]@Body; let c = clamp(q.y, -0.5, 0.5);
    return sphere(q - o, 0.5) + 0.01 * (acos(c) + asin(c) + atan(q.x) + tan(c) + log(2.0 + c) + fma(q.x, 0.5, 0.1) + atan2(q.z, q.x + 2.0));";
    if let Err((_, text)) = compile(&kind("", ok, OK_MATERIAL)) {
        panic!("{text}");
    }
    let (code, text) = compile(&kind("", "return sphere(q - [0,0,0]@Body, 0.5) + 0.01 * cos(12.0 * atan2(q.z, q.x));", OK_MATERIAL))
        .expect_err("atan2 round the axis has no finite bound");
    assert_eq!(code, Code::E211, "{text}");
    assert!(text.contains("atan2 jumps by 2π"), "{text}");
}

/// F2: a branch on `time` takes one arm at every point of the region, so its bound is the larger arm's. A branch on
/// the point stays refused, including the `if` expression that used to keep the larger arm's L across the jump
/// (`if q.x > 0.0 { sphere } else { 5.0 }` derived L = 1 for a field that steps at x = 0).
#[test]
fn a_branch_on_time_is_bounded_by_its_arms_and_a_branch_on_the_point_is_not() {
    for trace in [
        "let o = [0,0,0]@Body; return select(sphere(q - o, 0.5), sphere(q - o, 0.4), time > 3.0);",
        // A statement after an `if` block needs its own line: on one line the parser asks for `;` (E001).
        "let o = [0,0,0]@Body; if time > 3.0 { return sphere(q - o, 0.4); }
         return sphere(q - o, 0.5);",
        "let o = [0,0,0]@Body; var r = 0.5; if time > 2.0 { r = 0.45; }
         let d = if time > 3.0 { let s = 0.4; sphere(q - o, s) } else { sphere(q - o, r) }; return d;",
    ] {
        if let Err((_, text)) = compile(&kind("", trace, OK_MATERIAL)) {
            panic!("{text}");
        }
    }
    expect(Code::E211, &kind("", "let o = [0,0,0]@Body; return select(sphere(q - o, 0.5), 5.0, q.x > 0.0);", OK_MATERIAL));
    expect(Code::E211, &kind("", "let o = [0,0,0]@Body; let d = if q.x > 0.0 { sphere(q - o, 0.5) } else { 5.0 }; return d;", OK_MATERIAL));
}

/// F9b: an untyped integer const as a loop bound in a function the trace reaches was emitted `i32` against the
/// `u32` loop index, and WGSL validation refused the module.
#[test]
fn an_untyped_const_bounds_a_gpu_loop() {
    let scene = format!("const N = 6
fn bumps(y: f) -> f {{
  var s = 0.0
  for i in 0..N {{ s = s + 0.01 * sin(f32(i) * y) }}
  s
}}
{}", kind("", "return sphere(q - [0,0,0]@Body, 0.5) + bumps(q.y);", OK_MATERIAL));
    if let Err((_, text)) = compile(&scene) {
        panic!("{text}");
    }
}

/// F9a: `--set NAME` reached only entry-file params. A sibling module's param was refused, and with a second param
/// of the same name in the entry, the sibling's was silently left unchanged. Both scenes' L is the param's value.
#[test]
fn set_reaches_a_sibling_modules_param_and_refuses_an_ambiguous_name() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sibling_param");
    let set = [("size".to_string(), 2.0)];
    let src = loft_passes::source::Graph::file(&dir.join("main.loft"), "").unwrap();
    let (_, report, _) = loft_passes::compile(&src, &set).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    assert_eq!(report.kinds[0].l_max, 2.0);
    let src = loft_passes::source::Graph::file(&dir.join("dup.loft"), "").unwrap();
    let Err(d) = loft_passes::compile(&src, &set) else { panic!("two params named `size` must not both answer --set size") };
    assert_eq!(d.code, Code::E011, "{}", d.render(&src));
}

#[test]
fn empty_scheduled_part_costs_no_program_resources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = "
        use stdlib::materials::bsdf::{lambert}
        use stdlib::sdf::solid::{sphere}
        kind Empty @World (phase: f) { trace(p) = sphere(p - [0,0,0]@World, 1.0); material(p,n) = lambert(rgb(1.0)) }
        law advance(h: Empty, dt: s) writes h.{phase} { h.phase = h.phase + dt }
        schedule Main {
          orbit(at: [0,0,0]@World, radius: 2m, fov: 40deg)
          run_laws(advance)
          cull_indirect(Empty)
          render_bundle(Vis64) { draw_indirect(Empty) }
          find_shadow_map(Empty, size: 512)
          tile_classify(Vis64)
          shade_tiles(Vis64, light: sun_sky, sun: [0,1,0])
          tonemap_aces(ColorHdr, sky: sky_color)
        }";
    let src = loft_passes::source::Graph::text(&root, Path::new("empty.loft"), source);
    let (program, _, _) = loft_passes::compile(&src, &[]).unwrap_or_else(|e| panic!("{}", e.render(&src)));
    assert!(program.kinds.is_empty() && program.instances.is_empty() && program.dispatches.is_empty());
    assert_eq!(program.shadow_size, 0);
}
