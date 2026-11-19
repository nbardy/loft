use super::{CURVE_N, DETAIL_SLOTS, PASS_CAMERA_VEC4S, manifest::*};
use crate::Error;
use crate::artefact::{Artefact, Detail, DrawPass, PassKeep, PassWhat};
use std::collections::{HashMap, HashSet};

/// Every structural check on an artefact; returns the params' float count.
pub(super) fn check_artefact(a: &Artefact, baked_recurrences: &[Vec<u8>]) -> Result<usize, Error> {
    let fail = |msg: String| Error::Internal(format!("artefact: {msg}"));
    if a.version != 1 {
        return Err(fail(format!("unsupported version {}", a.version)));
    }
    for s in &a.inputs {
        let width = s.ty.floats();
        if s.def.len() != width {
            return Err(fail(format!(
                "input \"{}\" is {} but its default has {} floats",
                s.name,
                s.ty.name(),
                s.def.len()
            )));
        }
        if s.slot < 0 || s.slot as usize + width > a.input_floats {
            return Err(fail(format!(
                "input \"{}\" at slot {} overruns inputFloats = {}",
                s.name, s.slot, a.input_floats
            )));
        }
    }
    let param_floats = a.params.iter().fold(0, |n, p| n.max(p.slot + 1)) as usize;
    let mut seen = HashSet::new();
    for p in &a.params {
        if !seen.insert(p.slot) {
            return Err(fail(format!("param \"{}\" reuses slot {}", p.name, p.slot)));
        }
    }
    if !a.curves.len().is_multiple_of(CURVE_N * 4) {
        return Err(fail(format!(
            "curves has {} floats, not a multiple of CURVE_N * 4 = {}",
            a.curves.len(),
            CURVE_N * 4
        )));
    }
    let mut names = HashSet::new();
    let mut bindings = HashSet::new();
    for d in &a.dynamic {
        if !names.insert(d.name.as_str()) {
            return Err(fail(format!("dynamic buffer \"{}\" declared twice", d.name)));
        }
        if !bindings.insert(d.binding) {
            return Err(fail(format!("dynamic buffer \"{}\" reuses binding {}", d.name, d.binding)));
        }
        if d.vec4s == 0 {
            return Err(fail(format!("dynamic buffer \"{}\" has {} vec4s", d.name, d.vec4s)));
        }
    }
    for d in a.statics.as_deref().unwrap_or_default() {
        if !names.insert(d.name.as_str()) {
            return Err(fail(format!("static buffer \"{}\" declared twice", d.name)));
        }
        if !bindings.insert(d.binding) {
            return Err(fail(format!("static buffer \"{}\" reuses binding {}", d.name, d.binding)));
        }
        if d.data.is_empty() || !d.data.len().is_multiple_of(4) {
            return Err(fail(format!(
                "static buffer \"{}\" has {} floats (a positive multiple of 4)",
                d.name,
                d.data.len()
            )));
        }
    }
    for d in &a.draws {
        if a.shaders.get(&d.shader).is_none() {
            return Err(fail(format!("draw \"{}\" names unknown shader \"{}\"", d.label, d.shader)));
        }
        // Opaque draws are what scene depth is made of; one cannot read it while it is being written.
        if d.scene_depth == Some(true) && d.pass == DrawPass::Opaque {
            return Err(fail(format!(
                "draw \"{}\" sets sceneDepth but is opaque; only transparent draws can read the opaque scene's depth",
                d.label
            )));
        }
    }
    let mut by_slot = HashMap::new();
    for d in &a.draws {
        let slot = match &d.detail {
            Detail::Fixed => continue,
            Detail::Grid { slot, bound, levels, .. } => {
                let Some(buf) = a.dynamic.iter().find(|b| b.name == bound.buffer) else {
                    return Err(fail(format!(
                        "draw \"{}\" reads its bounds from \"{}\", which is not a dynamic buffer",
                        d.label, bound.buffer
                    )));
                };
                if bound.offset + d.instances > buf.vec4s {
                    return Err(fail(format!("draw \"{}\": its bounds overrun \"{}\"", d.label, bound.buffer)));
                }
                if levels.first().map(|l| l.vertices) != Some(d.vertices) {
                    return Err(fail(format!(
                        "draw \"{}\": its finest level must have the draw's {} vertices",
                        d.label, d.vertices
                    )));
                }
                if slot + d.instances > DETAIL_SLOTS {
                    return Err(fail(format!(
                        "draw \"{}\": slots {slot}..{} exceed {DETAIL_SLOTS}",
                        d.label,
                        slot + d.instances
                    )));
                }
                *slot
            }
            Detail::Projected { slot, .. } => {
                if slot + 2 > DETAIL_SLOTS {
                    return Err(fail(format!("draw \"{}\": slot {slot} exceeds {DETAIL_SLOTS}", d.label)));
                }
                *slot
            }
        };
        // A slot is one choice: a surface and its ink must describe the same detail.
        if by_slot.insert(slot, &d.detail).is_some_and(|detail| detail != &d.detail) {
            return Err(fail(format!(
                "draw \"{}\" shares level-of-detail slot {slot} with a draw whose detail differs",
                d.label
            )));
        }
    }
    for p in a.post.as_deref().unwrap_or_default() {
        if a.shaders.get(&p.shader).is_none() {
            return Err(fail(format!("post pass \"{}\" names unknown shader \"{}\"", p.label, p.shader)));
        }
    }
    // The baked states are the truth the files carry; an unbaked artefact would write zeros.
    for (i, r) in a.recurrences.as_deref().unwrap_or_default().iter().enumerate() {
        if baked_recurrences.get(i).is_none_or(Vec::is_empty) {
            return Err(fail(format!(
                "the recurrence \"{}\" is not baked; bake it on a GPU first (bakeRecurrences, which createHost's load runs)",
                r.name
            )));
        }
    }
    for st in &a.stages {
        if !(st.invocations.fract() == 0.0 && st.invocations >= 1.0) {
            return Err(fail(format!(
                "pose stage \"{}\" has {} invocations",
                st.label,
                super::to_json(&st.invocations)
            )));
        }
    }
    check_passes(a, fail)?;
    let sizes: HashMap<_, _> = a.dynamic.iter().map(|d| (d.name.as_str(), d.vec4s)).collect();
    for t in &a.tables {
        let Some(&size) = sizes.get(t.buffer.as_str()) else {
            return Err(fail(format!("table \"{}\" lives in \"{}\", which is not a dynamic buffer", t.name, t.buffer)));
        };
        if t.offset < 0 || t.offset as usize + t.vec4s > size {
            return Err(fail(format!("table \"{}\" overruns \"{}\"", t.name, t.buffer)));
        }
    }
    Ok(param_floats)
}

/// The passes' structure: names, what each draws, where its camera lives, and that it reads only passes before it.
fn check_passes(a: &Artefact, fail: impl Fn(String) -> Error) -> Result<(), Error> {
    let mut before = HashSet::new();
    for p in a.passes.as_deref().unwrap_or_default() {
        let mut name = p.name.bytes();
        if !name.next().is_some_and(|c| c.is_ascii_alphabetic())
            || !name.all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err(fail(format!("pass \"{}\": a name must be letters, digits and _", p.name)));
        }
        if before.contains(p.name.as_str()) {
            return Err(fail(format!("pass \"{}\" listed twice", p.name)));
        }
        for r in &p.reads {
            if !before.contains(r.as_str()) {
                return Err(fail(format!("pass \"{}\" reads \"{r}\", which is not drawn before it", p.name)));
            }
        }
        match &p.what {
            PassWhat::Screen { shader } => {
                if a.shaders.get(shader).is_none() {
                    return Err(fail(format!("pass \"{}\" names unknown shader \"{shader}\"", p.name)));
                }
            }
            PassWhat::Scene { camera, draws, keep, .. } => {
                let buf = a.dynamic.iter().find(|b| b.name == camera.buffer);
                if buf.is_none_or(|b| camera.offset + PASS_CAMERA_VEC4S > b.vec4s) {
                    return Err(fail(format!(
                        "pass \"{}\": its camera is not a table of {PASS_CAMERA_VEC4S} vec4s in a dynamic buffer",
                        p.name
                    )));
                }
                for &k in draws {
                    let Some(d) = a.draws.get(k) else {
                        return Err(fail(format!("pass \"{}\" draws draw {k}, which does not exist", p.name)));
                    };
                    if matches!(keep, PassKeep::Depth) && (d.depth.is_none() || d.pass != DrawPass::Opaque) {
                        return Err(fail(format!(
                            "pass \"{}\" keeps depth but draw \"{}\" is not opaque with a depth entry",
                            p.name, d.label
                        )));
                    }
                }
            }
        }
        before.insert(p.name.as_str());
    }
    Ok(())
}

/// A loader must understand every field that affects what it draws. The earliest usable version preserves old bytes.
pub(super) fn format_version(replay: bool, passes: bool, shared: bool, mixed: bool, compute: bool) -> usize {
    if compute {
        5
    } else if mixed {
        4
    } else if shared {
        3
    } else if replay || passes {
        2
    } else {
        1
    }
}

pub(super) fn check_binding_limits(pls: &[PipelineLayoutDesc], bgls: &[BindGroupLayoutDesc]) -> Result<(), Error> {
    for pipeline in pls {
        for &stage in super::ALL {
            let mut storage = 0;
            let mut sampled = 0;
            let mut textures = 0;
            for e in pipeline.bind_group_layouts.iter().flat_map(|&id| &bgls[id].entries) {
                if !e.visibility.contains(&stage) {
                    continue;
                }
                match e.resource {
                    LayoutResource::Buffer { ty: BufferType::Uniform } | LayoutResource::Sampler { .. } => {}
                    LayoutResource::Buffer { ty: BufferType::Storage | BufferType::ReadOnlyStorage } => storage += 1,
                    LayoutResource::Texture { .. } => sampled += 1,
                    LayoutResource::StorageTexture { .. } => textures += 1,
                }
            }
            for (count, max, what) in
                [(storage, 8, "storage buffers"), (sampled, 16, "sampled textures"), (textures, 4, "storage textures")]
            {
                if count > max {
                    return Err(Error::Scene(format!(
                        "{}: given {count} {what} in the {} layout; expected at most {max}. Fix: pack related outputs into one resource or remove a binding",
                        pipeline.label,
                        stage.name()
                    )));
                }
            }
        }
    }
    Ok(())
}
