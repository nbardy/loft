//! Path schedule records shared by the compiler passes.
// Keep the existing verification imports while the CPU oracles live with the instruments.
#[path = "../../../tools/loft-verify/src/path_twin.rs"]
pub mod cpu;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum Integrator {
    Reference,
    Spectral,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct WorldResources {
    pub kinds: Vec<u32>,
    pub media: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PathTrace {
    pub integrator: Integrator,
    pub bounces: u32,
    #[serde(default)]
    pub lens: [f32; 2],
    pub lamps: Vec<[[f32; 4]; 2]>,
    #[serde(default)]
    pub worlds: Vec<WorldResources>,
    #[serde(default)]
    pub media: Vec<[[f32; 4]; 5]>,
}
