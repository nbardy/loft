//! Render modules, pipeline keys, and planned commands.
use super::*;
use crate::artefact::{Blend, Cull, Draw, DrawDepth, DrawPass, pass_bindings_wgsl};

use crate::share::Sharing;
use crate::wgsl::{POST_WGSL, SCENE_CLEARANCE_WGSL, SCENE_DEPTH_WGSL};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Variant {
    Draw,
    SceneDepth,
    Post,
}

impl Variant {
    fn name(self) -> &'static str {
        match self {
            Self::Draw => "draw",
            Self::SceneDepth => "sceneDepth",
            Self::Post => "post",
        }
    }

    fn named(self) -> &'static str {
        match self {
            Self::Draw => "",
            Self::SceneDepth => " (with SCENE_DEPTH_WGSL and SCENE_CLEARANCE_WGSL)",
            Self::Post => " (as a post pass, with POST_WGSL)",
        }
    }
}

fn draw_variant(d: &Draw) -> Variant {
    if d.scene_depth == Some(true) { Variant::SceneDepth } else { Variant::Draw }
}

fn blend(d: Blend) -> Option<BlendState> {
    let factors = |src_factor, dst_factor| BlendComponent { operation: None, src_factor, dst_factor };
    let over = factors("one", "one-minus-src-alpha");
    let (color, alpha) = match d {
        Blend::None => return None,
        // Straight alpha over.
        Blend::Alpha => (factors("src-alpha", "one-minus-src-alpha"), over),
        Blend::Premultiplied => (factors("one", "one-minus-src-alpha"), over),
        // Additive, scaled by the fragment's straight alpha. The target's alpha is left as it is.
        Blend::Add => (factors("src-alpha", "one"), factors("zero", "one")),
        // A shadow block's tone: colour untouched, alpha the lesser. WebGPU requires one/one factors with min.
        Blend::AlphaMin => (factors("zero", "one"), BlendComponent { operation: Some("min"), ..factors("one", "one") }),
        // For the printed shadow block, the target's alpha says how much colour stays, then takes the fragment's alpha.
        Blend::AlphaPull => (factors("one-minus-dst-alpha", "dst-alpha"), factors("one", "zero")),
    };
    Some(BlendState { color, alpha })
}

pub(super) struct RenderState<'a, 'b> {
    pub scene: &'b SceneObjects<'a>,
    sharing: Sharing<'a>,
    prelude_chunk: Id,
    scene_chunk: Id,
    pass_chunk: Option<Id>,
    layouts: HashMap<Variant, Id>,
    modules: HashMap<String, Id>,
    pipelines: HashMap<String, Id>,
    pub resolve: Option<super::passes::DepthResolve>,
    pub clearance: Option<super::passes::Clearance>,
}

impl<'a, 'b> RenderState<'a, 'b> {
    pub fn new(scene: &'b SceneObjects<'a>, sharing: Sharing<'a>, prelude_chunk: Id, scene_chunk: Id) -> Self {
        Self {
            scene,
            sharing,
            prelude_chunk,
            scene_chunk,
            pass_chunk: None,
            layouts: HashMap::new(),
            modules: HashMap::new(),
            pipelines: HashMap::new(),
            resolve: None,
            clearance: None,
        }
    }
}

impl Lower<'_> {
    pub(super) fn layout_of(&mut self, r: &mut RenderState<'_, '_>, variant: Variant) -> Id {
        if let Some(&id) = r.layouts.get(&variant) {
            return id;
        }
        let l = &r.scene.layouts;
        let mut layouts = vec![l.prelude, l.tables];
        match variant {
            Variant::Draw => {
                // A draw that reads no scene depth still binds group 3 when the scene has passes. In a split frame
                // the scene depth layout at group 2 also lets the transparent pass draw it.
                if l.pass.is_some() {
                    layouts.push(l.depth.or(l.empty).expect("plain draws must have a group 2 with passes"));
                }
            }
            Variant::SceneDepth => layouts.push(l.depth.expect("depth layout in a split frame")),
            Variant::Post => layouts.push(l.post.expect("post layout in a split frame")),
        }
        layouts.extend(l.pass);
        let id = self.pipeline_layout(format!("loft {}", variant.name()), layouts);
        r.layouts.insert(variant, id);
        id
    }

    /// One module per shader and way it is used. A shader used nowhere has no known host WGSL to check against.
    pub(super) fn module_of(
        &mut self,
        r: &mut RenderState<'_, '_>,
        shader: &str,
        variant: Variant,
    ) -> Result<Id, Error> {
        let shared = r.sharing.module_of(shader);
        let with_passes = r.scene.layouts.pass.is_some();
        let key = format!("{}|{}|{}", shared.key, variant.name(), if with_passes { "passes" } else { "" });
        if let Some(&id) = r.modules.get(&key) {
            return Ok(id);
        }
        let variant_chunk = match variant {
            Variant::Draw => None,
            Variant::SceneDepth => Some(self.chunk(&[SCENE_DEPTH_WGSL, SCENE_CLEARANCE_WGSL].concat())?),
            Variant::Post => Some(self.chunk(POST_WGSL)?),
        };
        // Pass bindings are the same for every module, shared or not. The key covers every chunk that varies.
        if with_passes && r.pass_chunk.is_none() {
            r.pass_chunk = Some(self.chunk(&pass_bindings_wgsl(self.a.passes.as_deref().unwrap_or_default()))?);
        }
        let prefix: Vec<_> = std::iter::once(r.prelude_chunk).chain(r.pass_chunk).chain(variant_chunk).collect();
        let shader_chunk = self.chunk(shared.wgsl)?;
        let id = self.module(
            format!("shader \"{}\"{}", shared.key, variant.named()),
            prefix.iter().copied().chain([r.scene_chunk, shader_chunk]).collect(),
        );
        if let Some(mixed) = self.mixed {
            let alternative = mixed.shaders.get(shader);
            if alternative.is_some() || mixed.scene_wgsl.is_some() {
                let enable = self.enable_f16()?;
                let scene = match &mixed.scene_wgsl {
                    Some(text) => self.chunk(text)?,
                    None => r.scene_chunk,
                };
                let shader = self.chunk(alternative.map_or(shared.wgsl, String::as_str))?;
                self.mixed_modules
                    .0
                    .push((id.to_string(), std::iter::once(enable).chain(prefix).chain([scene, shader]).collect()));
            }
        }
        r.modules.insert(key, id);
        Ok(id)
    }

    /// Equal descriptors use one pipeline. The draw index selects its shared constants.
    pub(super) fn draw_pipeline(
        &mut self,
        r: &mut RenderState<'_, '_>,
        d: &Draw,
        k: usize,
        format: &'static str,
        fragment: &str,
    ) -> Result<Id, Error> {
        let variant = draw_variant(d);
        let module = self.module_of(r, &d.shader, variant)?;
        let layout = self.layout_of(r, variant);
        let shader = r.sharing.module_of(&d.shader);
        let draw = r.sharing.draw_of(k);
        let cull = to_json(&d.cull);
        let blend_name = to_json(&d.blend);
        let key = format!(
            "{}|{}|{}|{}|{}|{}|{format}|{fragment}",
            shader.key,
            variant.name(),
            cull.trim_matches('"'),
            blend_name.trim_matches('"'),
            d.depth_write,
            constants_key(&draw.constants)
        );
        if let Some(&id) = r.pipelines.get(&key) {
            return Ok(id);
        }
        let constants = &draw.constants;
        let desc = RenderPipelineDesc {
            label: format!("{} ({})", d.shader, d.label),
            layout,
            vertex: shader_stage(module, "vs", OrderedMap(constants.clone())),
            fragment: Some(FragmentDesc {
                stage: shader_stage(module, fragment, OrderedMap(constants.clone())),
                targets: vec![ColorTarget { format, blend: blend(d.blend) }],
            }),
            primitive: primitive(d.cull),
            depth_stencil: Some(depth_stencil(d.depth_write)),
            multisample: Multisample { count: SAMPLES },
        };
        let id = add(&mut self.renders, desc);
        r.pipelines.insert(key, id);
        Ok(id)
    }

    /// Depth passes have no colour target and one sample. They use the shared module and constants of a colour draw.
    pub(super) fn depth_pipeline(
        &mut self,
        r: &mut RenderState<'_, '_>,
        d: &Draw,
        k: usize,
        depth: &DrawDepth,
    ) -> Result<Id, Error> {
        let module = self.module_of(r, &d.shader, Variant::Draw)?;
        let shader = r.sharing.module_of(&d.shader);
        let constants = &r.sharing.draw_of(k).constants;
        let (entry, fragment) = match depth {
            DrawDepth::Vertex => ("vertex", None),
            DrawDepth::Entry { entry } => (
                entry.as_str(),
                Some(FragmentDesc {
                    stage: shader_stage(module, entry, OrderedMap(constants.clone())),
                    targets: vec![],
                }),
            ),
        };
        let cull = to_json(&d.cull);
        let key = format!("depth|{}|{}|{entry}|{}", shader.key, cull.trim_matches('"'), constants_key(constants));
        if let Some(&id) = r.pipelines.get(&key) {
            return Ok(id);
        }
        let vertex = shader_stage(module, "vs", OrderedMap(constants.clone()));
        let layout = self.layout_of(r, Variant::Draw);
        let id = add(
            &mut self.renders,
            RenderPipelineDesc {
                label: format!("{} ({}, depth)", d.shader, d.label),
                layout,
                vertex,
                fragment,
                primitive: primitive(d.cull),
                depth_stencil: Some(depth_stencil(true)),
                multisample: Multisample { count: 1 },
            },
        );
        r.pipelines.insert(key, id);
        Ok(id)
    }

    pub(super) fn planned(&mut self, r: &mut RenderState<'_, '_>, pass: DrawPass) -> Result<Vec<DrawCommand>, Error> {
        self.a
            .draws
            .iter()
            .enumerate()
            .filter(|(_, d)| d.pass == pass)
            .map(|(k, d)| {
                let pipeline = self.draw_pipeline(r, d, k, COLOR_FORMAT, "fs")?;
                Ok(planned_of(r, pipeline, k))
            })
            .collect()
    }

    pub(super) fn full_screen(
        &mut self,
        label: impl Into<String>,
        layout: Id,
        module: Id,
        vs: &str,
        format: &'static str,
        fs: &str,
    ) -> Id {
        add(
            &mut self.renders,
            RenderPipelineDesc {
                label: label.into(),
                layout,
                vertex: shader_stage(module, vs, OrderedMap::default()),
                fragment: Some(FragmentDesc {
                    stage: shader_stage(module, fs, OrderedMap::default()),
                    targets: vec![ColorTarget { format, blend: None }],
                }),
                primitive: primitive(Cull::None),
                depth_stencil: None,
                multisample: Multisample { count: 1 },
            },
        )
    }
}

/// Every planned command goes here so all consumers of a shared shader select the draw's row.
pub(super) fn planned_of(r: &RenderState<'_, '_>, pipeline: Id, draw: usize) -> DrawCommand {
    let base = r.sharing.draw_of(draw).instance_base as usize;
    // Zero is omitted so a file that shares nothing retains its bytes and format version.
    DrawCommand::Planned { pipeline, draw, instance_base: (base != 0).then_some(base) }
}

fn constants_key(constants: &[(String, f64)]) -> String {
    let mut pairs: Vec<_> = constants.iter().collect();
    pairs.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    pairs.into_iter().map(|(n, v)| format!("{n}={}", to_json(v))).collect::<Vec<_>>().join(",")
}

fn shader_stage(module: Id, entry: &str, constants: OrderedMap<f64>) -> ShaderStageDesc {
    ShaderStageDesc { module, entry_point: entry.to_owned(), constants }
}

fn primitive(cull_mode: Cull) -> Primitive {
    Primitive { topology: "triangle-list", front_face: "ccw", cull_mode }
}

fn depth_stencil(depth_write_enabled: bool) -> DepthStencil {
    DepthStencil { format: DEPTH_FORMAT, depth_write_enabled, depth_compare: "less" }
}
