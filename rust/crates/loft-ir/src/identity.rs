//! One identity (plan §1.9): the bundle file's hash, the frame asked for, and the device that drew it. The same
//! `Identity` gives the same frame bytes on that device.
use crate::bundle::Id;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Identity {
    /// sha256 of the bundle file: variant and style are inside it.
    pub build: [u8; 32],
    pub frame: FrameKey,
    pub device: Device,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FrameKey {
    /// Digest of the frames already observed by a Previous read, in presentation order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<[u8; 32]>,
    pub camera: Id,
    /// f32 bits of the time.
    pub time: u32,
    pub size: [u32; 2],
    /// f32 bits per param, then per input component, in the bundle's order.
    pub params: Box<[u32]>,
    pub inputs: Box<[u32]>,
    /// What ran, not what was asked.
    pub precision: Precision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Precision {
    F32,
    Mixed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Device {
    pub backend: Backend,
    pub adapter: Box<str>,
    pub driver: Box<str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Backend {
    Metal,
    Vulkan,
    Dx12,
    Gl,
    WebGpu,
}

/// sha256 of a bundle file's bytes: `Identity::build`.
pub fn build(bundle: &[u8]) -> [u8; 32] {
    Sha256::digest(bundle).into()
}

impl Identity {
    /// sha256 of the identity's JSON. JSON with fixed field order and quoted strings is injective, so any field change
    /// changes the hash.
    pub fn hash(&self) -> [u8; 32] {
        // Plain structs of integers, arrays and strings always serialise; a failure is a Loft bug, so it panics
        // (Internal at the binding) rather than hash a placeholder.
        Sha256::digest(serde_json::to_vec(self).expect("an Identity serialises")).into()
    }
}

impl FrameKey {
    /// Includes the previous digest, so two orders of the same observations remain distinct.
    pub fn prefix(&self) -> [u8; 32] {
        Sha256::digest(serde_json::to_vec(self).expect("a FrameKey serialises")).into()
    }
}
