//! CPU-only corpus build and bundle writing.

use crate::{budget::Failure, flag, sources};
use loft_cli::{artifact, write};
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
