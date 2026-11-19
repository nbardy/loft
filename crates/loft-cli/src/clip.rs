//! Phase 4 uses the same AOV selection as render. Phase 5 owns the training gate.
use crate::{budget::Failure, render::{self, Output}};

pub(crate) fn run(args: &[String], overrides: &[(String, f64)]) -> Result<(), Failure> {
    render::run(args, overrides, Output::Clip)
}
