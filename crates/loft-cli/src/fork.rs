//! Phase 2 authoring owns this command.
use crate::budget::Failure;
pub(crate) fn run(_args:&[String])->Result<(),Failure>{
    Err(Failure::Usage("loft fork is reserved for Phase 2 authoring".into()))
}
