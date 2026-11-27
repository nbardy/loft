use loft_parse::ast::{ExprKind, Item, NumSuffix};
use loft_parse::cst::{Cst, Kind, Trivia};
use std::{fs, path::Path};

fn replay(tree: &Cst, source: &str) -> String {
    let mut out = String::new();
    for token in tree.tokens() {
        for (_, span) in &token.leading {
            out.push_str(&source[span.lo as usize..span.hi as usize]);
        }
        out.push_str(token.text(source));
        for (_, span) in &token.trailing {
            out.push_str(&source[span.lo as usize..span.hi as usize]);
        }
    }
    out
}

#[test]
fn preserves_comment_attachment_and_raw_braces() {
    let src = "# leading λ\r\nfn f(x: f) = wgsl (x: f) -> f { /* } /* { */ } */ if(x>0){return x;} // }\n return 0.; } # trailing\r\n";
    let tree = Cst::parse(0, src).unwrap();
    assert_eq!(replay(&tree, src), src);
    let raw = tree
        .tokens()
        .into_iter()
        .find(|t| t.kind == Kind::Raw)
        .unwrap();
    assert!(raw.trailing.iter().any(|(k, _)| *k == Trivia::Comment));
}

#[test]
fn all_authored_files_replay_verbatim() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    fn replay_dir(dir: &Path) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                replay_dir(&path);
            } else if path.extension().is_some_and(|ext| ext == "loft") {
                let source = fs::read_to_string(&path).unwrap();
                assert_eq!(
                    replay(&Cst::parse(0, &source).unwrap(), &source),
                    source,
                    "{}",
                    path.display()
                );
            }
        }
    }
    // The authored library moved into nested modules under stdlib/.
    for dir in ["stdlib", "scenes", "engines", "agent_notes/2026-11-20_meta/v1_syntax"] {
        replay_dir(&root.join(dir));
    }
}

#[test]
fn unit_powers_and_reciprocals_are_accepted_in_type_position() {
    loft_parse::parse(0, "fn convert(x: 1/kg, y: m^2/s^2) -> J = x * y", &mut 0).unwrap();
    loft_parse::parse(0, "fn divide(s: f) -> f = 1e2/s", &mut 0).unwrap();
}

#[test]
fn unit_literals_keep_their_scale_and_spelling() {
    let source = "const radius = 0.38m\nconst angle = 40deg\nconst duration = 24s\nconst speed = 120mm/s";
    let module = loft_parse::parse(0, source, &mut 0).unwrap();
    assert_eq!(module.items.len(), 4);
    let expected = [("0.38m", 0.38), ("40deg", 40_f64.to_radians()), ("24s", 24.0), ("120mm/s", 0.12)];
    for (item, (spelling, scaled)) in module.items.iter().zip(expected) {
        let Item::Const(decl) = item else { panic!("expected a constant") };
        let ExprKind::Num { value, suffix: NumSuffix::Unit, text, float: true } = &decl.value.kind else {
            panic!("expected a floating-point unit literal");
        };
        assert_eq!(text, spelling);
        assert_eq!(*value as f32, scaled as f32);
    }
}
