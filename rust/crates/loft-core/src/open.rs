//! One kind switch per reading, shared by graph functions and pass-local WGSL.
use crate::unsupported;
use loft_ir::{Error, pass::Open};

pub(crate) fn dispatch(open: Open) -> Result<String, Error> {
    let mut cases = open.cases;
    cases.sort_by_key(|c| c.0);
    if cases.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(unsupported(format!("{} has duplicate cases; assign each kind a distinct tag", open.signature)));
    }
    let mut text = format!("\n{} {{\n  switch {} {{\n", open.signature, open.selector);
    for (tag, expr) in cases {
        text.push_str(&format!("    case {tag}u: {{ return {expr}; }}\n"));
    }
    text.push_str(&format!("    default: {{ return {}; }}\n  }}\n}}\n", open.otherwise));
    Ok(text)
}

pub fn loader(k: loft_ir::pass::RowType) -> Result<String, Error> {
    use loft_ir::pass::WordType;
    let mut offset = 0;
    let mut args = Vec::new();
    for field in k.fields {
        if !(1..=4).contains(&field.width) {
            return Err(unsupported(format!("{}: choose a scalar or vector of 2 to 4 words", k.name)));
        }
        let words = (0..field.width)
            .map(|_| {
                let read = format!("{}[at + {offset}u]", k.storage);
                offset += 1;
                read
            })
            .collect::<Vec<_>>();
        let value = match words.as_slice() {
            [scalar] => scalar.clone(),
            _ => format!("vec{}u({})", field.width, words.join(", ")),
        };
        let arg = match field.ty {
            WordType::U32 => value,
            WordType::F32 => {
                let ty = if field.width == 1 { "f32".into() } else { format!("vec{}f", field.width) };
                format!("bitcast<{ty}>({value})")
            }
        };
        args.push(arg);
    }
    Ok(format!(
        "
fn load_{0}(at: u32) -> {0} {{ return {0}({1}); }}
",
        k.name,
        args.join(", ")
    ))
}

/// Link exactly the same row accessors and case switches for a device module or a CPU observer.
pub fn source(src: loft_ir::pass::Source) -> Result<String, Error> {
    let mut text = src.text;
    for kind in src.kinds {
        text.push_str(&loader(kind)?);
    }
    for open in src.opens {
        text.push_str(&dispatch(open)?);
    }
    Ok(text)
}
