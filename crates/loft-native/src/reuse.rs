use super::Program;
use naga::valid::{Capabilities, FunctionInfo, ValidationFlags, Validator};
use naga::{Expression, Function, Handle, Module, ShaderStage, SwizzleComponent};
use std::collections::BTreeMap;

// These are the only frame words that vary with the receiver frame alone.
const TIME: std::ops::Range<usize> = 140..144;
const SAMPLES: std::ops::Range<usize> = 188..192;

pub(super) enum Reuse {
    EveryFrame,
    /// `find_shadow_map(...) on load`: the first frame draws the map and every later frame keeps it.
    Once { drawn: bool },
    Static {
        reads_params: bool,
        last: Option<Vec<u8>>,
    },
}

impl Reuse {
    pub(super) fn shadow(
        p: &Program,
        casters: &[u32],
        indexed: impl Fn(&loft_passes::program::KindTable) -> bool,
    ) -> Self {
        if p.shadow_clock == loft_passes::program::StepClock::Load {
            return Self::Once { drawn: false };
        }
        Self::geometry(p, casters, indexed)
    }

    // A compacted cull list can submit equal-depth rows in different orders. Retention
    // keeps the first draw's legal winner at those pixels until the view invalidates.
    pub(super) fn visibility(
        p: &Program, indexed: impl Fn(&loft_passes::program::KindTable) -> bool,
    ) -> Self {
        if p.camera.fixed_eye.is_none() { return Self::EveryFrame; }
        let Some(drawn) = Self::stock_visibility(p) else { return Self::EveryFrame; };
        Self::geometry(p, drawn, indexed)
    }

    /// Native retention and segment tiling both require the stock culler to own these row lists.
    fn stock_visibility(p: &Program) -> Option<&[u32]> {
        use loft_passes::program::{AuthoredRun, Extent, FrameView, Pass};
        let [Pass::Authored(cull), Pass::Vis(drawn), tail @ ..] = p.passes.as_slice() else {
            return None;
        };
        if !p.bakes.is_empty()
            || !p.dispatches.is_empty() || p.path.is_some() || tail.iter().any(|pass| match pass {
                Pass::Vis(_) | Pass::Path(_) => true,
                Pass::Shadow(_) | Pass::Shade(_) => false,
                Pass::Authored(a) => a.bindings.iter().any(|(group, slot, view)| match view {
                    FrameView::DrawArgs | FrameView::Visible | FrameView::Instances | FrameView::KindBases => true,
                    FrameView::Visibility => !p.modules[a.module].naga.global_variables.iter().any(|(_, global)| {
                        global.binding.as_ref().is_some_and(|b| (b.group, b.binding) == (*group, *slot))
                            && matches!(p.modules[a.module].naga.types[global.ty].inner,
                                naga::TypeInner::Image { class: naga::ImageClass::Sampled { .. }, .. })
                    }),
                    FrameView::ShadeArgs | FrameView::Tiles | FrameView::HdrSample | FrameView::HdrStore => false,
                }),
            })
        {
            return None;
        }
        // An authored culler can use the clock, live data or previous outputs. Recognize the
        // stock kernel and its actual bindings, not a pass label or the template's filename.
        if !matches!(&cull.run, AuthoredRun::Compute { entry, extent: Extent::Instances { width: 64 } } if entry == "cull")
            || cull.bindings != [(1, 0, FrameView::Instances), (1, 1, FrameView::DrawArgs),
                (1, 2, FrameView::Visible), (1, 3, FrameView::KindBases)]
        {
            return None;
        }
        let prelude = include_str!("../../../stdlib/core/prelude.wgsl").replace("{{NONCE}}", loft_passes::program::abi::NONCE_PLACEHOLDER);
        let source = &p.modules[cull.module].wgsl;
        let Some(body) = source.strip_prefix(&prelude) else { return None; };
        let template = include_str!("../../../stdlib/core/cull.wgsl");
        let stock = [include_str!("../../../stdlib/core/cull_single.wgsl"), include_str!("../../../stdlib/core/cull_multi.wgsl")];
        if !stock.iter().any(|draw| body == template.replace("{{DRAW}}", draw)) {
            return None;
        }
        Some(drawn)
    }

    fn geometry(p: &Program, casters: &[u32], indexed: impl Fn(&loft_passes::program::KindTable) -> bool,
    ) -> Self {
        let mut reads_params = false;
        for &id in casters {
            let kind = &p.kinds[id as usize];
            let vertex = if indexed(kind) { "vs_grid" } else { "vs" };
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

    /// A frame that was not recorded did not draw what `needs_draw` just retained.
    pub(super) fn forget(&mut self) {
        match self {
            Self::EveryFrame => {}
            Self::Once { drawn } => *drawn = false,
            Self::Static { last, .. } => *last = None,
        }
    }

    pub(super) fn needs_draw(&mut self, uniform: &[u8], params_changed: bool) -> bool {
        match self {
            Self::EveryFrame => true,
            Self::Once { drawn } => !std::mem::replace(drawn, true),
            Self::Static { reads_params, last } => {
                // Projection, eye/ray direction and extent participate in both keys. The row
                // buffers, indices, baked grid, LUTs and attachment allocation belong to this load.
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
        // A skipped draw must not suppress writes to a storage table.
        if matches!(global.space, naga::AddressSpace::Storage { access } if access.intersects(naga::StorageAccess::STORE | naga::StorageAccess::ATOMIC)) {
            return None;
        }
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
    use super::{Reuse, static_caster};

    #[test]
    fn changed_geometry_inputs_invalidate_retained_depth() {
        let mut reuse = Reuse::Static { reads_params: true, last: None };
        let mut frame = [0; 256];
        assert!(reuse.needs_draw(&frame, false));
        frame[140] = 1;
        frame[188] = 4;
        assert!(!reuse.needs_draw(&frame, false));
        assert!(reuse.needs_draw(&frame, true));
        frame[64] = 1;
        assert!(reuse.needs_draw(&frame, false));
    }

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
