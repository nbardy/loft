//! The language's promises, as the errors that keep them. Each case is a bug the checker must refuse: if one of
//! these compiles, the class of bug it names (the draft's gaze-frame bug, an unproven march, a per-instance module
//! explosion, a stage-0 leak into kind code) is silently back.

use loft_ir::{Code, Sources};
use std::path::Path;

fn compile(scene: &str) -> Result<(), (Code, String)> {
    let lib = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib");
    let mut src = Sources::default();
    let mut files: Vec<_> = std::fs::read_dir(&lib).expect("lib/").map(|e| e.expect("entry").path()).collect();
    files.sort();
    for f in files {
        src.add(f.display().to_string(), std::fs::read_to_string(&f).expect("lib file"));
    }
    src.add("case.loft", scene);
    loft_passes::compile(&src, &[]).map(|_| ()).map_err(|d| (d.code, d.render(&src)))
}

const SCHEDULE: &str = "
schedule Main {
  pass rows: spawn();
  pass camera: orbit(at: Pt[World](0.0, 0.0, 0.0), radius: 4.0m, height: 1.0m, period: 10.0s, fov: 40deg);
  pass cull: build_indirect(K);
  pass vis: render_bundle(Vis64) { draw_indirect(K); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky, sun: vec3(0.0, 1.0, 0.0));
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
comptime fn spawn() { emit K { pose: Xform[Body -> World](Pt[World](0.0, 0.0, 0.0), 0.0) }; }
";

/// A kind around a trace body and a material body, both in the kind's `Body` space.
fn kind(params: &str, trace: &str, material: &str) -> String {
    format!(
        "space World, Body;
kind K{params} {{
  row {{ pose: Xform[Body -> World] }}
  query trace(self, p: Pt[World]) -> Field[Body] {{
    let q = inv(self.pose) * p;
    {trace}
  }}
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {{
    let q = inv(self.pose) * p;
    {material}
  }}
}}
{SCHEDULE}"
    )
}

const OK_TRACE: &str = "return sd_sphere(q - Pt[Body](0.0, 0.0, 0.0), 0.5);";
const OK_MATERIAL: &str = "return ggx(vec3(0.5), 0.5, 0.0);";

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
    let material = "map sym = mirror_x(q) |> translate(Vec[Body](-0.4, 0.5, 1.2));
    let eye_c = Pt['sym](0.0, 0.2, 0.1);
    let eye_world = self.pose * eye_c;
    return ggx(vec3(eye_world.x), 0.5, 0.0);";
    expect(Code::E104, &kind("", OK_TRACE, material));
    // The fix the error names compiles.
    let fixed = material.replace("self.pose * eye_c", "self.pose * (sym.to_parent * eye_c)");
    if let Err((_, text)) = compile(&kind("", OK_TRACE, &fixed)) {
        panic!("{text}");
    }
}

#[test]
fn e104_points_do_not_add() {
    expect(Code::E104, &kind("", "let r = q + q; return r.x;", OK_MATERIAL));
}

/// A denominator whose interval contains 0 over the traced region: L is `Unbounded`, so the march is refused.
/// The box keeps the surface bounded, so the refusal is E211 (no L), not E212 (no bound).
#[test]
fn e211_unbounded_lipschitz() {
    let trace = "let flare = 0.04 / max(0.0, length(q.xz) - 0.3);
    let o = Pt[Body](0.0, 0.0, 0.0);
    return max(sd_sphere(q - o, 0.5) - flare, sd_box(q - o, vec3(0.6)));";
    expect(Code::E211, &kind("", trace, OK_MATERIAL));
    // Clamping the denominator away from zero, as the fix says, derives a finite L.
    let fixed = trace.replace("max(0.0,", "max(0.05,");
    if let Err((_, text)) = compile(&kind("", &fixed, OK_MATERIAL)) {
        panic!("{text}");
    }
}

#[test]
fn e211_lipschitz_is_never_authored() {
    expect(Code::E211, &kind("", OK_TRACE, OK_MATERIAL).replace("-> Field[Body]", "-> Field[Body, 1.35]"));
}

/// Twelve legs as twelve kinds would mint twelve modules: a continuous kind parameter is refused.
#[test]
fn e401_continuous_kind_parameter() {
    expect(Code::E401, &kind("[comptime femur_len: Meters]", OK_TRACE, OK_MATERIAL));
}

#[test]
fn e402_kind_variant_budget() {
    expect(Code::E402, &kind("[comptime leg: u32 in 0u..12u]", OK_TRACE, OK_MATERIAL));
    expect(Code::E402, &kind("[comptime a: bool, comptime b: bool, comptime c: bool]", OK_TRACE, OK_MATERIAL));
}

#[test]
fn e403_emit_outside_stage_0() {
    expect(Code::E403, &kind("", "emit K { pose: self.pose }; return 1.0;", OK_MATERIAL));
}

/// Kind code reading a stage-0 constant would make the module depend on the instance count.
#[test]
fn e404_kind_code_reads_stage_0() {
    let src = kind("", "return sd_sphere(q - Pt[Body](0.0, 0.0, 0.0), 0.1 * f32(COUNT));", OK_MATERIAL);
    expect(Code::E404, &format!("{src}\ncomptime const COUNT: u32 = 20u;"));
}

/// The draft's curvature bug: differentiating through a kink (here `abs`) has no derivative to give; `diff` refuses.
#[test]
fn e307_diff_of_a_c0_expression() {
    let src = format!("{}\nfn bump(t: f32) -> f32 {{ return abs(t - 0.5); }}\nfn slope(t: f32) -> f32 {{ return diff(bump, t)(t); }}", kind("", OK_TRACE, OK_MATERIAL));
    expect(Code::E307, &src);
    // A smooth bump differentiates.
    if let Err((_, text)) = compile(&src.replace("abs(t - 0.5)", "smootherstep(0.0, 1.0, t)")) {
        panic!("{text}");
    }
}
