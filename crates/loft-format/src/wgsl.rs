//! Parse and validate WGSL with the caller's capabilities and diagnostic policy.
use naga::{Module, WithSpan, front::wgsl::ParseError, valid::{ModuleInfo, ValidationError, Validator}};

#[derive(Debug)]
pub enum Error {
    Parse(ParseError),
    Validate(WithSpan<ValidationError>),
}

impl Error {
    pub fn emit_to_string(&self, source: &str) -> String {
        match self {
            Self::Parse(error) => error.emit_to_string(source),
            Self::Validate(error) => error.emit_to_string(source),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(error) => std::fmt::Display::fmt(error, f),
            Self::Validate(error) => std::fmt::Display::fmt(error, f),
        }
    }
}

pub fn validate(source: &str, validator: &mut Validator) -> Result<(Module, ModuleInfo), Error> {
    let module = naga::front::wgsl::parse_str(source).map_err(Error::Parse)?;
    let info = validator.validate(&module).map_err(Error::Validate)?;
    Ok((module, info))
}
