//! Exit statuses shared by the CLI operations and their instruments.
use loft_runner::{Gpu, Loaded};

#[derive(Debug)]
pub enum Failure {
    Compile(String),
    Truth(String),
    Parity(String),
    Load(String),
    Usage(String),
    Device(String),
    Budget(String),
}

impl Failure {
    pub fn exit(self) -> (u8, String) {
        match self {
            Self::Compile(e) => (1, e),
            Self::Truth(e) => (2, e),
            Self::Parity(e) => (3, e),
            Self::Load(e) => (4, e),
            Self::Usage(e) => (5, e),
            Self::Device(e) => (6, e),
            Self::Budget(e) => (7, e),
        }
    }
}

pub fn timestamps(available: bool) -> Result<(), Failure> {
    if available {
        Ok(())
    } else {
        Err(Failure::Device("adapter does not support GPU timestamps".into()))
    }
}

/// Measure the frame the command submitted, before attempting to read its pixels.
pub fn check(g: &Gpu, loaded: &Loaded, budget: f64) -> Result<(), Failure> {
    timestamps(g.timestamps)?;
    let (ms, _) = loaded.gpu_ms(g).map_err(Failure::Device)?
        .ok_or_else(|| Failure::Device("GPU timestamps are unavailable".into()))?;
    if ms.is_finite() && ms <= budget {
        println!("GPU frame {ms:.3} ms <= {budget:.3} ms");
        Ok(())
    } else {
        Err(Failure::Budget(format!("GPU frame {ms:.3} ms exceeds {budget:.3} ms")))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn missing_timestamps_are_an_environment_failure() {
        assert_eq!(super::timestamps(false).unwrap_err().exit().0, 6);
    }
}
