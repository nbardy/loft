use super::*;

#[test]
fn comparison_checks_operand_types() {
    let compare = |op, a: &Ty, b: &Ty| binary(op, a, b, Span::NONE, Span::NONE, Span::NONE);
    for ty in [
        Ty::Bsdf,
        Ty::Xform(Space::Open, Space::Open),
        Ty::Map(Space::Open, Space::Open),
        Ty::Struct("World".into()),
    ] {
        for op in [BinOp::Lt, BinOp::Le, BinOp::Gt, BinOp::Ge, BinOp::Eq, BinOp::Ne] {
            assert_eq!(compare(op, &ty, &ty).unwrap_err().code, Code::E003);
        }
    }
    for ty in [Ty::Bool, Ty::VecN(3), Ty::Row("K".into()), Ty::Id("K".into())] {
        assert_eq!(compare(BinOp::Lt, &ty, &ty).unwrap_err().code, Code::E003);
        assert_eq!(compare(BinOp::Eq, &ty, &ty).unwrap(), Ty::Bool);
    }
    assert_eq!(compare(BinOp::Ge, &Ty::F32, &Ty::AbstractInt).unwrap(), Ty::Bool);
    assert_eq!(compare(BinOp::Ge, &Ty::I32, &Ty::U32).unwrap_err().code, Code::E003);
}

#[test]
fn det_kernel_checks_transitive_helpers() {
    for (operation, diagnostic) in [
        ("pow(x, 2.0)", "no deterministic motion implementation"),
        ("wgsl(x: f) -> f { return x; }", "unverified raw WGSL"),
    ] {
        for tier in ["", "nondet"] {
            let source = format!(
                "fn leaf(x: f) -> f = {operation}\n\
                 fn helper(x: f) -> f = leaf(x)\n\
                 kernel probe(x: f) {tier} {{ let y = helper(x) }}"
            );
            let modules = [loft_parse::parse(0, &source, &mut 0).unwrap()];
            let p = Prog::index(&modules).unwrap();
            let result = check(&p, &mut vec![]);
            if tier.is_empty() {
                let Err(err) = result else { panic!("Det accepted {operation}") };
                assert_eq!(err.code, Code::E504, "{err:?}");
                assert!(err.msg.contains(diagnostic), "{err:?}");
            } else {
                assert!(result.is_ok(), "{:?}", result.err());
            }
        }
    }
}
