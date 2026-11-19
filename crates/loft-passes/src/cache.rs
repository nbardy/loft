//! Local-space interval certificates. A placement is applied after this cache;
//! geometry functions, baked frames and the columns they read determine the answer.
use crate::{
    Prog, emit,
    eval::{Lut, RowVal},
    lip::Derived,
};
use loft_ir::{Code, Diag, Res, Span, ast::*};
use std::{
    io::{ErrorKind, Write},
    path::Path,
    sync::OnceLock,
};

fn error(sp: Span, e: impl std::fmt::Display) -> Diag {
    Diag::new(
        Code::E701,
        sp,
        format!("derivation cache: {e}"),
        "check permissions and free space in target/loft_cache",
    )
}
fn semantic(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(o)
            if o.contains_key("file") && o.contains_key("lo") && o.contains_key("hi") =>
        {
            o.clear()
        }
        serde_json::Value::Object(o) => {
            o.remove("id");
            o.remove("span");
            o.remove("text");
            for v in o.values_mut() {
                semantic(v)
            }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(semantic),
        _ => {}
    }
}
pub(crate) fn kind_key(p: &Prog, k: &KindDecl, luts: &[Lut]) -> Res<blake3::Hasher> {
    static ABI: OnceLock<blake3::Hash> = OnceLock::new();
    let abi = ABI.get_or_init(|| {
        blake3::hash(
            concat!(
                "local-derivation-v1",
                include_str!("eval.rs"),
                include_str!("lip.rs"),
                include_str!("check.rs")
            )
            .as_bytes(),
        )
    });
    let roots: Vec<_> = k
        .queries
        .iter()
        .filter(|(q, _)| *q != QueryKind::Material)
        .map(|(_, f)| f)
        .collect();
    let (helpers, names) = emit::reachable(p, &roots, &[]);
    let constants: Vec<_> = names.iter().map(|n|p.consts[n.as_str()]).collect();
    let mut ir = serde_json::to_value((&k.name.name, &k.row, roots, helpers, constants))
        .map_err(|e| error(k.name.span, e))?;
    semantic(&mut ir);
    let mut h = blake3::Hasher::new();
    h.update(abi.as_bytes());
    h.update(&serde_json::to_vec(&ir).map_err(|e| error(k.name.span, e))?);
    for l in luts {
        h.update(&(l.name.len() as u64).to_le_bytes());
        h.update(l.name.as_bytes());
        h.update(&l.lo.to_le_bytes());
        h.update(&l.hi.to_le_bytes());
        h.update(&(l.frames.len() as u64).to_le_bytes());
        for frame in &l.frames {
            for x in frame {
                h.update(&x.to_le_bytes());
            }
        }
    }
    Ok(h)
}

pub(crate) fn derive(
    domain: &blake3::Hasher,
    fields: &[&RowVal],
    sp: Span,
    compute: impl FnOnce() -> Res<Derived>,
) -> Res<Derived> {
    let mut key = domain.clone();
    for f in fields {
        let mut words = vec![];
        crate::pack(std::slice::from_ref(*f), &mut words);
        key.update(&(words.len() as u32).to_le_bytes());
        for w in words {
            key.update(&w.to_bits().to_le_bytes());
        }
    }
    let key = key.finalize();
    let dir = Path::new("target/loft_cache");
    let path = dir.join(format!("deriv_{key}.bin"));
    // The checksum covers both the key and seven f64s; a torn or misplaced file is a miss.
    let checksum = |b: &[u8]| blake3::keyed_hash(key.as_bytes(), b);
    match std::fs::read(&path) {
        Ok(b) if b.len() == 88 && checksum(&b[..56]).as_bytes() == &b[56..] => {
            let word = |n: usize| f64::from_le_bytes(b[8 * n..8 * n + 8].try_into().unwrap());
            return Ok(Derived {
                lo: [word(0), word(1), word(2)],
                hi: [word(3), word(4), word(5)],
                l: word(6),
                evals: 0,
            });
        }
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(error(sp, e)),
    }
    let d = compute()?;
    let mut bytes = vec![];
    for x in d.lo.into_iter().chain(d.hi).chain([d.l]) {
        bytes.extend(x.to_le_bytes());
    }
    bytes.extend(*checksum(&bytes).as_bytes());
    std::fs::create_dir_all(dir).map_err(|e| error(sp, e))?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|e| error(sp, e))?;
    tmp.write_all(&bytes)
        .and_then(|()| tmp.flush())
        .map_err(|e| error(sp, e))?;
    tmp.persist(&path).map_err(|e| error(sp, e))?;
    Ok(d)
}
