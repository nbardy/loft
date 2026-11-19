//! How loft-lower ends: the CLI's verdicts (src/cli/verdict.ts) with its exit codes, plus `Unported`, a module no lane
//! has translated yet. The last stderr line is `loft: <kind>: <head>`, as `loft build` prints it, so tools/port/diff.ts
//! compares a refusal with the TS compiler's by verdict and head.
use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// A module of the TS compiler not translated yet: its TS source and the lane that owns it.
    Unported { module: &'static str, lane: &'static str },
    /// The scene refused, as the TS compiler refuses it (src/errors.ts SceneError): exit 4.
    Scene(String),
    /// The input directory is not what src/record/port.ts writes.
    Input(String),
    /// A Loft bug.
    Internal(String),
}

impl Error {
    pub fn code(&self) -> i32 {
        match self {
            Error::Scene(_) => 4,
            Error::Input(_) => 3,
            Error::Unported { .. } | Error::Internal(_) => 1,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unported { module, lane } => write!(f, "loft: internal: unported: {module} (lane {lane})"),
            Error::Scene(head) => write!(f, "loft: scene: {head}"),
            Error::Input(why) => write!(f, "loft: load: {why}"),
            Error::Internal(why) => write!(f, "loft: internal: {why}"),
        }
    }
}

impl From<loft_ir::Error> for Error {
    fn from(e: loft_ir::Error) -> Self {
        Error::Input(format!("the job does not decode: {e:?}"))
    }
}
