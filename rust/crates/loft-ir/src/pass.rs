//! An ordered program. Tables use the bundle's packs. Images follow the output extent.
//! Read bindings start at group 2 binding 0 in declaration order.
use crate::bundle::{Blend, Clear, Cull, Id, TexFormat};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    pub images: Vec<Image>,
    pub passes: Vec<Pass>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Image {
    pub format: TexFormat,
    pub samples: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Read {
    Now(Id),
    Previous(Id),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Destination {
    Image(Id),
    Output,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Write {
    pub target: Destination,
    pub clear: Clear,
    pub blend: Blend,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Count {
    Fixed(u32),
    Rows(Id),
    Indirect(Id),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Draw {
    pub vertex: String,
    pub fragment: String,
    pub vertices: u32,
    pub instances: Count,
    pub cull: Cull,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Run {
    Raster { draws: Vec<Draw> },
    Compute { entry: String, extent: [u32; 3] },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pass {
    pub name: String,
    pub module: Id,
    pub run: Run,
    pub reads: Vec<Read>,
    pub writes: Vec<Write>,
    pub repeat: u32,
}

impl Schedule {
    pub fn draws(&self) -> usize {
        self.passes
            .iter()
            .map(|p| match &p.run {
                Run::Raster { draws } => draws.len() * p.repeat as usize,
                Run::Compute { .. } => 0,
            })
            .sum()
    }
}

/// Wire 5 appends a declaration span. Only the linked modules and resolved plan persist.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Declarations {
    pub tables: Vec<Table>,
    pub modules: Vec<Source>,
    pub schedule: Schedule,
}
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Table {
    Data { lanes: u16, words: Vec<u32> },
    Made { lanes: u16, rows: u32, module: Id, entry: String, extent: [u32; 3] },
}
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Source {
    pub text: String,
    pub opens: Vec<Open>,
}
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Open {
    pub signature: String,
    pub selector: String,
    pub cases: Vec<(u32, String)>,
    /// The caller supplies the observation of an empty or unknown row.
    pub otherwise: String,
}
