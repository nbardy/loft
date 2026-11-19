//! l4c owns this command.
use crate::budget::Failure;
pub(crate) fn run(_args:&[String])->Result<(),Failure>{
    Err(Failure::Usage("loft render is reserved for l4c".into()))
}
