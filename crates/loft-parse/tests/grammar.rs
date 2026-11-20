use std::path::Path;

#[test]
fn parser_accepts_the_decoder_corpus_and_rejects_the_same_fixture() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let folders = ["lib", "engines", "agent_notes/2026-11-20_meta/v1_syntax"];
    let paths = folders.into_iter().flat_map(|dir| std::fs::read_dir(root.join(dir)).unwrap())
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "loft"));
    let mut count = 0;
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap();
        loft_parse::parse(0, &text, &mut 0).unwrap_or_else(|d| panic!("{}: {d:?}", path.display()));
        count += 1;
    }
    assert!(count >= 14);
    loft_parse::parse(0, include_str!("fixtures/review-accept.loft"), &mut 0).unwrap();
    assert!(loft_parse::parse(0, include_str!("fixtures/review-reject.invalid"), &mut 0).is_err());
}

#[test]
fn author_card_uses_the_native_parser() {
    let card=include_str!("../../../AGENT_CARD.md");
    let mut blocks=0;
    for block in card.split("```loft\n").skip(1) {
        loft_parse::parse(0,block.split_once("\n```").unwrap().0,&mut 0).unwrap();
        blocks+=1;
    }
    assert!(blocks>0);
}

#[test]
fn forward_kind_helper_shadows_a_builtin_only_inside_its_kind() {
    use loft_parse::ast::{ExprKind, Item, QueryKind, Stmt};

    let source = "space World\n\
        kind Scoped @World () { trace(p) = sin(0.0); fn sin(x: f) -> f = cos(x) }\n\
        fn outside(x: f) -> f = sin(x)";
    let module = loft_parse::parse(0, source, &mut 0).unwrap();
    let [Item::Space(_), Item::Fn(helper), Item::Kind(kind), Item::Fn(outside)] =
        module.items.as_slice()
    else {
        panic!("expected the helper to be lifted before its kind");
    };
    for (function, expected) in [
        (kind.query(QueryKind::Trace).unwrap(), "Scoped#sin"),
        (helper, "cos"),
        (outside, "sin"),
    ] {
        let [Stmt::Return(Some(expr), _)] = function.body.stmts.as_slice() else {
            panic!("expected a call in {}", function.name.name);
        };
        let ExprKind::Call { callee, .. } = &expr.kind else {
            panic!("expected a call in {}", function.name.name);
        };
        assert_eq!(callee.name, expected);
    }
}
