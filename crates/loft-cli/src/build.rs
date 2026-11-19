//! l5g owns this command.
use crate::budget::Failure;
pub(crate) fn run(_args:&[String])->Result<(),Failure>{
    Err(Failure::Usage("loft build is reserved for l5g".into()))
}
