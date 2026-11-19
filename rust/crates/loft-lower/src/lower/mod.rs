//! src/host/lower.ts: the host's load-time decisions as WebGPU descriptors (docs/artefact_format.md).
mod check;
mod compute;
mod detail;
mod manifest;
mod passes;
mod render;

use crate::Error;
use crate::artefact::{Artefact, AuthorLines, AuthorText, Kept, Mixed, OrderedMap, PoseStage, StaticBuffer};
use crate::files::{chunk_name, to_json, try_to_json, write_bundle};
use crate::share::{ROWS_BINDING, ShareDraw, ShareInput, try_share};
use crate::wgsl::PRELUDE_WGSL;
use manifest::*;
use std::collections::HashMap;
use std::path::Path;

use self::compute::ComputeRuns;
use self::passes::PassObjects;
use self::render::RenderState;

const CURVE_N: usize = 64;
const DETAIL_SLOTS: usize = 4096;
const PASS_CAMERA_VEC4S: usize = 12;
const POSE_WORKGROUP: usize = 64;
/// Two mat4 then four vec4 (the prelude's Frame uniform).
const FRAME_BYTES: usize = (16 + 16 + 4 * 4) * 4;
/// WebGPU rejects zero-sized bindings, even for an artefact with no inputs or params.
const MIN_BUFFER_BYTES: usize = 16;
const DRAWN: &[Stage] = &[Stage::Vertex, Stage::Fragment];
const ALL: &[Stage] = &[Stage::Vertex, Stage::Fragment, Stage::Compute];
const COMPUTE: &[Stage] = &[Stage::Compute];
const STORAGE: &[&str] = &["STORAGE", "COPY_DST"];
const UNIFORM: &[&str] = &["UNIFORM", "COPY_DST"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    F32,
    Mixed,
}

/// Lower the TS compiler's compile.json and GPU bake outputs to the default f32 bundle.
pub fn lower(compile_json: &str, baked: &Path, out: &Path) -> Result<(), Error> {
    lower_with_precision(compile_json, baked, out, Precision::F32)
}

/// As `lower`, with the version-4 mixed overlay when requested.
pub fn lower_with_precision(compile_json: &str, baked: &Path, out: &Path, precision: Precision) -> Result<(), Error> {
    let a: Artefact = serde_json::from_str(compile_json).map_err(|e| Error::Input(format!("compile.json: {e}")))?;
    let recurrences = a.recurrences.as_deref().unwrap_or_default();
    let states: Vec<_> =
        recurrences.iter().map(|r| read_baked(baked, "recurrence", &r.name)).collect::<Result<_, _>>()?;
    let param_floats = check::check_artefact(&a, &states)?;
    let share_input = ShareInput {
        scene_wgsl: a.scene_wgsl.clone(),
        shaders: a.shaders.0.clone(),
        draws: a
            .draws
            .iter()
            .map(|d| ShareDraw {
                shader: d.shader.clone(),
                constants: d.constants.0.clone(),
                // Sharing only needs to distinguish counts up to 2^16. Larger counts always exclude the shader.
                instances: d.instances.min(u32::MAX as usize) as u32,
            })
            .collect(),
        mixed_shaders: a.mixed.iter().flat_map(|m| m.shaders.0.iter().map(|(name, _)| name.clone())).collect(),
    };
    let sharing = try_share(&share_input)?;
    let statics = baked_statics(&a, &states)?;
    let mixed = match precision {
        Precision::F32 => None,
        Precision::Mixed => a.mixed.as_ref(),
    };

    let mut lower = Lower { a: &a, mixed, ..Lower::default_for(&a) };
    let mut compute = ComputeRuns::new(baked, a.compute.as_ref())?;
    let (prelude, dynamic, static_bufs) = lower.buffers(param_floats, &statics, &sharing.rows, &mut compute)?;
    let layouts = lower.layouts(&prelude, &dynamic, &static_bufs, &compute)?;
    let scene = SceneObjects {
        prelude,
        layouts,
        buffer_of: a.dynamic.iter().zip(&dynamic).map(|(d, b)| (d.name.as_str(), b.id)).collect(),
    };
    let prelude_chunk = lower.chunk(PRELUDE_WGSL)?;
    let (pose, replay) = lower.pose_stages(&scene, prelude_chunk)?;
    lower.author_compute(&scene, &mut compute)?;

    let scene_chunk = lower.chunk(&a.scene_wgsl)?;
    let mut render = RenderState::new(&scene, sharing, prelude_chunk, scene_chunk);
    let opaque = lower.planned(&mut render, crate::artefact::DrawPass::Opaque)?;
    let transparent = lower.planned(&mut render, crate::artefact::DrawPass::Transparent)?;
    let offscreen = scene.layouts.pass.map(|_| lower.lower_pass_objects(&scene));
    let (view_passes, filler) = lower.view_passes(&mut render, offscreen.as_ref(), opaque, transparent)?;
    let (mut render_passes, copies) = match offscreen {
        Some(PassObjects { copies, lowered, .. }) => (lower.record(&mut render, lowered, filler)?, Some(copies)),
        None => (Vec::new(), None),
    };
    render_passes.extend(view_passes);
    let bounds = lower.bounds(&scene);
    if a.compute.is_some() {
        check::check_binding_limits(&lower.pls, &lower.bgls)?;
    }

    let manifest = Manifest {
        format: FORMAT,
        version: check::format_version(
            !replay.is_empty(),
            scene.layouts.pass.is_some(),
            scene.prelude.rows.is_some(),
            precision == Precision::Mixed,
            a.compute.is_some(),
        ),
        precision: (precision == Precision::Mixed).then_some(MixedPrecision {
            kind: "mixed",
            fallback: "f32-fallback",
            modules: lower.mixed_modules,
            buffers: lower.mixed_buffers,
        }),
        chunks: lower.chunks.iter().map(|(name, _)| name.as_str()).collect(),
        authored: (!lower.authored.0.is_empty()).then_some(lower.authored),
        blobs: lower.blobs.iter().map(|(name, data)| Blob { name, bytes: data.len() }).collect(),
        inputs: &a.inputs,
        input_floats: a.input_floats,
        params: &a.params,
        param_floats,
        cameras: &a.cameras,
        curves: FloatWrite { buffer: scene.prelude.curve, floats: a.curves.len() },
        buffers: lower.buffers,
        textures: lower.textures,
        samplers: lower.samplers,
        bind_group_layouts: lower.bgls,
        pipeline_layouts: lower.pls,
        modules: lower.modules,
        compute_pipelines: lower.computes,
        render_pipelines: lower.renders,
        bind_groups: lower.groups,
        build: (!compute.build.runs.is_empty()).then_some(compute.build),
        load: compute.load,
        frame: Frame {
            compute: compute.frame,
            writes: [
                FrameWrite::Frame { buffer: scene.prelude.frame },
                FrameWrite::Inputs { buffer: scene.prelude.input },
                FrameWrite::Params { buffer: scene.prelude.param },
            ],
            pose,
            bounds,
            // Only used slots are written each frame; the rest of the 64 KiB array is never read.
            detail: FloatWrite { buffer: scene.prelude.detail, floats: (detail::slots_used(&a.draws) * 4).max(4) },
            passes: render_passes,
            replay,
            copies,
        },
        dynamic: dynamic.iter().map(|d| d.id).collect(),
        plan: a
            .draws
            .iter()
            .map(|d| PlanDraw {
                label: &d.label,
                layer: d.layer,
                vertices: d.vertices,
                instances: d.instances,
                detail: &d.detail,
            })
            .collect(),
        tables: a
            .tables
            .iter()
            .map(|t| TableEntry {
                name: &t.name,
                buffer: scene.buffer_of[t.buffer.as_str()],
                offset: t.offset,
                vec4s: t.vec4s,
                lanes: t.lanes,
                fill: &t.fill,
            })
            .collect(),
        stats: &a.stats,
    };
    write_bundle(out, &try_to_json(&manifest)?, &lower.chunks, &lower.blobs)
}

fn read_baked(dir: &Path, kind: &str, key: &str) -> Result<Vec<u8>, Error> {
    let path = dir.join(format!("{kind}.{key}.bin"));
    match std::fs::read(&path) {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(Error::Input(format!("{}: {e}", path.display()))),
    }
}

fn baked_statics(a: &Artefact, states: &[Vec<u8>]) -> Result<Vec<StaticBuffer>, Error> {
    let mut statics = a.statics.clone().unwrap_or_default();
    for (r, bytes) in a.recurrences.as_deref().unwrap_or_default().iter().zip(states) {
        let fail =
            || Error::Input(format!("recurrence \"{}\": missing or wrong-sized static placement or bake data", r.name));
        // compile.json flattens the recurrence's typed-array view. Its scene accessor still names the exact
        // packed buffer and vec4 offset, even when several histories share that buffer.
        let prefix = format!("fn t_{}(i: u32) -> vec4<f32> {{ return ", r.name);
        let (buffer, offset) = a
            .scene_wgsl
            .lines()
            .find_map(|line| {
                let (buffer, rest) = line.strip_prefix(&prefix)?.split_once('[')?;
                let offset = rest.strip_suffix("u + i]; }")?.parse::<usize>().ok()?;
                Some((buffer, offset.checked_mul(16)?))
            })
            .ok_or_else(fail)?;
        let data = &mut statics.iter_mut().find(|s| s.name == buffer).ok_or_else(fail)?.data.0;
        if bytes.len() != r.data.0.len() {
            return Err(fail());
        }
        let end = offset.checked_add(bytes.len()).ok_or_else(fail)?;
        data.get_mut(offset..end).ok_or_else(fail)?.copy_from_slice(bytes);
    }
    Ok(statics)
}

struct Lower<'a> {
    a: &'a Artefact,
    mixed: Option<&'a Mixed>,
    mixed_modules: OrderedMap<Vec<Id>>,
    mixed_buffers: OrderedMap<MixedBuffer>,
    chunk_ids: HashMap<String, Id>,
    chunks: Vec<(String, String)>,
    authored: OrderedMap<Vec<AuthorLines>>,
    blobs: Vec<(String, Vec<u8>)>,
    buffers: Vec<BufferDesc>,
    textures: Vec<TextureDesc>,
    samplers: Vec<SamplerDesc>,
    bgls: Vec<BindGroupLayoutDesc>,
    pls: Vec<PipelineLayoutDesc>,
    modules: Vec<ModuleDesc>,
    computes: Vec<ComputePipelineDesc>,
    renders: Vec<RenderPipelineDesc>,
    groups: Vec<BindGroupDesc>,
}

#[derive(Clone, Copy)]
struct Prelude {
    frame: Id,
    input: Id,
    param: Id,
    curve: Id,
    detail: Id,
    rows: Option<Id>,
}

impl Prelude {
    fn bindings(self, frame: Id) -> Vec<BindGroupEntry> {
        [frame, self.input, self.param, self.curve, self.detail]
            .into_iter()
            .chain(self.rows)
            .enumerate()
            .map(|(binding, id)| BindGroupEntry { binding, resource: Resource::Buffer(id) })
            .collect()
    }
}

struct TableBuffer {
    binding: usize,
    id: Id,
}

struct Layouts {
    prelude: Id,
    tables: Id,
    pose_tables: Id,
    group0: Id,
    group1: Id,
    pose_group1: Id,
    depth: Option<Id>,
    post: Option<Id>,
    pass: Option<Id>,
    empty: Option<Id>,
}

struct SceneObjects<'a> {
    prelude: Prelude,
    layouts: Layouts,
    buffer_of: HashMap<&'a str, Id>,
}

impl<'a> Lower<'a> {
    fn default_for(a: &'a Artefact) -> Self {
        Self {
            a,
            mixed: None,
            mixed_modules: OrderedMap::default(),
            mixed_buffers: OrderedMap::default(),
            chunk_ids: HashMap::new(),
            chunks: Vec::new(),
            authored: OrderedMap::default(),
            blobs: Vec::new(),
            buffers: Vec::new(),
            textures: Vec::new(),
            samplers: Vec::new(),
            bgls: Vec::new(),
            pls: Vec::new(),
            modules: Vec::new(),
            computes: Vec::new(),
            renders: Vec::new(),
            groups: Vec::new(),
        }
    }

    fn chunk(&mut self, text: &str) -> Result<Id, Error> {
        let name = chunk_name(text);
        if let Some(&id) = self.chunk_ids.get(&name) {
            if self.chunks[id].1 != text {
                return Err(Error::Internal(format!("lower: two different WGSL texts hash to chunk {name}")));
            }
            return Ok(id);
        }
        let id = self.chunks.len();
        self.chunk_ids.insert(name.clone(), id);
        let spans = author_lines(text, self.a.authored.as_deref().unwrap_or_default());
        if !spans.is_empty() {
            self.authored.0.push((name.clone(), spans));
        }
        self.chunks.push((name, text.to_owned()));
        Ok(id)
    }

    fn blob(&mut self, name: impl Into<String>, data: Vec<u8>) -> String {
        let name = name.into();
        if let Some((_, owned)) = self.blobs.iter_mut().find(|(key, _)| key == &name) {
            *owned = data;
        } else {
            self.blobs.push((name.clone(), data));
        }
        name
    }

    fn buffer(
        &mut self,
        label: impl Into<String>,
        bytes: usize,
        usage: &'static [&'static str],
        init: Option<String>,
    ) -> Id {
        add(&mut self.buffers, BufferDesc { label: label.into(), size: bytes.max(MIN_BUFFER_BYTES), usage, init })
    }

    fn buffers(
        &mut self,
        param_floats: usize,
        statics: &[StaticBuffer],
        rows: &[f32],
        compute: &mut ComputeRuns<'a>,
    ) -> Result<(Prelude, Vec<TableBuffer>, Vec<TableBuffer>), Error> {
        let a = self.a;
        let frame = self.buffer("frame", FRAME_BYTES, UNIFORM, None);
        let input = self.buffer("inputs", a.input_floats * 4, STORAGE, None);
        let param = self.buffer("params", param_floats * 4, STORAGE, None);
        let curves = (!a.curves.is_empty()).then(|| self.blob("curves", a.curves.0.clone()));
        let curve = self.buffer("curves", a.curves.0.len(), STORAGE, curves);
        let detail = self.buffer("loft_detail", DETAIL_SLOTS * 16, UNIFORM, None);
        // The level-of-detail bounds and readTables copy dynamic buffers back.
        let dynamic = a
            .dynamic
            .iter()
            .map(|d| TableBuffer {
                binding: d.binding,
                id: self.buffer(&d.name, d.vec4s * 16, &["STORAGE", "COPY_DST", "COPY_SRC"], None),
            })
            .collect();
        let static_bufs: Vec<_> = statics
            .iter()
            .map(|d| {
                let init = self.blob(&d.name, d.data.0.clone());
                let id = self.buffer(&d.name, d.data.0.len(), STORAGE, Some(init));
                TableBuffer { binding: d.binding, id }
            })
            .collect();
        if let Some(mixed) = self.mixed {
            for (d, b) in statics.iter().zip(&static_bufs) {
                if let Some(data) = mixed.statics.get(&d.name) {
                    let init = self.blob(format!("{}.f16", d.name), data.0.clone());
                    self.mixed_buffers
                        .0
                        .push((b.id.to_string(), MixedBuffer { size: data.0.len().max(MIN_BUFFER_BYTES), init }));
                }
            }
        }
        self.raw_buffers(compute)?;
        // Shared shaders' rows are bound at group 0 only when some shader is shared.
        let rows = (!rows.is_empty()).then(|| {
            let init = self.blob("loft rows", rows.iter().flat_map(|f| f.to_le_bytes()).collect());
            self.buffer("loft rows", size_of_val(rows), STORAGE, Some(init))
        });
        Ok((Prelude { frame, input, param, curve, detail, rows }, dynamic, static_bufs))
    }

    fn layouts(
        &mut self,
        prelude: &Prelude,
        dynamic: &[TableBuffer],
        statics: &[TableBuffer],
        compute: &ComputeRuns,
    ) -> Result<Layouts, Error> {
        let a = self.a;
        // Explicit layouts let shaders read any subset. Auto layouts would drop bindings not used by a shader.
        let mut entries = vec![
            buffer_layout(0, ALL, BufferType::Uniform),
            storage(1, ALL),
            storage(2, ALL),
            storage(3, ALL),
            buffer_layout(4, ALL, BufferType::Uniform),
        ];
        if prelude.rows.is_some() {
            entries.push(storage(ROWS_BINDING as usize, ALL));
        }
        let l0 = self.bgl("loft prelude", entries);
        let resources = a.compute.as_ref().map_or(&[][..], |c| &c.resources);
        let raw_entries =
            |visibility| resources.iter().map(move |r| compute::raw_layout(&r.resource, r.binding, visibility, false));
        let raw_groups = resources
            .iter()
            .map(|r| Ok(BindGroupEntry { binding: r.binding, resource: compute.resource(&r.resource.key)? }))
            .collect::<Result<Vec<_>, Error>>()?;
        let group1_buffers = || dynamic.iter().chain(statics);
        let l1 = self.bgl(
            "loft scene tables",
            group1_buffers().map(|d| storage(d.binding, DRAWN)).chain(raw_entries(DRAWN)).collect(),
        );
        // The pose stages' group 1 makes dynamic buffers writable while keeping statics read-only.
        let pose_tables = self.bgl(
            "loft pose tables",
            dynamic
                .iter()
                .map(|d| buffer_layout(d.binding, COMPUTE, BufferType::Storage))
                .chain(statics.iter().map(|d| storage(d.binding, COMPUTE)))
                .chain(raw_entries(COMPUTE))
                .collect(),
        );
        let group1_entries = || {
            group1_buffers()
                .map(|d| BindGroupEntry { binding: d.binding, resource: Resource::Buffer(d.id) })
                .chain(raw_groups.iter().copied())
                .collect()
        };
        let group0 = self.group("loft prelude", l0, prelude.bindings(prelude.frame));
        let group1 = self.group("loft scene tables", l1, group1_entries());
        let pose_group1 = self.group("loft pose tables", pose_tables, group1_entries());

        let split =
            a.post.as_ref().is_some_and(|p| !p.is_empty()) || a.draws.iter().any(|d| d.scene_depth == Some(true));
        // Group 2 is the layout documented on SCENE_DEPTH_WGSL and POST_WGSL, and exists only in a split frame.
        let scene_depth = || texture_layout(0, DRAWN, SampleType::UnfilterableFloat, false);
        let depth = split.then(|| {
            self.bgl(
                "loft scene depth",
                vec![scene_depth(), texture_layout(1, DRAWN, SampleType::UnfilterableFloat, false)],
            )
        });
        let post = split.then(|| {
            self.bgl(
                "loft post",
                vec![
                    scene_depth(),
                    texture_layout(1, DRAWN, SampleType::Float, false),
                    sampler_layout(2, SamplerType::Filtering),
                ],
            )
        });
        let passes = a.passes.as_deref().unwrap_or_default();
        // Group 3 binds passes. Pipeline layouts number groups without gaps, even when nothing reads group 2.
        let pass = (!passes.is_empty()).then(|| {
            self.bgl(
                "loft passes",
                std::iter::once(sampler_layout(0, SamplerType::Filtering))
                    .chain(
                        passes
                            .iter()
                            .enumerate()
                            .map(|(i, p)| texture_layout(i + 1, DRAWN, passes::keep_of(p).sample_type(), false)),
                    )
                    .collect(),
            )
        });
        let empty = (pass.is_some() && !split).then(|| self.bgl("loft none", vec![]));
        Ok(Layouts { prelude: l0, tables: l1, pose_tables, group0, group1, pose_group1, depth, post, pass, empty })
    }

    fn pose_stages(&mut self, scene: &SceneObjects<'_>, prelude: Id) -> Result<(Vec<Dispatch>, Vec<Replay>), Error> {
        let a = self.a;
        let mixed = self.mixed;
        let mut replayed = a
            .recurrences
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|r| r.replay.as_ref().map(|p| (r, p)))
            .peekable();
        if a.stages.is_empty() && replayed.peek().is_none() {
            return Ok((Vec::new(), Vec::new()));
        }
        let layout = self.pipeline_layout("loft pose", vec![scene.layouts.prelude, scene.layouts.pose_tables]);
        let groups = [scene.layouts.group0, scene.layouts.pose_group1];
        let pose = a
            .stages
            .iter()
            .enumerate()
            .map(|(i, st)| {
                self.dispatch(
                    st,
                    "pose stage",
                    mixed.and_then(|m| m.stages.get(&i.to_string())).map(String::as_str),
                    prelude,
                    layout,
                    groups,
                )
            })
            .collect::<Result<_, _>>()?;
        // A checkpointed recurrence has three pose-stage modules, executed before the regular stages.
        let replay = replayed
            .map(|(r, p)| {
                let mut dispatch = |st| self.dispatch(st, "replay stage", None, prelude, layout, groups);
                let every = match r.kept {
                    Kept::Every => 1,
                    Kept::Checkpoints { every } => every,
                };
                Ok::<_, Error>(Replay {
                    label: format!("replay {}", r.name),
                    slot: p.slot,
                    rate: r.rate,
                    start: r.start,
                    steps: r.steps,
                    every,
                    first: dispatch(&p.first)?,
                    ping: dispatch(&p.ping)?,
                    pong: dispatch(&p.pong)?,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok((pose, replay))
    }

    fn dispatch(
        &mut self,
        stage: &PoseStage,
        what: &str,
        alternative: Option<&str>,
        prelude: Id,
        layout: Id,
        groups: [Id; 2],
    ) -> Result<Dispatch, Error> {
        let wgsl = self.chunk(&stage.wgsl)?;
        let module = self.module(format!("{what} \"{}\"", stage.label), vec![prelude, wgsl]);
        if let Some(alternative) = alternative.filter(|text| *text != stage.wgsl) {
            let enable = self.enable_f16()?;
            let alternative = self.chunk(alternative)?;
            self.mixed_modules.0.push((module.to_string(), vec![enable, prelude, alternative]));
        }
        let pipeline = self.compute_pipeline(stage.label.clone(), layout, module, "pose_main");
        Ok(Dispatch {
            label: stage.label.clone(),
            pipeline,
            bind_groups: groups.to_vec(),
            workgroups: Workgroups::X((stage.invocations as usize).div_ceil(POSE_WORKGROUP)),
        })
    }

    fn enable_f16(&mut self) -> Result<Id, Error> {
        self.chunk("enable f16;\n")
    }

    fn bounds(&mut self, scene: &SceneObjects<'_>) -> BoundsRead {
        let grids = detail::grid_bounds(&self.a.draws);
        if grids.is_empty() {
            return BoundsRead::None;
        }
        let mut at = 0;
        let grids = grids
            .into_iter()
            .map(|g| {
                let c = GridCopy {
                    slot: g.slot,
                    from: scene.buffer_of[g.bound.buffer.as_str()],
                    from_offset: g.bound.offset * 16,
                    at,
                    instances: g.instances,
                };
                at += g.instances;
                c
            })
            .collect();
        BoundsRead::Read { buffer: self.buffer("loft bounds", at * 16, &["MAP_READ", "COPY_DST"], None), grids }
    }

    fn bgl(&mut self, label: impl Into<String>, entries: Vec<LayoutEntry>) -> Id {
        add(&mut self.bgls, BindGroupLayoutDesc { label: label.into(), entries })
    }

    fn group(&mut self, label: impl Into<String>, layout: Id, entries: Vec<BindGroupEntry>) -> Id {
        add(&mut self.groups, BindGroupDesc { label: label.into(), layout, entries })
    }

    fn pipeline_layout(&mut self, label: impl Into<String>, bind_group_layouts: Vec<Id>) -> Id {
        add(&mut self.pls, PipelineLayoutDesc { label: label.into(), bind_group_layouts })
    }

    fn module(&mut self, label: impl Into<String>, chunks: Vec<Id>) -> Id {
        add(&mut self.modules, ModuleDesc { label: label.into(), chunks })
    }

    fn compute_pipeline(&mut self, label: String, layout: Id, module: Id, entry: &str) -> Id {
        add(&mut self.computes, ComputePipelineDesc { label, layout, module, entry_point: entry.to_owned() })
    }
}

fn add<T>(list: &mut Vec<T>, item: T) -> Id {
    let id = list.len();
    list.push(item);
    id
}

fn buffer_layout(binding: usize, visibility: &'static [Stage], ty: BufferType) -> LayoutEntry {
    LayoutEntry { binding, visibility, resource: LayoutResource::Buffer { ty } }
}

fn storage(binding: usize, visibility: &'static [Stage]) -> LayoutEntry {
    buffer_layout(binding, visibility, BufferType::ReadOnlyStorage)
}

fn texture_layout(
    binding: usize,
    visibility: &'static [Stage],
    sample_type: SampleType,
    multisampled: bool,
) -> LayoutEntry {
    LayoutEntry {
        binding,
        visibility,
        resource: LayoutResource::Texture { sample_type, view_dimension: "2d", multisampled },
    }
}

fn sampler_layout(binding: usize, ty: SamplerType) -> LayoutEntry {
    LayoutEntry { binding, visibility: DRAWN, resource: LayoutResource::Sampler { ty } }
}

/// Registered author text is found in the finished chunk because shader assembly does not retain offsets.
fn author_lines(code: &str, texts: &[AuthorText]) -> Vec<AuthorLines> {
    let mut spans = Vec::new();
    // The authored-text registry keeps the first source location for each verbatim text.
    let mut seen = std::collections::HashSet::new();
    for t in texts.iter().filter(|t| !t.text.trim().is_empty() && seen.insert(t.text.as_str())) {
        let lines = t.text.bytes().filter(|&b| b == b'\n').count() + 1;
        for (at, _) in code.match_indices(&t.text) {
            spans.push(AuthorLines {
                from: line_of(code, at),
                lines,
                file: t.file.clone(),
                line: t.line,
                what: t.what.clone(),
            });
        }
    }
    spans.sort_by_key(|s| s.from);
    spans
}

fn line_of(code: &str, offset: usize) -> usize {
    code.as_bytes()[..offset].iter().filter(|&&b| b == b'\n').count()
}
