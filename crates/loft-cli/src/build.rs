//! CPU-only corpus build and bundle writing.
mod artifact;
pub(crate) mod trace;

use crate::{budget::Failure, flag, sources};
use std::path::Path;

pub(crate) fn run(args: &[String], set: &[(String, f64)]) -> Result<(), Failure> {
    let mut args = args.to_vec();
    let output = flag(&mut args, "-o")?;
    let [scene] = args.as_slice() else {
        return Err(Failure::Usage(
            "usage: loft build SCENE [-o OUT.loftb]".into(),
        ));
    };
    let src = sources(Path::new(scene)).map_err(Failure::Usage)?;
    let (program, _, compiled) =
        loft_passes::compile(&src, set).map_err(|d| Failure::Compile(d.render(&src)))?;
    if let Some(path) = output {
        let bundle = artifact::lower(&program, &compiled).map_err(Failure::Compile)?;
        write(Path::new(&path), &loft_runner::bundle::encode(&bundle))?;
    }
    println!("built {} modules", program.modules.len());
    Ok(())
}

pub(super) fn write(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
    use std::io::Write;
    let err = |e: std::io::Error| Failure::Usage(format!("{}: {e}", path.display()));
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(err)?;
    file.write_all(bytes).map_err(err)?;
    file.persist(path).map_err(|e| err(e.error))?;
    Ok(())
}

fn live_names<'a>(prog: &'a loft_passes::Prog<'_>) -> impl Iterator<Item = &'a str> {
    use loft_parse::ast::TyExpr;
    prog.consts.iter().filter_map(|(&name, c)| {
        (c.comptime && matches!(&c.ty, Some(TyExpr::Name(n)) if n.name == "f32")).then_some(name)
    })
}
