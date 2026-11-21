use std::path::Path;

#[test]
fn parser_accepts_the_decoder_corpus_and_rejects_the_same_fixture() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut folders = vec![root.join("stdlib"), root.join("agent_notes/2026-11-20_meta/v1_syntax")];
    let mut paths = Vec::new();
    while let Some(folder) = folders.pop() {
        for item in std::fs::read_dir(folder).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|e| e == "loft") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    let mut count = 0;
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap();
        loft_parse::parse(0, &text, &mut 0).unwrap_or_else(|d| panic!("{}: {d:?}", path.display()));
        count += 1;
    }
    assert!(count >= 29);
    loft_parse::parse(0, include_str!("fixtures/nested-imports.loft"), &mut 0).unwrap();
    loft_parse::parse(0, include_str!("fixtures/review-accept.loft"), &mut 0).unwrap();
    assert!(loft_parse::parse(0, include_str!("fixtures/review-reject.invalid"), &mut 0).is_err());
}

#[test]
fn authoring_guide_uses_the_native_parser() {
    let guide = include_str!("../../../docs/context.md");
    let mut blocks=0;
    for block in guide.split("```loft\n").skip(1) {
        loft_parse::parse(0,block.split_once("\n```").unwrap().0,&mut 0).unwrap();
        blocks+=1;
    }
    assert!(blocks>0);
}
