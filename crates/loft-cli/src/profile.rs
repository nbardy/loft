//! l23a owns this command.
use crate::budget::Failure;
pub(crate) fn run(_args:&[String])->Result<(),Failure>{
    Err(Failure::Usage("loft profile is reserved for l23a".into()))
}
