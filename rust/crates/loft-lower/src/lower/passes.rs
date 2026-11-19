//! The view and off-screen passes, with shared depth resolve and clearance pipelines.
use super::render::{Variant, planned_of};
use super::*;
use crate::artefact::{ArtefactPass, DrawPass, PassKeep, PassWhat, TextureSize};
use crate::wgsl::{CLEARANCE_WGSL, DEPTH_RESOLVE_WGSL};

const RA: &str = "RENDER_ATTACHMENT";
const TB: &str = "TEXTURE_BINDING";
const ONE_PIXEL: TextureSize = TextureSize::Fixed { width: 1, height: 1 };
/// cover_vs draws one full-screen triangle, in both the view and the colour passes.
const CLEARANCE_VERTICES: usize = 3;

pub(super) const fn keep_of(p: &ArtefactPass) -> PassKeep {
    match p.what {
        PassWhat::Screen { .. } => PassKeep::Light,
        PassWhat::Scene { keep, .. } => keep,
    }
}

impl PassKeep {
    const fn format(self) -> &'static str {
        match self {
            Self::Colour => COLOR_FORMAT,
            Self::Light => LIGHT_FORMAT,
            Self::Depth => DEPTH_FORMAT,
        }
    }

    pub(super) const fn sample_type(self) -> SampleType {
        match self {
            Self::Colour | Self::Light => SampleType::Float,
            Self::Depth => SampleType::UnfilterableFloat,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct DepthResolve {
    layout: Id,
    pipeline: Id,
}

#[derive(Clone, Copy)]
pub(super) struct Clearance {
    input: Id,
    nearest: Id,
    steps: [(&'static str, Id); 3],
}

pub(super) struct PassObjects<'a> {
    view_group: Id,
    pub copies: Vec<BufferCopy>,
    pub lowered: Vec<LoweredPass<'a>>,
}

pub(super) struct LoweredPass<'a> {
    pass: &'a ArtefactPass,
    picture: Id,
    own: Id,
    frame: Id,
}

impl<'a> Lower<'a> {
    fn texture(
        &mut self,
        label: impl Into<String>,
        size: Option<TextureSize>,
        format: &'static str,
        sample_count: usize,
        usage: &'static [&'static str],
    ) -> Id {
        add(
            &mut self.textures,
            TextureDesc { label: label.into(), size, extent: None, format, sample_count, usage, init: None },
        )
    }

    fn sampler(&mut self, label: &'static str, filter: &'static str) -> Id {
        add(
            &mut self.samplers,
            SamplerDesc {
                label,
                mag_filter: filter,
                min_filter: filter,
                address_mode_u: "clamp-to-edge",
                address_mode_v: "clamp-to-edge",
            },
        )
    }

    /// The first frame structure to need a resolve or clearance creates their shared objects.
    fn depth_resolve(&mut self, r: &mut RenderState<'_, '_>) -> Result<DepthResolve, Error> {
        if let Some(resolve) = r.resolve {
            return Ok(resolve);
        }
        let label = "loft depth resolve";
        let layout = self.bgl(label, vec![texture_layout(0, &[Stage::Fragment], SampleType::Depth, true)]);
        let chunk = self.chunk(DEPTH_RESOLVE_WGSL)?;
        let module = self.module(label, vec![chunk]);
        let pipeline_layout = self.pipeline_layout(label, vec![layout]);
        let pipeline = self.full_screen(label, pipeline_layout, module, "vs", SCENE_DEPTH_FORMAT, "fs");
        let resolve = DepthResolve { layout, pipeline };
        r.resolve = Some(resolve);
        Ok(resolve)
    }

    fn clearance(&mut self, r: &mut RenderState<'_, '_>) -> Result<Clearance, Error> {
        if let Some(clearance) = r.clearance {
            return Ok(clearance);
        }
        // The clearance map has three passes, each reading the previous one's texture.
        let input = self.bgl(
            "loft clearance input",
            vec![
                texture_layout(0, DRAWN, SampleType::UnfilterableFloat, false),
                sampler_layout(1, SamplerType::NonFiltering),
            ],
        );
        let nearest = self.sampler("loft clearance", "nearest");
        let layout = self.pipeline_layout("loft clearance", vec![input]);
        let chunk = self.chunk(CLEARANCE_WGSL)?;
        let module = self.module("loft clearance", vec![chunk]);
        // A full-screen triangle covers each step; fragments past that step's grid discard.
        let steps = ["quads", "rows", "clear"].map(|step| {
            (
                step,
                self.full_screen(
                    format!("loft clearance {step}"),
                    layout,
                    module,
                    "cover_vs",
                    "r8unorm",
                    &format!("{step}_fs"),
                ),
            )
        });
        let clearance = Clearance { input, nearest, steps };
        r.clearance = Some(clearance);
        Ok(clearance)
    }

    fn clearance_textures(&mut self, label: &str, size: Option<TextureSize>, clearance: Clearance) -> [Id; 3] {
        clearance
            .steps
            .map(|(step, _)| self.texture(format!("{label} clearance {step}"), size, "r8unorm", 1, &[RA, TB]))
    }

    fn clearance_passes(
        &mut self,
        label: &str,
        clearance: Clearance,
        scene_depth: Id,
        textures: [Id; 3],
    ) -> [RenderPassDesc; 3] {
        let mut previous = scene_depth;
        let mut textures = textures.into_iter();
        clearance.steps.map(|(step, pipeline)| {
            let label = format!("{label} clearance {step}");
            let group = self.group(
                format!("{label} input"),
                clearance.input,
                vec![bind(0, Resource::Texture(previous)), bind(1, Resource::Sampler(clearance.nearest))],
            );
            let texture = textures.next().expect("one texture per clearance step");
            previous = texture;
            fixed_pass(label, target(texture), [0.0; 4], vec![group], pipeline, CLEARANCE_VERTICES)
        })
    }

    pub(super) fn view_passes(
        &mut self,
        r: &mut RenderState<'_, '_>,
        offscreen: Option<&PassObjects<'_>>,
        opaque: Vec<DrawCommand>,
        transparent: Vec<DrawCommand>,
    ) -> Result<(Vec<RenderPassDesc>, Option<Id>), Error> {
        let l = &r.scene.layouts;
        let g3 = offscreen.map(|p| p.view_group);
        let groups = |group2| [l.group0, l.group1].into_iter().chain(group2).chain(g3).collect();
        let colour = self.texture("loft colour x4", None, COLOR_FORMAT, SAMPLES, &[RA]);
        let clear = self.clear_value();
        let Some(depth_layout) = l.depth else {
            let depth = self.texture("loft depth x4", None, DEPTH_FORMAT, SAMPLES, &[RA]);
            let filler = l.empty.map(|empty| self.group("loft none", empty, vec![]));
            return Ok((
                vec![RenderPassDesc {
                    label: "loft scene".into(),
                    color: Some(color_clear(target(colour), Some(Target::Output), "discard", clear)),
                    depth: Some(depth_clear(depth, "discard")),
                    bind_groups: groups(filler),
                    commands: opaque.into_iter().chain(transparent).collect(),
                }],
                filler,
            ));
        };

        let depth = self.texture("loft depth x4", None, DEPTH_FORMAT, SAMPLES, &[RA, TB]);
        let scene_depth = self.texture("loft scene depth", None, SCENE_DEPTH_FORMAT, 1, &[RA, TB]);
        let resolve = self.depth_resolve(r)?;
        let post = self.a.post.as_deref().unwrap_or_default();
        let post_pipelines = post
            .iter()
            .map(|p| {
                let layout = self.layout_of(r, Variant::Post);
                let module = self.module_of(r, &p.shader, Variant::Post)?;
                Ok(self.full_screen(
                    format!("{} ({})", p.shader, p.label),
                    layout,
                    module,
                    "post_vs",
                    COLOR_FORMAT,
                    "fs",
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let sampler = self.sampler("loft post", "linear");
        let resolve_group = self.group("loft depth resolve", resolve.layout, vec![bind(0, Resource::Texture(depth))]);
        let clearance = self.clearance(r)?;
        let steps = self.clearance_textures("loft", None, clearance);
        let depth_group = self.group(
            "loft scene depth",
            depth_layout,
            vec![bind(0, Resource::Texture(scene_depth)), bind(1, Resource::Texture(steps[2]))],
        );
        // Pass k reads intermediate (k - 1) % 2 and writes k % 2. A single post pass needs one intermediate.
        let n = post.len();
        let intermediates: Vec<_> = (0..n.min(2))
            .map(|i| {
                let texture = self.texture(format!("loft post {i}"), None, COLOR_FORMAT, 1, &[RA, TB]);
                let group = self.group(
                    format!("loft post input {i}"),
                    l.post.expect("post layout in split frame"),
                    vec![
                        bind(0, Resource::Texture(scene_depth)),
                        bind(1, Resource::Texture(texture)),
                        bind(2, Resource::Sampler(sampler)),
                    ],
                );
                (texture, group)
            })
            .collect();
        // Stage 0 is the scene. The last stage writes the output.
        let output_of = |k: usize| if k == n { Target::Output } else { target(intermediates[k % 2].0) };
        let mut passes = vec![
            RenderPassDesc {
                label: "loft scene opaque".into(),
                color: Some(color_clear(target(colour), None, "store", clear)),
                depth: Some(depth_clear(depth, "store")),
                bind_groups: groups(g3.map(|_| depth_group)),
                commands: opaque,
            },
            fixed_pass(
                "loft depth resolve".into(),
                target(scene_depth),
                [1.0, 0.0, 0.0, 1.0],
                vec![resolve_group],
                resolve.pipeline,
                3,
            ),
        ];
        passes.extend(self.clearance_passes("loft", clearance, scene_depth, steps));
        passes.push(RenderPassDesc {
            label: "loft scene transparent".into(),
            color: Some(color_load(colour, output_of(0))),
            depth: Some(depth_load(depth)),
            // Pipelines whose layout stops at group 1 ignore group 2.
            bind_groups: groups(Some(depth_group)),
            commands: transparent,
        });
        passes.extend(post_pipelines.into_iter().enumerate().map(|(i, pipeline)| {
            fixed_pass(
                format!("loft post {i}"),
                output_of(i + 1),
                [0.0, 0.0, 0.0, 1.0],
                groups(Some(intermediates[i % 2].1)),
                pipeline,
                3,
            )
        }));
        Ok((passes, Some(depth_group)))
    }

    /// Allocate pass objects before the view. Record their commands after the view's pipelines exist.
    pub(super) fn lower_pass_objects(&mut self, scene: &SceneObjects<'_>) -> PassObjects<'a> {
        let passes = self.a.passes.as_deref().unwrap_or_default();
        let layout = scene.layouts.pass.expect("pass layout when there are passes");
        let sampler = self.sampler("loft pass", "linear");
        let pictures: Vec<_> = passes
            .iter()
            .map(|p| self.texture(format!("loft pass {}", p.name), Some(p.size), keep_of(p).format(), 1, &[RA, TB]))
            .collect();
        let stand_in = self.texture("loft pass stand-in", Some(ONE_PIXEL), COLOR_FORMAT, 1, &[TB]);
        let stand_in_depth = self.texture("loft pass stand-in (depth)", Some(ONE_PIXEL), SCENE_DEPTH_FORMAT, 1, &[TB]);
        // In pass up_to, its own picture and all later pictures are replaced by stand-ins.
        let group3 = |lower: &mut Self, label: String, up_to| {
            let entries = std::iter::once(bind(0, Resource::Sampler(sampler)))
                .chain(passes.iter().zip(&pictures).enumerate().map(|(i, (p, &picture))| {
                    let texture = if i < up_to {
                        picture
                    } else {
                        match keep_of(p) {
                            PassKeep::Depth => stand_in_depth,
                            PassKeep::Colour | PassKeep::Light => stand_in,
                        }
                    };
                    bind(i + 1, Resource::Texture(texture))
                }))
                .collect();
            lower.group(label, layout, entries)
        };
        let view_group = group3(self, "loft passes (view)".into(), passes.len());
        let mut copies = Vec::new();
        let lowered = passes
            .iter()
            .zip(&pictures)
            .enumerate()
            .map(|(i, (p, &picture))| {
                let own = group3(self, format!("loft passes (in {})", p.name), i);
                let frame = match &p.what {
                    PassWhat::Screen { .. } => scene.layouts.group0,
                    PassWhat::Scene { camera, .. } => {
                        // Copy the pass camera table after the pose stages. Group 0 also binds shared rows when present.
                        let label = format!("loft pass {} frame", p.name);
                        let buf = self.buffer(&label, FRAME_BYTES, UNIFORM, None);
                        copies.push(BufferCopy {
                            from: scene.buffer_of[camera.buffer.as_str()],
                            from_offset: camera.offset * 16,
                            to: buf,
                            bytes: FRAME_BYTES,
                        });
                        self.group(label, scene.layouts.prelude, scene.prelude.bindings(buf))
                    }
                };
                LoweredPass { pass: p, picture, own, frame }
            })
            .collect();
        PassObjects { view_group, copies, lowered }
    }

    /// A pass is drawn in artefact order, after every pass it reads. Group 2 uses the view's filler when unused.
    pub(super) fn record(
        &mut self,
        r: &mut RenderState<'_, '_>,
        lowered: Vec<LoweredPass<'a>>,
        filler: Option<Id>,
    ) -> Result<Vec<RenderPassDesc>, Error> {
        let empty = filler.expect("off-screen passes bind group 3, so a group 2 filler must exist");
        let group1 = r.scene.layouts.group1;
        let mut passes = Vec::new();
        for p in lowered {
            let label = format!("loft pass {}", p.pass.name);
            let groups = vec![p.frame, group1, empty, p.own];
            match &p.pass.what {
                PassWhat::Screen { shader } => {
                    let layout = self.layout_of(r, Variant::Draw);
                    let module = self.module_of(r, shader, Variant::Draw)?;
                    let pipeline = self.full_screen(
                        format!("{shader} (pass {})", p.pass.name),
                        layout,
                        module,
                        "vs",
                        LIGHT_FORMAT,
                        "fs",
                    );
                    passes.push(fixed_pass(label, target(p.picture), [0.0; 4], groups, pipeline, 3));
                }
                PassWhat::Scene { keep: PassKeep::Depth, draws, .. } => {
                    let commands = draws
                        .iter()
                        .map(|&k| {
                            let d = &self.a.draws[k];
                            let pipeline =
                                self.depth_pipeline(r, d, k, d.depth.as_ref().expect("checked depth pass draw"))?;
                            Ok(planned_of(r, pipeline, k))
                        })
                        .collect::<Result<_, Error>>()?;
                    passes.push(RenderPassDesc {
                        label,
                        color: None,
                        depth: Some(depth_clear(p.picture, "store")),
                        bind_groups: groups,
                        commands,
                    });
                }
                PassWhat::Scene { keep: keep @ (PassKeep::Colour | PassKeep::Light), draws, .. } => {
                    let format = keep.format();
                    let drawn = || draws.iter().map(|&k| (&self.a.draws[k], k));
                    let mut cmds = |pass| {
                        drawn()
                            .filter(|(d, _)| d.pass == pass)
                            .map(|(d, k)| {
                                let pipeline = self.draw_pipeline(
                                    r,
                                    d,
                                    k,
                                    format,
                                    d.scene_pass_fragment.as_deref().unwrap_or("fs"),
                                )?;
                                Ok(planned_of(r, pipeline, k))
                            })
                            .collect::<Result<_, Error>>()
                    };
                    let opaque = cmds(DrawPass::Opaque)?;
                    let transparent = cmds(DrawPass::Transparent)?;
                    let split = drawn().any(|(d, _)| d.scene_depth == Some(true));
                    passes.extend(self.colour_pass(r, &p, format, empty, opaque, transparent, split)?);
                }
            }
        }
        Ok(passes)
    }

    /// The view's frame structure at the pass's size, resolved into its picture. A depth-reading draw splits it.
    #[allow(clippy::too_many_arguments)]
    fn colour_pass(
        &mut self,
        r: &mut RenderState<'_, '_>,
        p: &LoweredPass<'_>,
        format: &'static str,
        empty: Id,
        opaque: Vec<DrawCommand>,
        transparent: Vec<DrawCommand>,
        split: bool,
    ) -> Result<Vec<RenderPassDesc>, Error> {
        let label = format!("loft pass {}", p.pass.name);
        let size = Some(p.pass.size);
        let x4 = self.texture(format!("{label} colour x4"), size, format, SAMPLES, &[RA]);
        let depth = self.texture(
            format!("{label} depth x4"),
            size,
            DEPTH_FORMAT,
            SAMPLES,
            if split { &[RA, TB] } else { &[RA] },
        );
        let groups = |group2| vec![p.frame, r.scene.layouts.group1, group2, p.own];
        let clear = self.clear_value();
        let output = target(p.picture);
        if !split {
            return Ok(vec![RenderPassDesc {
                label,
                color: Some(color_clear(target(x4), Some(output), "discard", clear)),
                depth: Some(depth_clear(depth, "discard")),
                bind_groups: groups(empty),
                commands: opaque.into_iter().chain(transparent).collect(),
            }]);
        }
        let scene_depth = self.texture(format!("{label} scene depth"), size, SCENE_DEPTH_FORMAT, 1, &[RA, TB]);
        let resolve = self.depth_resolve(r)?;
        let clearance = self.clearance(r)?;
        let steps = self.clearance_textures(&label, size, clearance);
        let depth_group = self.group(
            format!("{label} scene depth"),
            r.scene.layouts.depth.expect("split pass has depth layout"),
            vec![bind(0, Resource::Texture(scene_depth)), bind(1, Resource::Texture(steps[2]))],
        );
        let resolve_label = format!("{label} depth resolve");
        let resolve_group = self.group(&resolve_label, resolve.layout, vec![bind(0, Resource::Texture(depth))]);
        let mut passes = vec![
            RenderPassDesc {
                label: format!("{label} opaque"),
                color: Some(color_clear(target(x4), None, "store", clear)),
                depth: Some(depth_clear(depth, "store")),
                bind_groups: groups(empty),
                commands: opaque,
            },
            fixed_pass(
                resolve_label,
                target(scene_depth),
                [1.0, 0.0, 0.0, 1.0],
                vec![resolve_group],
                resolve.pipeline,
                3,
            ),
        ];
        passes.extend(self.clearance_passes(&label, clearance, scene_depth, steps));
        passes.push(RenderPassDesc {
            label: format!("{label} transparent"),
            color: Some(color_load(x4, output)),
            depth: Some(depth_load(depth)),
            bind_groups: groups(depth_group),
            commands: transparent,
        });
        Ok(passes)
    }

    fn clear_value(&self) -> [f64; 4] {
        let [r, g, b] = self.a.clear;
        [r, g, b, 1.0]
    }
}

fn bind(binding: usize, resource: Resource) -> BindGroupEntry {
    BindGroupEntry { binding, resource }
}

fn target(texture: Id) -> Target {
    Target::Texture { texture }
}

fn color_clear(
    view: Target,
    resolve_target: Option<Target>,
    store_op: &'static str,
    clear: [f64; 4],
) -> ColorAttachment {
    ColorAttachment { view, resolve_target, load_op: "clear", store_op, clear_value: Some(clear) }
}

fn color_load(texture: Id, resolve: Target) -> ColorAttachment {
    ColorAttachment {
        view: target(texture),
        resolve_target: Some(resolve),
        load_op: "load",
        store_op: "discard",
        clear_value: None,
    }
}

fn depth_clear(view: Id, depth_store_op: &'static str) -> DepthAttachment {
    DepthAttachment { view, depth_clear_value: Some(1), depth_load_op: "clear", depth_store_op }
}

fn depth_load(view: Id) -> DepthAttachment {
    DepthAttachment { view, depth_clear_value: None, depth_load_op: "load", depth_store_op: "discard" }
}

fn fixed_pass(
    label: String,
    view: Target,
    clear: [f64; 4],
    bind_groups: Vec<Id>,
    pipeline: Id,
    vertices: usize,
) -> RenderPassDesc {
    RenderPassDesc {
        label,
        color: Some(color_clear(view, None, "store", clear)),
        depth: None,
        bind_groups,
        commands: vec![DrawCommand::Fixed { pipeline, vertices, instances: 1 }],
    }
}
