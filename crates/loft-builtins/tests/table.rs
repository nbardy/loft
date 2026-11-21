use loft_builtins::{BUILTINS, SigRule, lookup, spec};
use std::collections::HashSet;
use std::path::Path;

#[test]
fn each_variant_and_spelling_has_one_row() {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    // The catalog declaration also generates the enum, in discriminant order.
    // Its exhaustive spec match makes an extra enum variant a compile error.
    for (index, row) in BUILTINS.iter().enumerate() {
        assert_eq!(row.id as usize, index);
        assert!(ids.insert(row.id), "duplicate variant {:?}", row.id);
        assert!(!row.name.is_empty());
        assert!(names.insert(row.name), "duplicate name {}", row.name);
        assert_eq!(spec(row.id), row);
    }
}

#[test]
fn names_round_trip() {
    for row in BUILTINS {
        assert_eq!(lookup(row.name), Some(row));
    }
    assert_eq!(lookup(""), None);
    assert_eq!(lookup("not_a_loft_builtin"), None);
}

#[test]
fn arity_ranges_are_valid() {
    for row in BUILTINS {
        assert!(row.min_arity <= row.max_arity, "{}", row.name);
        if let SigRule::Fixed(args, _) = row.sig {
            assert_eq!(usize::from(row.min_arity), args.len(), "{}", row.name);
            assert_eq!(usize::from(row.max_arity), args.len(), "{}", row.name);
        }
    }
}

#[test]
fn source_fits_line_budget() {
    fn count_lines(path: &Path) -> usize {
        if path.is_dir() {
            return std::fs::read_dir(path)
                .unwrap()
                .map(|entry| count_lines(&entry.unwrap().path()))
                .sum();
        }
        std::fs::read_to_string(path).unwrap().lines().count()
    }

    let lines = count_lines(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    assert!(lines <= 550, "{lines} physical lines under src/");
}
