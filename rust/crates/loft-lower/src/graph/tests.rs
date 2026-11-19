use super::{Arena, Expr, In, Op, Prim, ProbeBind, Ty, lit};
use std::collections::HashMap;

#[test]
fn number_spelling_observes_js_ties_and_cutoffs() {
    assert_eq!(lit::to_fixed(2.25, 1), "2.3");
    assert_eq!(lit::to_precision(2.25, 2), "2.3");
    assert_eq!(lit::to_fixed(2.55, 1), "2.5");
    assert_eq!(lit::to_fixed(-0.0, 1), "0.0");
    assert_eq!(lit::to_fixed(-0.001, 1), "-0.0");
    assert_eq!(lit::to_string(1e-6), "0.000001");
    assert_eq!(lit::to_string(1e-7), "1e-7");
    assert_eq!(lit::to_string(1e21), "1e+21");
    let n = 1_000_000_000_000_000_128.0;
    assert_eq!(lit::to_string(n), "1000000000000000100");
    assert_eq!(lit::to_fixed(n, 1), "1000000000000000128.0");
    let tiny = f64::from_bits(1);
    assert_eq!(lit::to_precision(tiny, 17), "4.9406564584124654e-324");
    assert_eq!(lit::to_string(tiny), "5e-324");
    assert_eq!(lit::to_string(f64::MAX), "1.7976931348623157e+308");
    let n = f64::from(0.1_f32);
    assert_eq!(lit::lit(n), "0.1");
    assert_eq!(lit::scene_lit(n), "0.10000000149011612");
    assert_eq!(lit::recur_lit(n), "0.10000000149011612");
}

#[test]
fn substitute_reapplies_style_identities_but_preserves_a_lifted_one() {
    let mut a = Arena::default();
    let x = a.variable("x", Ty::V3);
    let y = a.variable("y", Ty::V3);
    let sum = a.prim(Prim::Add, &[x, y]).unwrap();
    let zero = a.lift(&[0.0; 3]).unwrap();
    let replaced = a.with_identities(|a| a.substitute(&[sum], y, zero)).unwrap();
    assert_eq!(replaced, [x]);

    let same = a.prim(Prim::Mul, &[In::from(x), In::from(&[1.0; 3])]).unwrap();
    assert_eq!(same, x);
    let ones = a.lift(&[1.0; 3]).unwrap();
    let mult = a.prim(Prim::Mul, &[x, ones]).unwrap();
    assert_ne!(mult, x);

    let minus_zero = a.lift(-0.0).unwrap();
    assert_eq!(minus_zero, a.lift(0.0).unwrap());
    let Op::Const(c) = &a[minus_zero].op else { panic!("constant") };
    assert!(c[0].is_sign_negative());
}

#[test]
fn fold_preserves_overflow_knowledge_and_rounds_dot_products_before_addition() {
    let mut a = Arena::default();
    let x = a.lift(1.0).unwrap();
    let huge = a.lift(f64::from(f32::MAX)).unwrap();
    let over = a.prim(Prim::Add, &[huge, huge]).unwrap();
    let div = a.prim(Prim::Div, &[x, over]).unwrap();
    let left = a.lift(&[4097.0, -16_785_408.0]).unwrap();
    let right = a.lift(&[4097.0, 1.0]).unwrap();
    let dot = a.prim(Prim::Dot, &[left, right]).unwrap();
    let zero = a.lift(0.0).unwrap();
    assert_eq!(a.fold(&[over, div, dot]).unwrap(), [over, zero, zero]);
}

#[test]
fn loop_rebuilds_once_and_next_state_lets_occupy_ordinals() {
    let mut a = Arena::default();
    let x = a.variable("x", Ty::F32);
    let zero = a.lift(0.0).unwrap();
    let one = a.lift(1.0).unwrap();
    let roots = a
        .iterate(3.0, &[zero, one], |a, state, i| {
            let next = a.prim(Prim::Add, &[state[1], i, x])?;
            Ok((vec![next, state[0]], None))
        })
        .unwrap();
    let roots = a.substitute(&roots, x, one).unwrap();
    let add = a.prim(Prim::Add, &roots).unwrap();
    let e = a.emit_wgsl(&[add], &ProbeBind, "e", &HashMap::<Expr, String>::new(), None).unwrap();
    assert_eq!(e.out, ["e4"]);
    assert_eq!(e.lines.iter().filter(|l| l.starts_with("for")).count(), 1);
    assert!(e.lines.contains(&"  let eL0n1: f32 = eL0s0;".to_owned()));
    assert!(e.lines.contains(&"let e4: f32 = (eL0s0 + eL0s1);".to_owned()));
}

#[test]
fn wire_function_binder_does_not_capture_a_free_variable_with_the_same_name() {
    use loft_ir::graph::{Graph, GraphFn, Node, Op};

    let node = |op, args, n| Node { op, ty: Ty::F32, args, n };
    let graph = Graph {
        nodes: [
            node(Op::Bind(0), 0, 0),
            node(Op::Var(0), 0, 0),
            node(Op::Prim(Prim::Add), 0, 2),
            node(Op::Lit(0), 0, 1),
            node(Op::Call(0), 2, 1),
        ]
        .into(),
        args: [1, 0, 3].into(),
        lits: [2.0_f64.to_bits()].into(),
        syms: "F0a0sum".into(),
        sym_at: [0, 4, 7].into(),
        fns: [GraphFn { name: 1, ret: Ty::F32, binders: [0].into(), body: 2 }].into(),
        roots: [4].into(),
        ..Graph::default()
    };
    let imported = Arena::from_wire(&graph).unwrap();
    let e = imported.arena.emit_wgsl(&imported.roots, &ProbeBind, "e", &HashMap::new(), None).unwrap();
    assert_eq!(e.lines, ["let e0: f32 = (F0a0 + 2.0);"]);
}

#[test]
fn variadic_operator_refuses_a_bad_argument_before_lifting_earlier_ones() {
    let mut a = Arena::default();
    let args = [In::Number(1.0), In::Number(2.0), In::Array(&[3.0])];
    let err = a.prim(Prim::Add, &args).unwrap_err();
    let crate::Error::Scene(head) = err else { panic!("expected a scene refusal") };
    assert!(head.starts_with("add, operand 3: given an array of 1 numbers; expected an expression"));
    assert!(a.nodes.is_empty());
}
