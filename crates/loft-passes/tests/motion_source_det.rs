use loft_ir::{Code, Sources};
use loft_passes::{compile, program::Program};
use std::{fs, path::Path};

#[derive(Clone, Copy, Debug)]
enum Route {
    Law,
    Solver,
    Check,
}

fn scene(route: Route, helper: &str, material: &str) -> Sources {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib");
    let mut paths: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "loft"))
        .collect();
    paths.sort();
    let mut src = Sources::default();
    for path in paths {
        src.add(
            path.display().to_string(),
            fs::read_to_string(&path).unwrap(),
        );
    }
    let motion = match route {
        Route::Law => "law test(h: Probe, dt) writes h.{x} { h.x = apply(h.x) }
schedule Main { run_laws(test) }",
        Route::Solver => "state Test { x: f = 1.0 }
solver TestMotion over Test writes Test.{x} config { strategy: sym_gs, iters: 1 } { x = apply(x) }
schedule Main { run_solver(TestMotion) }",
        Route::Check => "check test(h: Probe) -> f { return if apply(h.x) >= 0.0 { held(1.0) } else { missed(1.0) } }
schedule Main { check(test) }",
    };
    src.add(
        "authored.loft",
        format!(
            "{helper}
space World
kind Probe @World (x: f = 1.0) {{
    trace(p) = sphere(p - [0,0,0]@World, 0.25)
    material(p,n) = {material}
}}
build {{ emit Probe() }}
{motion}
"
        ),
    );
    src
}

const GGX: &str = "ggx([0.5,0.5,0.5], 0.4, 0.0)";

fn accepted(src: &Sources) -> Program {
    compile(src, &[])
        .unwrap_or_else(|d| panic!("{}", d.render(src)))
        .0
}

fn rejected(src: &Sources, token: &str, reason: &str) {
    let Err(d) = compile(src, &[]) else {
        panic!("compiled an illegal motion body containing {token}")
    };
    assert_eq!(d.code, Code::E504, "{}", d.render(src));
    assert!(d.msg.contains(reason), "{}", d.render(src));
    let (path, _, col, line, _) = src.locate(d.span);
    assert_eq!(path, "authored.loft");
    assert!(line[col - 1..].starts_with(token), "{}", d.render(src));
}

fn raw(body: &str) -> String {
    format!("fn apply(x: f) -> f = wgsl det (x: f) -> f {{\n{body}\n}}")
}

#[test]
fn verifies_raw_motion_on_each_route() {
    for route in [Route::Law, Route::Solver, Route::Check] {
        for helper in [
            "fn apply(x: f) -> f = x".into(),
            raw("return x;"),
            raw("var y = x; for (var i: u32; i<3u; i++) { y+=1.0; } return y;"),
            raw("var y = x; for (var i=0u; i<3u; i++) { y+=1.0; } return y;"),
            raw("const limit = 1i + 2i; var y = x;
for (var i=limit; i>0i; i--) {
    for (var j=0u; j<2u; j++) { y += 1.0; }
    continue;
}
return y;"),
        ] {
            accepted(&scene(route, &helper, GGX));
        }
        let unmarked = raw("return x;").replace("wgsl det", "wgsl");
        rejected(&scene(route, &unmarked, GGX), "wgsl", "unverified");
        rejected(
            &scene(route, &raw("return f32(f16(x));"), GGX),
            "wgsl",
            "f16",
        );
        for (body, at, reason) in [
            (
                "var y = x;\nwhile y > 0.0 { y -= 1.0; }\nreturn y;",
                "while",
                "fixed bound",
            ),
            ("return pow(x,x);", "pow", "Pow"),
            (
                "for (var i=0xffffffffu; i<=0xffffffffu; i++) {} return x;",
                "for",
                "fixed bound",
            ),
            (
                "for (var i=0u; i<3u; i++) { let p = &i; *p = 0u; } return x;",
                "for",
                "fixed bound",
            ),
            (
                "var i=0u; let old = i;\nfor (i=0u; old<3u; i++) {} return x;",
                "for",
                "fixed bound",
            ),
            (
                "workgroupBarrier(); return x;",
                "workgroupBarrier",
                "effect",
            ),
        ] {
            rejected(&scene(route, &raw(body), GGX), at, reason);
        }
    }
}

#[test]
fn follows_authored_calls_and_checks_effects() {
    for (route, buffer) in [
        (Route::Law, "rows"),
        (Route::Solver, "solver_rows"),
        (Route::Check, "rows"),
    ] {
        let leaf = format!(
            "fn leaf(x: f) -> f = wgsl det (x: f) -> f {{\n{buffer}[0u] = x; return x;\n}}"
        );
        let loft =
            format!("{leaf}\nfn bridge(x: f) -> f = leaf(x)\nfn apply(x: f) -> f = bridge(x)");
        rejected(
            &scene(route, &loft, GGX),
            buffer,
            "write function-local state",
        );
        let raw = format!(
            "{leaf}\n{}",
            raw("return motion_loft_user_leaf /* a call */ (x);")
        );
        rejected(
            &scene(route, &raw, GGX),
            buffer,
            "write function-local state",
        );
        let transitive = loft.replace(&format!("{buffer}[0u] = x;"), "while x > 0.0 {}");
        rejected(&scene(route, &transitive, GGX), "while", "fixed bound");
        let transitive = raw.replacen("wgsl det", "wgsl", 1);
        rejected(&scene(route, &transitive, GGX), "wgsl", "unverified");
    }
}

#[test]
fn wraps_motion_math_and_preserves_render_islands() {
    let helper = raw("return sin(cos(x)) + exp(x) + log(x) + atan2(x, x) + acos(x);");
    let render = "fn tint(x: f) -> f = wgsl (x: f) -> f { return pow(x,x); }";
    let helper = format!("{render}\n{helper}");
    let src = scene(Route::Law, &helper, "ggx([tint(x), 0.5, 0.5], 0.4, 0.0)");
    let p = accepted(&src);
    let motion = &p.modules[p.dispatches[0].module];
    assert!(motion.wgsl.contains(
        "return det_sin(det_cos(x)) + det_exp(x) + det_log(x) + det_atan2(x, x) + det_acos(x)"
    ));
    assert!(
        p.modules
            .iter()
            .any(|m| m.name == "Probe" && m.wgsl.contains("return pow(x,x);"))
    );
    assert!(
        motion
            .origins
            .iter()
            .any(|o| o.span.0 == "authored.loft" && o.span.1 == 2)
    );
}

#[test]
fn leaves_outbox_atomics_in_generated_code() {
    let mut src = scene(Route::Law, &raw("return x;"), GGX);
    let (_, authored) = src.files.last_mut().unwrap();
    *authored = authored.replace(
        "h.x = apply(h.x) }",
        "h.x = apply(h.x); emit_event(ProbeTick, 0, 0, [1.0, 0.0, 0.0, 0.0]) }",
    );
    let p = accepted(&src);
    assert!(
        p.modules[p.dispatches[0].module]
            .wgsl
            .contains("atomicAdd(&loft_outbox")
    );
}
