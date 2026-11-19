//! The language's promises, as the errors that keep them. Each case is a bug the checker must refuse: if one of
//! these compiles, the class of bug it names (the draft's gaze-frame bug, an unproven march, a per-instance module
//! explosion, a stage-0 leak into kind code) is silently back.

use loft_ir::{Code, Sources};
use std::path::Path;

fn compile(scene: &str) -> Result<(), (Code, String)> {
    let lib = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib");
    let mut src = Sources::default();
    let mut files: Vec<_> = std::fs::read_dir(&lib)
        .expect("lib/")
        .map(|e| e.expect("entry").path())
        .collect();
    files.sort();
    for f in files {
        src.add(
            f.display().to_string(),
            std::fs::read_to_string(&f).expect("lib file"),
        );
    }
    src.add("case.loft", scene);
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
        &kind("[a: bool, b: bool, c: bool]", OK_TRACE, OK_MATERIAL),
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
    assert!(compile(&motion.replace("wgsl (","wgsl det (")).is_ok());
}
