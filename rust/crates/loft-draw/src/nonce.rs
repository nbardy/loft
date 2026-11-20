//! A cold profile changes a live predicate in each bundle pipeline, including the lens and load fills.
//! The device cannot dispatch 2^24 groups on one axis and this executor uses at most four coverage samples.
use crate::Verdict;
use loft_ir::bundle::{Bundle, Format};

pub fn apply(bytes: &[u8], nonce: u32) -> Result<Vec<u8>, Verdict> {
    let mut b = Bundle::read(bytes)?;
    if !matches!(b.format, Format::V2(_)) || nonce < 1 << 24 || nonce == u32::MAX {
        return Err(Verdict::Usage(
            "cold nonce: use an ordered program and a distinct integer in 16777216..4294967294".into(),
        ));
    }
    for m in &b.modules {
        let slot = &mut b.sections[m.wgsl as usize];
        let mut text = String::from_utf8(slot.to_vec()).map_err(|_| Verdict::Load("invalid WGSL text".into()))?;
        // Locate the declared stage, even when other attributes occur before its function.
        for (stage, attribute, ty, condition, action) in [
            ("@fragment", "sample_mask", "u32", "loft_profile_key", "discard"),
            ("@compute", "num_workgroups", "vec3u", "loft_profile_key.x", "return"),
        ] {
            let mut at = 0;
            while let Some(start) = text[at..].find(stage) {
                let start = at + start;
                let fail = || {
                    Verdict::Usage(format!(
                        "cold nonce: could not instrument {stage} entry; profile the original module without a nonce"
                    ))
                };
                let fun = start + text[start..].find("fn ").ok_or_else(fail)?;
                let arg = fun + text[fun..].find('(').ok_or_else(fail)? + 1;
                let body = arg + text[arg..].find('{').ok_or_else(fail)? + 1;
                let comma = if text[arg..].trim_start().starts_with(')') { "" } else { "," };
                let param = format!("@builtin({attribute}) loft_profile_key: {ty}{comma}");
                if text[fun..body].contains(&format!("@builtin({attribute})")) {
                    return Err(fail());
                }
                let guard = format!("\nif {condition} == {nonce}u {{ {action}; }}\n");
                text.insert_str(body, &guard);
                text.insert_str(arg, &param);
                at = body + guard.len() + param.len();
            }
        }
        *slot = text.into_bytes().into();
    }
    Ok(b.write()?)
}
