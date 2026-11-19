//! CPU-only corpus build. L5g supplies serialization for the -o form.
use crate::{budget::Failure, sources};
use std::path::Path;

pub(crate) fn run(args: &[String], set: &[(String, f64)]) -> Result<(), Failure> {
    let [scene] = args else {
        return Err(Failure::Usage("usage: loft build SCENE (bundle output awaits L5g)".into()));
    };
    let src = sources(Path::new(scene)).map_err(Failure::Usage)?;
    let (program, _, _) = loft_passes::compile(&src, set).map_err(|d| Failure::Compile(d.render(&src)))?;
    println!("built {} modules", program.modules.len());
    Ok(())
}
