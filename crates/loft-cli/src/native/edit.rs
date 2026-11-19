//! An edit session owns the render policy, including debounce and promotion;
//! the renderer's tables and bind groups stay live across an edit.
use super::*;
use loft_passes::tier0::{TraceEdit, authored_trace};
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

struct Pipelines {
    module: wgpu::ShaderModule,
    render: wgpu::RenderPipeline,
    shade: wgpu::ComputePipeline,
}

impl Pipelines {
    fn current(kind: &KindGpu) -> Self {
        Self {
            module: kind.module.clone(),
            render: kind.render.clone(),
            shade: kind.shade.clone(),
        }
    }
}

pub struct TraceSession {
    pub loaded: Loaded,
    kind: usize,
    normalized_shader: String,
    fallback: Pipelines,
    generation: u64,
    request: Option<(Instant, String)>,
    job: Option<(u64, String, Receiver<Result<Pipelines, String>>)>,
    cache: VecDeque<(String, Pipelines)>,
    edited: bool,
}

fn pass_kinds(passes: &[Pass], select: fn(&Pass) -> Option<&[u32]>) -> &[u32] {
    passes.iter().find_map(select).unwrap_or_default()
}

fn shadow_kinds(passes: &[Pass]) -> &[u32] {
    pass_kinds(passes, |p| {
        if let Pass::Shadow(k) = p {
            Some(k)
        } else {
            None
        }
    })
}

fn same_words(a: &[f32], b: &[f32]) -> bool {
    a.iter()
        .map(|f| f.to_bits())
        .eq(b.iter().map(|f| f.to_bits()))
}

impl TraceSession {
    pub fn new(
        g: &Gpu,
        p: &Program,
        kind: &str,
        tape: &TraceEdit,
        size: (u32, u32),
    ) -> Result<Self, String> {
        let index = p
            .kinds
            .iter()
            .position(|k| k.name == kind && k.draw == Draw::Traced)
            .ok_or("edit mode needs a drawn, sphere-traced kind")?;
        if !p.dispatches.is_empty() {
            return Err("edit mode requires a render-only program".into());
        }
        let normalized_shader = tape.shader(&p.modules[p.kinds[index].module].wgsl)?;
        let loaded = Loaded::load(g, p, size, None, Some((index, tape)), Aov::Beauty)?;
        let fallback = Pipelines::current(&loaded.kinds[index]);
        Ok(Self {
            loaded,
            kind: index,
            normalized_shader,
            fallback,
            generation: 0,
            request: None,
            job: None,
            cache: VecDeque::new(),
            edited: false,
        })
    }

    pub fn edit(
        &mut self,
        g: &Gpu,
        initial: &Program,
        src: &loft_ir::Sources,
        overrides: &[(String, f64)],
    ) -> Result<Duration, String> {
        let (next, _, compiled) =
            loft_passes::compile(src, overrides).map_err(|d| d.render(src))?;
        let kind = &initial.kinds[self.kind].name;
        let tape = authored_trace(&compiled, kind).map_err(|d| d.render(src))?;
        if !self.compatible(initial, &next) {
            return Err(
                "the edit changes the scene layout or another kind; reload the scene".into(),
            );
        }
        let kt = &next.kinds[self.kind];
        let specialized = &next.modules[kt.module].wgsl;
        if tape.shader(specialized)? != self.normalized_shader {
            return Err("the edit changes WGSL outside the trace query; reload the scene".into());
        }
        let rows = f32_bytes(&kt.rows);
        let instructions = tape.bytes();
        let instances = instance_bytes(&next);
        let shadow = self.loaded.shadow.as_ref().map(|_| {
            let (lo, hi) = caster_bounds(&next, shadow_kinds(&next.passes));
            sun_camera(next.sun, lo, hi, next.shadow_size)
        });
        let start = Instant::now();
        let dst = &self.loaded.kinds[self.kind].rows;
        g.queue.write_buffer(dst, 0, &rows);
        g.queue.write_buffer(dst, rows.len() as u64, &instructions);
        g.queue
            .write_buffer(&self.loaded.instance_bounds, 0, &instances);
        if let Some((vp, texel, dir)) = shadow {
            let s = self.loaded.shadow.as_mut().unwrap();
            (s.view_proj, s.texel, s.dir) = (vp, texel, dir);
        }
        if self.loaded.kinds[self.kind].shade != self.fallback.shade {
            self.install(
                &g.device,
                Pipelines {
                    module: self.fallback.module.clone(),
                    render: self.fallback.render.clone(),
                    shade: self.fallback.shade.clone(),
                },
            );
        }
        let upload = start.elapsed();
        self.generation = self.generation.wrapping_add(1);
        self.request = Some((
            Instant::now() + Duration::from_millis(400),
            specialized.clone(),
        ));
        self.edited = true;
        Ok(upload)
    }

    fn compatible(&self, old: &Program, new: &Program) -> bool {
        let camera = |p: &Program| {
            let c = p.camera;
            [
                c.at[0], c.at[1], c.at[2], c.radius, c.height, c.period, c.fov,
            ]
        };
        new.dispatches.is_empty()
            && old.kinds.len() == new.kinds.len()
            && old.modules.len() == new.modules.len()
            && old.instances.len() == new.instances.len()
            && old.passes == new.passes
            && old.shadow_size == new.shadow_size
            && old.sun == new.sun
            && camera(old) == camera(new)
            && same_words(&old.params, &new.params)
            && same_words(&old.luts, &new.luts)
            && old
                .kinds
                .iter()
                .zip(&new.kinds)
                .enumerate()
                .all(|(i, (a, b))| {
                    a.name == b.name
                        && a.module == b.module
                        && a.stride == b.stride
                        && a.n_rows == b.n_rows
                        && a.draw == b.draw
                        && (i == self.kind || same_words(&a.rows, &b.rows))
                })
            && old
                .modules
                .iter()
                .zip(&new.modules)
                .enumerate()
                .all(|(i, (a, b))| i == old.kinds[self.kind].module || a.wgsl == b.wgsl)
    }

    fn install(&mut self, d: &wgpu::Device, pipelines: Pipelines) {
        let kind = &mut self.loaded.kinds[self.kind];
        kind.module = pipelines.module;
        kind.render = pipelines.render;
        kind.shade = pipelines.shade;
        let l = &mut self.loaded;
        let kinds = pass_kinds(
            &l.passes,
            |p| if let Pass::Vis(k) = p { Some(k) } else { None },
        );
        l.bundle = draw_bundle(d, &l.g0, &l.kinds, kinds, &l.draw_args, None);
        if let Some(shadow) = &mut l.shadow {
            shadow.bundle = draw_bundle(
                d,
                &shadow.groups.0,
                &l.kinds,
                shadow_kinds(&l.passes),
                &shadow.args,
                Some(&shadow.groups.1),
            );
        }
    }

    /// Call from the frame loop. An in-flight result for an old tape can
    /// never replace the current pipeline.
    pub fn poll_promotion(&mut self, g: &Gpu) -> Result<bool, String> {
        if let Some((_, _, done)) = &self.job {
            let result = match done.try_recv() {
                Err(TryRecvError::Empty) => return Ok(false),
                Err(TryRecvError::Disconnected) => Err("trace pipeline worker stopped".into()),
                Ok(result) => result,
            };
            let (generation, source, _) = self.job.take().unwrap();
            if generation == self.generation {
                let ready = result?;
                self.install(
                    &g.device,
                    Pipelines {
                        module: ready.module.clone(),
                        render: ready.render.clone(),
                        shade: ready.shade.clone(),
                    },
                );
                self.cache.push_front((source, ready));
                self.cache.truncate(8);
                self.edited = false;
                return Ok(true);
            }
        }
        let Some((at, _)) = &self.request else {
            return Ok(!self.edited);
        };
        if *at > Instant::now() {
            return Ok(false);
        }
        let (_, source) = self.request.take().unwrap();
        if let Some(i) = self.cache.iter().position(|(s, _)| s == &source) {
            let item = self.cache.remove(i).unwrap();
            let p = &item.1;
            self.install(
                &g.device,
                Pipelines {
                    module: p.module.clone(),
                    render: p.render.clone(),
                    shade: p.shade.clone(),
                },
            );
            self.cache.push_front(item);
            self.edited = false;
            return Ok(true);
        }
        let (send, receive) = std::sync::mpsc::channel();
        let d = g.device.clone();
        let [g0, g1, g2] = [
            &self.loaded.bgl0,
            &self.loaded.bgl_kind,
            &self.loaded.bgl_shade,
        ]
        .map(Clone::clone);
        let wgsl = source.clone();
        std::thread::Builder::new()
            .name("loft-trace-promotion".into())
            .spawn(move || {
                let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
                let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("promoted trace"),
                    source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&wgsl)),
                });
                let render = render(
                    &d,
                    "promoted visibility",
                    &[&g0, &g1],
                    &module,
                    ("vs", "fs"),
                    VIS,
                    true,
                    Some(wgpu::Face::Front),
                    None,
                );
                let shade = compute(
                    &d,
                    "promoted shade",
                    &[&g0, &g1, &g2],
                    &module,
                    "shade",
                    None,
                );
                let result = pollster::block_on(scope.pop()).map_or(
                    Ok(Pipelines {
                        module,
                        render,
                        shade,
                    }),
                    |e| Err(e.to_string()),
                );
                let _ = send.send(result);
            })
            .map_err(|e| e.to_string())?;
        self.job = Some((self.generation, source, receive));
        Ok(false)
    }
}
