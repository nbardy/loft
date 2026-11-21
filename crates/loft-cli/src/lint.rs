use crate::budget::Failure;
use loft_parse::cst::{Cst, Kind};
use std::{collections::HashMap, fs, path::Path};

const TOKEN_CAP: usize = 10_000;
const BLOCK_TOKENS: usize = 32;
const BLOCK_REPEATS: usize = 8;

pub(crate) fn run(args: &[String]) -> Result<(), Failure> {
    let strict = args.iter().any(|arg| arg == "--strict");
    let inputs: Vec<_> = args.iter().filter(|arg| *arg != "--strict").collect();
    if inputs.is_empty() || inputs.iter().any(|arg| arg.starts_with('-')) {
        return Err(Failure::Usage("usage: loft lint [--strict] SCENE.loft ...".into()));
    }
    let mut warnings = 0;
    for input in inputs {
        warnings += lint(Path::new(input))?;
    }
    if strict && warnings != 0 {
        return Err(Failure::Budget(format!("{warnings} lint warnings")));
    }
    Ok(())
}

fn lint(path: &Path) -> Result<usize, Failure> {
    let source = fs::read_to_string(path)
        .map_err(|error| Failure::Usage(format!("{}: {error}", path.display())))?;
    let cst = Cst::parse(0, &source)
        .map_err(|error| Failure::Usage(format!("{}: {}", path.display(), error.msg)))?;
    let tokens = cst.tokens();
    let tokens = &tokens[..tokens.len() - 1]; // The CST ends with the EOF leaf.
    let mut warnings = 0;
    if tokens.len() > TOKEN_CAP {
        eprintln!("{}: warning: {} Loft tokens exceeds the {TOKEN_CAP} token budget", path.display(), tokens.len());
        warnings += 1;
    }

    // Anchor each window on a literal and count non-overlapping repeats.
    // Keep identifiers and punctuation exact, while allowing samples to vary.
    let keys: Vec<_> = tokens.iter().map(|token| {
        if token.kind == Kind::Number { "#" } else { token.text(&source) }
    }).collect();
    let mut blocks = HashMap::new();
    for (start, block) in keys.windows(BLOCK_TOKENS).enumerate() {
        if block[0] != "#" || block.iter().filter(|&&key| key == "#").count() < 6 {
            continue;
        }
        let (end, count) = blocks.entry(block).or_insert((0, 0));
        if start >= *end {
            *end = start + BLOCK_TOKENS;
            *count += 1;
        }
        if *count == BLOCK_REPEATS {
            let line = source.as_bytes()[..tokens[start].span.lo as usize].iter()
                .filter(|&&byte| byte == b'\n').count() + 1;
            eprintln!("{}:{line}: warning: repeated {BLOCK_TOKENS}-token literal block; consider a build loop or binary sampled table", path.display());
            warnings += 1;
            break;
        }
    }
    Ok(warnings)
}
