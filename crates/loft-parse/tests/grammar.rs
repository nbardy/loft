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
    assert!(loft_parse::parse(0, include_str!("fixtures/review-reject.loft"), &mut 0).is_err());
}
