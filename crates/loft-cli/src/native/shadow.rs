use super::Program;
use naga::valid::{Capabilities, FunctionInfo, ValidationFlags, Validator};
use naga::{Expression, Function, Handle, Module, ShaderStage, SwizzleComponent};
use std::collections::BTreeMap;

// These are the only frame words that vary with the receiver frame alone.
const TIME: std::ops::Range<usize> = 140..144;
const SAMPLES: std::ops::Range<usize> = 188..192;

pub(super) enum Reuse {
    EveryFrame,
    Static {
        reads_params: bool,
        last: Option<Vec<u8>>,
    },
}

impl Reuse {
    pub(super) fn prepare(
        p: &Program,
        casters: &[u32],
        indexed: impl Fn(&loft_passes::program::KindTable) -> bool,
        editable: Option<usize>,
    ) -> Self {
        let mut reads_params = false;
        let editable_module = editable.map(|id| p.kinds[id].module);
        for &id in casters {
            let kind = &p.kinds[id as usize];
            // A tape can start reading time after publication, and promotion replaces its shader.
            if editable_module == Some(kind.module) {
                return Self::EveryFrame;
            }
            let vertex = if indexed(kind) { "vs_indexed" } else { "vs" };
            let Some(params) = static_caster(&p.modules[kind.module].naga, vertex) else {
                return Self::EveryFrame;
            };
            reads_params |= params;
        }
        Self::Static {
            reads_params,
            last: None,
        }
    }

    pub(super) fn needs_draw(&mut self, uniform: &[u8], params_changed: bool) -> bool {
        match self {
            Self::EveryFrame => true,
            Self::Static { reads_params, last } => {
                // The other bytes include the actual light projection and ray direction. The map,
                // caster list, row/bounds buffers, indices, baked grid and LUTs belong to this load.
                let same_projection = last.as_ref().is_some_and(|previous| {
                    uniform[..TIME.start] == previous[..TIME.start]
                        && uniform[TIME.end..SAMPLES.start] == previous[TIME.end..SAMPLES.start]
                        && uniform[SAMPLES.end..] == previous[SAMPLES.end..]
                });
                if same_projection && !(*reads_params && params_changed) {
                    return false;
                }
                *last = Some(uniform.to_vec());
                true
            }
        }
    }
}

fn static_caster(module: &Module, vertex: &str) -> Option<bool> {
    let mut module = module.clone();
    let stages = [(ShaderStage::Vertex, vertex), (ShaderStage::Fragment, "fs")];
    module.entry_points.retain(|e| stages.contains(&(e.stage, e.name.as_str())));
    if module.entry_points.len() != 2 {
        return None;
    }
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    validator.validate(&module).ok()?;
    naga::compact::compact(&mut module, naga::compact::KeepUnused::No);
    let info = validator.validate(&module).ok()?;

    let mut frame = None;
    let mut reads_params = false;
    for (id, global) in module.global_variables.iter() {
        let slot = global.binding.as_ref().map(|b| (b.group, b.binding));
        match slot {
            Some((0, 0)) => {
                let naga::TypeInner::Struct { members, span: 256 } = &module.types[global.ty].inner
                else {
                    return None;
                };
                if !members
                    .iter()
                    .map(|m| m.offset)
                    .eq([0, 64, 128, 144, 160, 176, 192])
                {
                    return None;
                }
                for (index, scalar) in [(2, naga::Scalar::F32), (5, naga::Scalar::U32)] {
                    if module.types[members[index].ty].inner
                        != (naga::TypeInner::Vector {
                            size: naga::VectorSize::Quad,
                            scalar,
                        })
                    {
                        return None;
                    }
                }
                frame = Some(id);
            }
            Some((0, 2)) => reads_params = true,
            Some((0, 1) | (1, 0..=3) | (2, 5)) => {}
            _ => return None,
        }
    }
    if let Some(frame) = frame {
        let functions = module.functions.iter().map(|(id, f)| (f, &info[id]));
        let entries = module
            .entry_points
            .iter()
            .enumerate()
            .map(|(i, e)| (&e.function, info.get_entry_point(i)));
        if !functions
            .chain(entries)
            .all(|(f, info)| static_frame(f, info, frame))
        {
            return None;
        }
    }
    Some(reads_params)
}

#[derive(Clone, Copy)]
enum FrameAccess {
    Struct,
    Tail,
}

fn static_frame(f: &Function, info: &FunctionInfo, frame: Handle<naga::GlobalVariable>) -> bool {
    let mut uses = BTreeMap::new();
    for (id, expr) in f.expressions.iter() {
        #[allow(clippy::wildcard_enum_match_arm)]
        // Other users get no credit and fail the reference count.
        let next = match *expr {
            Expression::GlobalVariable(global) if global == frame => Some(FrameAccess::Struct),
            Expression::AccessIndex { base, index } => {
                let Some((kind, credited)) = uses.get_mut(&base) else {
                    continue;
                };
                *credited += 1;
                match (*kind, index) {
                    (FrameAccess::Struct, 0 | 1 | 3 | 4 | 6) | (FrameAccess::Tail, 0..=2) => None,
                    (FrameAccess::Struct, 2 | 5) => Some(FrameAccess::Tail),
                    _ => return false,
                }
            }
            Expression::Load { pointer } => {
                let Some((FrameAccess::Tail, credited)) = uses.get_mut(&pointer) else {
                    continue;
                };
                *credited += 1;
                Some(FrameAccess::Tail)
            }
            Expression::Swizzle {
                vector,
                size,
                pattern,
            } if pattern[..size as usize]
                .iter()
                .all(|c| *c != SwizzleComponent::W) =>
            {
                let Some((FrameAccess::Tail, credited)) = uses.get_mut(&vector) else {
                    continue;
                };
                *credited += 1;
                None
            }
            _ => continue,
        };
        if let Some(kind) = next {
            uses.insert(id, (kind, 0));
        }
    }
    // An unrecognized consumer could carry time or sample count through a whole-vector/struct read.
    uses.into_iter()
        .all(|(id, (_, credited))| info[id].ref_count == credited)
}

#[cfg(test)]
mod tests {
    use super::static_caster;

    #[test]
    fn only_caster_stages_must_ignore_frame_time_and_samples() {
        for (depth, expected) in [
            ("frame.eye.xyz.z", Some(false)),
            ("params[0]", Some(true)),
            ("frame.eye.w", None),
            ("last(frame.eye)", None),
            ("f32(frame.cold.w)", None),
        ] {
            let wgsl = format!(
                r"
                struct Frame {{
                    vp: mat4x4f, inv: mat4x4f, eye: vec4f, sun: vec4f,
                    size: vec4u, cold: vec4u, light_vp: mat4x4f,
                }}
                @group(0) @binding(0) var<uniform> frame: Frame;
                @group(0) @binding(2) var<storage, read> params: array<f32>;
                fn last(eye: vec4f) -> f32 {{ return eye.w; }}
                @vertex fn vs() -> @builtin(position) vec4f {{ return frame.vp * vec4f(0.0); }}
                @fragment fn fs() -> @builtin(frag_depth) f32 {{ return {depth}; }}
                @fragment fn unrelated() -> @location(0) vec4f {{ return vec4f(frame.eye.w); }}
                "
            );
            let module = naga::front::wgsl::parse_str(&wgsl).unwrap();
            assert_eq!(static_caster(&module, "vs"), expected, "{depth}");
        }
    }
}
