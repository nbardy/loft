//! Phase 5 training gate owns this command.
use crate::budget::Failure;
pub(crate) fn run(_args:&[String])->Result<(),Failure>{
    Err(Failure::Usage("loft clip is reserved for Phase 5 training gate".into()))
}
