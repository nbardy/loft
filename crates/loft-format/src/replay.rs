//! Fixed-tick host input recording. A replay driver supplies state and verdicts for its own backend.
use crate::bundle::{
    BundleError,
    wire::{Reader, Wire, fields, variants},
};
use std::fmt;

const MAGIC: &[u8; 8] = b"LOFTLOG1";

#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Level {
        name: String,
        value: [f32; 4],
    },
    Event {
        name: String,
        payload: [f32; 4],
    },
    PosePacket {
        name: String,
        bytes: Vec<u8>,
    },
    Param {
        name: String,
        value: [f32; 4],
    },
    /// Terminal tick, including when the preceding input stream was empty.
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoggedInput {
    pub tick: u32,
    pub seq: u32,
    pub input: Input,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoftLog {
    pub format_version: u32,
    pub bundle_hash: [u8; 32],
    pub device_signature: String,
    pub dt_s: f32,
    pub initial_seed: u64,
    pub params: Vec<(String, [f32; 4])>,
    pub checkpoints_every: u32,
    pub inputs: Vec<LoggedInput>,
    pub expected_final_identity: Option<[u8; 32]>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReplayError {
    Encoding(BundleError),
    Version,
    Metadata,
    InputOrder,
    Bundle,
    Identity,
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encoding(e) => write!(f, "invalid .loftlog data ({e})"),
            Self::Version => f.write_str("unsupported .loftlog version"),
            Self::Metadata => f.write_str("invalid .loftlog metadata or input"),
            Self::InputOrder => {
                f.write_str(".loftlog requires increasing (tick, seq) and one final End")
            }
            Self::Bundle => f.write_str(".loftlog belongs to a different bundle"),
            Self::Identity => f.write_str(".loftlog final state identity differs"),
        }
    }
}
impl std::error::Error for ReplayError {}

impl LoftLog {
    /// Append a host update before the corresponding tick is advanced.
    pub fn record(&mut self, tick: u32, input: Input) -> Result<(), ReplayError> {
        let seq = match self.inputs.last() {
            Some(previous) if previous.input == Input::End || previous.tick > tick => {
                return Err(ReplayError::InputOrder);
            }
            Some(previous) if previous.tick == tick => {
                previous.seq.checked_add(1).ok_or(ReplayError::InputOrder)?
            }
            _ => 0,
        };
        self.inputs.push(LoggedInput { tick, seq, input });
        Ok(())
    }

    pub fn validate(&self) -> Result<u32, ReplayError> {
        if self.format_version != 1 {
            return Err(ReplayError::Version);
        }
        let mut names = std::collections::BTreeSet::new();
        if !self.dt_s.is_finite()
            || self.dt_s <= 0.
            || self.device_signature.is_empty()
            || self
                .params
                .iter()
                .any(|(name, value)| name.is_empty() || !names.insert(name) || !finite(value))
        {
            return Err(ReplayError::Metadata);
        }
        let (end, inputs) = self.inputs.split_last().ok_or(ReplayError::InputOrder)?;
        if end.input != Input::End
            || inputs.iter().any(|i| i.input == Input::End)
            || self
                .inputs
                .windows(2)
                .any(|w| (w[0].tick, w[0].seq) >= (w[1].tick, w[1].seq))
        {
            return Err(ReplayError::InputOrder);
        }
        for item in inputs {
            let valid = match &item.input {
                Input::Level { name, value } | Input::Param { name, value } => {
                    !name.is_empty() && finite(value)
                }
                Input::Event { name, payload } => !name.is_empty() && finite(payload),
                Input::PosePacket { name, .. } => !name.is_empty(),
                Input::End => unreachable!(),
            };
            if !valid {
                return Err(ReplayError::Metadata);
            }
        }
        Ok(end.tick)
    }

    pub fn encode(&self) -> Result<Vec<u8>, ReplayError> {
        self.validate()?;
        let mut out = MAGIC.to_vec();
        self.put(&mut out);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ReplayError> {
        let mut reader = Reader::new(bytes);
        if reader.take(8).map_err(ReplayError::Encoding)? != MAGIC {
            return Err(ReplayError::Version);
        }
        let log = Self::get(&mut reader).map_err(ReplayError::Encoding)?;
        reader.finish().map_err(ReplayError::Encoding)?;
        log.validate()?;
        Ok(log)
    }

    /// Visit tick zero and every subsequent tick, applying updates before the step.
    pub fn ticks(
        &self,
        bundle_hash: [u8; 32],
    ) -> Result<impl Iterator<Item = (u32, &[LoggedInput])>, ReplayError> {
        if self.bundle_hash != bundle_hash {
            return Err(ReplayError::Bundle);
        }
        let end = self.validate()?;
        let mut remaining = &self.inputs[..self.inputs.len() - 1];
        Ok((0..=end).map(move |tick| {
            let split = remaining.partition_point(|entry| entry.tick == tick);
            let (current, tail) = remaining.split_at(split);
            remaining = tail;
            (tick, current)
        }))
    }

    /// Tier 0 and same-backend replay can verify the complete state bytes by identity.
    pub fn verify_identity(&self, state: &[u8]) -> Result<(), ReplayError> {
        if self
            .expected_final_identity
            .is_some_and(|hash| hash != identity(state))
        {
            return Err(ReplayError::Identity);
        }
        Ok(())
    }
}

pub fn identity(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

fn finite(value: &[f32; 4]) -> bool {
    value.iter().all(|x| x.is_finite())
}

fields!(LoftLog {
    format_version,
    bundle_hash,
    device_signature,
    dt_s,
    initial_seed,
    params,
    checkpoints_every,
    inputs,
    expected_final_identity,
});
fields!(LoggedInput { tick, seq, input });
variants!(Input {
    0 => Level { name, value },
    1 => Event { name, payload },
    2 => PosePacket { name, bytes },
    3 => Param { name, value },
    4 => End,
});
