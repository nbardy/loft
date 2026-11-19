pub(super) use loft_std::inputs::Inputs;
impl From<loft_std::inputs::Error> for super::RunnerError {
    fn from(error: loft_std::inputs::Error) -> Self {
        use loft_std::inputs::Error;
        match error {
            Error::Capacity => Self::Capacity,
            Error::AbiMismatch => Self::AbiMismatch,
            Error::UnknownResource => Self::UnknownResource,
        }
    }
}
