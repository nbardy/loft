//! Exercise the authored scenes and their scheduled laws without a device.
use loft_abi::{AdvanceArgs, VerdictTag};
use loft_ir::{Sources, Span};
use loft_parse::ast::{Arg, Expr, ExprKind, FnDecl, Item, NumSuffix, Stmt, Ty, TyExpr};
use loft_passes::{
    Compiled, Prog, compile,
    eval::{Eval, RowVal, Val, concrete},
};
use std::{fs, path::Path};

const DT: f32 = AdvanceArgs::DT;
const LAST_TICK: u32 = 481;

fn build(name: &str) -> Compiled {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut prelude: Vec<_> = fs::read_dir(root.join("lib"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "loft"))
        .collect();
    prelude.sort();
    let mut source = Sources::default();
    for path in prelude
        .into_iter()
        .chain([root.join(format!("scenes/{name}.loft"))])
    {
        source.add(
            path.strip_prefix(&root).unwrap().display().to_string(),
            fs::read_to_string(&path).unwrap(),
        );
    }
    let (program, _, mut code) =
        compile(&source, &[]).unwrap_or_else(|diag| panic!("{}", diag.render(&source)));
    assert_eq!(program.dispatches.len(), 1, "{name} motion dispatches");
    let dispatch = &program.dispatches[0];
    assert!(!dispatch.fields.is_empty());
    assert!(!dispatch.checks.is_empty());
    for module in &program.modules {
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        )
        .validate(&module.naga)
        .unwrap_or_else(|e| panic!("{}: {e:?}", module.name));
    }
    println!(
        "{name}: {} modules built and validated",
        program.modules.len()
    );

    // The snapshot compiler's CPU evaluator runs all numeric/branch bodies.
    // Lower its two verdict constructors to the same tag/value pair as
    // snapshot WGSL, since Eval has no native Verdict value.
    let tag_id = code.types.len() as u32;
    code.types.push(Ty::F32);
    for item in code.modules.iter_mut().flat_map(|module| &mut module.items) {
        let Item::Check(check) = item else { continue };
        let Some(Stmt::Return(Some(expr), _)) = check.body.stmts.last_mut() else {
            panic!("{} has no terminal verdict", check.name.name)
        };
        let ExprKind::If {
            then, otherwise, ..
        } = &mut expr.kind
        else {
            panic!("{} has no verdict decision", check.name.name)
        };
        for arm in [then, otherwise] {
            let ExprKind::Call { callee, args } = &mut arm.value.kind else {
                panic!("expected a verdict constructor")
            };
            let tag = match callee.name.as_str() {
                "held" => VerdictTag::Held,
                "missed" => VerdictTag::Missed,
                name => panic!("unexpected verdict {name}"),
            };
            callee.name = "vec2".into();
            args.insert(
                0,
                Arg {
                    name: None,
                    value: Expr {
                        id: tag_id,
                        span: arm.value.span,
                        kind: ExprKind::Num {
                            value: tag as u32 as f64,
                            suffix: NumSuffix::None,
                            text: (tag as u32).to_string(),
                            float: true,
                        },
                    },
                },
            );
        }
    }
    code
}

struct Scene<'p, 'a> {
    ev: Eval<'p, 'a, f32>,
    table: usize,
    columns: &'a [loft_parse::ast::RowField],
    laws: Vec<&'a FnDecl>,
    checks: Vec<&'a FnDecl>,
}

impl<'p, 'a> Scene<'p, 'a> {
    fn new(prog: &'p Prog<'a>, code: &'p Compiled, kind: &str) -> Self {
        let mut laws = vec![];
        let mut checks = vec![];
        for pass in &prog.schedule.unwrap().passes {
            let dst = match pass.verb.name.as_str() {
                "run_laws" => &mut laws,
                "check" => &mut checks,
                _ => continue,
            };
            for arg in &pass.args {
                let ExprKind::Name(name) = &arg.value.kind else {
                    panic!("expected an entry name")
                };
                let fun = if pass.verb.name == "run_laws" {
                    &prog.laws[name.as_str()].f
                } else {
                    prog.checks[name.as_str()]
                };
                assert!(matches!(&fun.params[0].ty, Some(TyExpr::Name(n)) if n.name == kind));
                dst.push(fun);
            }
        }
        let mut ev = Eval::new(prog, &code.types, code.tables.clone(), &[]);
        ev.luts = &code.luts;
        let table = prog.kind_index(kind);
        assert_eq!(ev.tables[table].len(), 1);
        Self {
            ev,
            table,
            columns: &prog.kinds[kind].row,
            laws,
            checks,
        }
    }

    fn column(&self, name: &str) -> usize {
        self.columns
            .iter()
            .position(|f| f.name.name == name)
            .unwrap()
    }

    fn get(&self, name: &str) -> f32 {
        let RowVal::F(x) = self.ev.tables[self.table][0][self.column(name)] else {
            panic!("expected scalar column {name}")
        };
        x
    }

    fn set(&mut self, name: &str, value: f32) {
        let column = self.column(name);
        self.ev.tables[self.table][0][column] = RowVal::F(value);
    }

    fn tick(&mut self, tick: u32) {
        self.ev.time = tick as f32 * DT;
        for law in self.laws.clone() {
            self.ev
                .bind(&law.params[0].name.name, Val::Row(self.table, 0));
            self.ev.bind(&law.params[1].name.name, Val::S(DT));
            let mut local_count = 2;
            // Adapt row writes at the boundary of the existing Eval twin.
            // Every right-hand side and its reachable functions use Eval.
            for stmt in &law.body.stmts {
                match stmt {
                    Stmt::Let {
                        name,
                        value,
                        mutable: false,
                        ..
                    } => {
                        let value = self.ev.expr(value).unwrap();
                        self.ev.bind(&name.name, value);
                        local_count += 1;
                    }
                    Stmt::Assign {
                        target,
                        op: None,
                        value,
                    } => {
                        let ExprKind::Member(base, field) = &target.kind else {
                            panic!("expected row write")
                        };
                        assert!(
                            matches!(&base.kind, ExprKind::Name(n) if n == &law.params[0].name.name)
                        );
                        let v = self.ev.expr(value).unwrap();
                        let RowVal::F(x) = concrete(&v, value.span).unwrap() else {
                            panic!("expected scalar write")
                        };
                        assert!(x.is_finite(), "{} at tick {tick}", field.name);
                        self.set(&field.name, x);
                    }
                    stmt => panic!("unsupported row adapter statement: {stmt:?}"),
                }
            }
            for _ in 0..local_count {
                self.ev.unbind();
            }
        }
    }

    fn verdict(&mut self, check: &'a FnDecl) -> [f32; 2] {
        let Val::V(v, _) = self
            .ev
            .call_fn(check, vec![Val::Row(self.table, 0)], Span::NONE)
            .unwrap()
        else {
            panic!("{} did not return a verdict", check.name.name)
        };
        v.try_into().unwrap()
    }

    fn holds(&mut self, tick: u32) {
        for check in self.checks.clone() {
            let [tag, value] = self.verdict(check);
            assert_eq!(
                tag,
                VerdictTag::Held as u32 as f32,
                "{} at tick {tick}: {value}",
                check.name.name
            );
            assert!(
                value >= 0.0 && value.is_finite(),
                "{} at tick {tick}: {value}",
                check.name.name
            );
        }
    }

    fn toe_y(&mut self, i: u32) -> f32 {
        let Val::V(toe, _) = self
            .ev
            .call_fn(
                self.ev.p.fns["crab_end"],
                vec![Val::Row(self.table, 0), Val::U(i)],
                Span::NONE,
            )
            .unwrap()
        else {
            panic!("crab_end did not return a point")
        };
        toe[1]
    }
}

#[test]
fn crab_reverses_and_rejects_fake_plant() {
    let code = build("crab");
    let prog = code.prog();
    let mut scene = Scene::new(&prog, &code, "Scramble");
    scene.holds(0);
    let plant = prog.checks["crab_planted"];
    scene.set("toe_bias", 0.020);
    let [tag, drift] = scene.verdict(plant);
    assert_eq!(tag, VerdictTag::Missed as u32 as f32);
    assert!((drift - 0.020).abs() < 1e-6, "fake plant drift: {drift}");
    scene.set("toe_bias", 0.0);

    let toe_y = scene.toe_y(0);
    let mut previous = [false; 8];
    let mut lifts = vec![];
    let mut reversed = None;
    let mut worst_drift = 0.0_f32;
    for tick in 1..=LAST_TICK {
        scene.tick(tick);
        scene.holds(tick);
        let [_, margin] = scene.verdict(plant);
        worst_drift = worst_drift.max(0.00005 - margin);
        if scene.get("vx") < 0.0 {
            reversed.get_or_insert(tick);
        }
        let mut lifted = false;
        for (i, was_up) in previous.iter_mut().enumerate() {
            let up = scene.toe_y(i as u32) - toe_y > 0.00005;
            lifted |= up && !*was_up;
            *was_up = up;
        }
        if lifted {
            lifts.push(tick);
        }
    }
    let first = lifts[0];
    let reversed = reversed.unwrap();
    let next = lifts
        .iter()
        .copied()
        .find(|&tick| tick >= reversed)
        .unwrap();
    let from_command = first as f32 * DT;
    let from_zero = (next - reversed) as f32 * DT;
    println!(
        "crab: command lift tick {first} ({from_command:.6} s), reversal {reversed}, next lift {next} ({from_zero:.6} s), drift {worst_drift:.9} m, mutant {drift:.6} m"
    );
    assert!(from_command <= 0.10 && from_zero <= 0.10);
    assert!(worst_drift <= 0.00005);
}

#[test]
fn heron_holds_pins_neck_posture_and_gaze() {
    let code = build("heron");
    let prog = code.prog();
    let mut scene = Scene::new(&prog, &code, "HuntingHeron");
    scene.holds(0);
    let mut min_y = scene.get("hip_y");
    let mut peak_foot = 0.0_f32;
    let mut max_neck_error = 0.0_f32;
    let neck = prog.checks["heron_neck_length"];
    for tick in 1..=LAST_TICK {
        scene.tick(tick);
        scene.holds(tick);
        min_y = min_y.min(scene.get("hip_y"));
        peak_foot = peak_foot.max(scene.get("foot_y"));
        max_neck_error = max_neck_error.max(0.0002 - scene.verdict(neck)[1]);
    }
    println!(
        "heron: crouch {min_y:.6} m, neck error {max_neck_error:.9} m, peak foot {peak_foot:.6} m"
    );
    assert!((min_y - 0.59).abs() < 1e-6 && peak_foot > 0.80);
    assert!((scene.get("foot_y") - 0.20).abs() < 1e-6);
    assert!((scene.get("pin_x") - 1.40).abs() < 1e-6);
}

#[test]
fn boss_slips_then_takes_the_second_hit() {
    let code = build("boss");
    let prog = code.prog();
    let mut scene = Scene::new(&prog, &code, "HermitBoss");
    scene.holds(0);
    let mut hit_ticks = vec![];
    let mut windup = 0.0_f32;
    let mut hits = 0.0;
    let mut stopped = None;
    for tick in 1..=LAST_TICK {
        let v_before = scene.get("vx");
        scene.tick(tick);
        scene.holds(tick);
        windup = windup.max(scene.get("windup"));
        let new_hits = scene.get("hits");
        if new_hits != hits {
            assert_eq!(new_hits, hits + 1.0);
            assert_eq!(scene.get("slipping"), 1.0);
            assert!(scene.get("vx") < v_before - 3.0, "impulse at tick {tick}");
            hit_ticks.push(tick);
            hits = new_hits;
        }
        if hits == 2.0 && scene.get("slipping") == 0.0 {
            stopped.get_or_insert(tick);
        }
    }
    assert_eq!(hit_ticks, [60, 75]);
    assert!((windup - 0.69).abs() < 1e-6);
    println!(
        "boss: windup {windup:.6} m, hits {hit_ticks:?}, replant tick {}",
        stopped.unwrap()
    );
}
