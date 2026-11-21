#[derive(Debug, PartialEq, Eq)]
pub enum RunnerError {
    UnknownInstance,
    UnknownResource,
    AbiMismatch,
    Readback,
    Bundle(loft_format::bundle::BundleError),
    Capacity,
    InvalidShader(String),
    Unsupported(&'static str),
    TextureAccess { format: String, access: String },
}
impl From<crate::bundle::InvalidResources> for RunnerError {
    fn from(error: crate::bundle::InvalidResources) -> Self {
        use crate::bundle::InvalidResources;
        match error {
            InvalidResources::Layout => Self::AbiMismatch,
            InvalidResources::AttachmentCount => Self::Unsupported("multiple host attachments of one kind"),
        }
    }
}
pub type Result<T> = std::result::Result<T, RunnerError>;
impl From<crate::events::InvalidOutbox> for RunnerError {
    fn from(_: crate::events::InvalidOutbox) -> Self {
        Self::AbiMismatch
    }
}
