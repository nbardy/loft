//! Classify the validated wire columns at the lowering boundary.
use super::types::*;
use crate::error::Error;
use loft_ir::graph::{Fill, Kernel, Lens, Recur, Size, Target};
use loft_ir::wire::{Job, PortRecord, PortTag};

#[derive(Default)]
pub(super) struct Port {
    pub(super) scene: Option<SceneDecl>,
    pub(super) things: Vec<Thing>,
    pub(super) effects: Vec<Effect>,
    pub(super) style: Vec<EmitRequest>,
    pub(super) lighting: Option<Lighting>,
    pub(super) lamps: Vec<LampPass>,
    pub(super) posts: Option<Box<[Post]>>,
    pub(super) compute: Vec<ComputePass>,
    pub(super) recurrences: Vec<Recurrence>,
    pub(super) passes: Vec<PassDef>,
    pub(super) authored: Vec<Authored>,
    pub(super) cast: Option<Cast>,
    pub(super) raws: Vec<Option<RawFlags>>,
    pub(super) cameras: Vec<Camera>,
}

impl Port {
    pub(super) fn decode(job: &Job) -> Result<Self, Error> {
        let mut out = Self::default();
        if job.port.is_empty() {
            return Ok(out);
        }
        let mut passes = none(job.graph.passes.len());
        out.raws = none(job.graph.raws.len());
        for (i, d) in job.port.iter().enumerate() {
            let mut c = Cursor::new(job, d);
            let bad = |()| {
                Error::Input(format!("port declaration {i} ({}) has an invalid layout or reference", d.tag.name()))
            };
            let decoded = match d.tag {
                PortTag::Scene => put(&mut out.scene, scene(&mut c)),
                PortTag::Thing => thing(&mut c).map(|x| out.things.push(x)),
                PortTag::Effect => effect(&mut c).map(|x| out.effects.push(x)),
                PortTag::Style => style(&mut c).map(|x| out.style.push(x)),
                PortTag::Lighting => put(&mut out.lighting, lighting(&mut c)),
                PortTag::Lamp => lamp(&mut c).map(|x| out.lamps.push(x)),
                PortTag::Posts => put(&mut out.posts, posts(&mut c)),
                PortTag::Compute => compute(&mut c).map(|x| out.compute.push(x)),
                PortTag::Recurrence => recurrence(&mut c).map(|x| out.recurrences.push(x)),
                PortTag::Pass => pass_def(&mut c).and_then(|(i, x)| put(passes.get_mut(i).ok_or(())?, Ok(x))),
                PortTag::Authored => authored(&mut c).map(|x| out.authored.push(x)),
                PortTag::Cast => put(&mut out.cast, cast(&mut c)),
                PortTag::Raw => raw_flags(&mut c).and_then(|(i, x)| put(out.raws.get_mut(i).ok_or(())?, Ok(x))),
                PortTag::Camera => camera(&mut c).map(|x| out.cameras.push(x)),
            };
            decoded.and_then(|()| c.done()).map_err(bad)?;
        }
        out.passes = passes
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| Error::Input("a port pass lacks its definition".into()))?;
        if out.scene.is_none() || out.raws.iter().any(Option::is_none) {
            return Err(Error::Input("port job lacks its scene or raw function flags".into()));
        }
        Ok(out)
    }
}

fn none<T>(n: usize) -> Vec<Option<T>> {
    (0..n).map(|_| None).collect()
}

fn put<T>(slot: &mut Option<T>, value: Result<T, ()>) -> Result<(), ()> {
    if slot.is_some() {
        return Err(());
    }
    *slot = Some(value?);
    Ok(())
}

struct Cursor<'a> {
    job: &'a Job,
    words: &'a [u32],
    lits: &'a [u32],
    syms: &'a [u32],
    fns: &'a [u32],
    kernels: &'a [u32],
    blobs: &'a [[u32; 2]],
    roots: &'a [u32],
}

fn take<T: Copy>(xs: &mut &[T]) -> Result<T, ()> {
    let (x, tail) = xs.split_first().ok_or(())?;
    *xs = tail;
    Ok(*x)
}

fn many<T>(n: u32, mut read: impl FnMut() -> Result<T, ()>) -> Result<Box<[T]>, ()> {
    (0..n).map(|_| read()).collect()
}

impl<'a> Cursor<'a> {
    fn new(job: &'a Job, d: &'a PortRecord) -> Self {
        Self {
            job,
            words: &d.words,
            lits: &d.lits,
            syms: &d.syms,
            fns: &d.fns,
            kernels: &d.kernels,
            blobs: &d.blobs,
            roots: &d.roots,
        }
    }
    fn word(&mut self) -> Result<u32, ()> {
        take(&mut self.words)
    }
    fn lit(&mut self) -> Result<f64, ()> {
        Ok(f64::from_bits(self.job.graph.lits[take(&mut self.lits)? as usize]))
    }
    fn pair(&mut self) -> Result<[f64; 2], ()> {
        Ok([self.lit()?, self.lit()?])
    }
    fn triple(&mut self) -> Result<[f64; 3], ()> {
        Ok([self.lit()?, self.lit()?, self.lit()?])
    }
    fn text(&mut self) -> Result<String, ()> {
        Ok(self.job.graph.sym(take(&mut self.syms)?).to_owned())
    }
    fn opt_text(&mut self) -> Result<Option<String>, ()> {
        self.flag()?.then(|| self.text()).transpose()
    }
    fn choice<T: Copy>(&mut self, entries: &[T]) -> Result<T, ()> {
        entries.get(self.word()? as usize).copied().ok_or(())
    }
    fn flag(&mut self) -> Result<bool, ()> {
        self.choice(&[false, true])
    }
    fn root(&mut self) -> Result<u32, ()> {
        take(&mut self.roots)
    }
    fn roots(&mut self) -> Box<[u32]> {
        std::mem::take(&mut self.roots).into()
    }
    fn ty(&mut self) -> Result<Ty, ()> {
        self.choice(&Ty::ALL)
    }
    fn rate(&mut self) -> Result<Rate, ()> {
        self.choice(&[Rate::Build, Rate::Frame, Rate::Copy, Rate::Item])
    }
    fn raw_at(&self, k: u32) -> Result<u32, ()> {
        match self.job.fn_ref(k) {
            Some(FnRef::Raw(k)) => Ok(k),
            Some(FnRef::Graph(_)) | None => Err(()),
        }
    }
    fn raw(&mut self) -> Result<u32, ()> {
        let k = take(&mut self.fns)?;
        self.raw_at(k)
    }
    fn table(&mut self) -> Result<u32, ()> {
        let k = take(&mut self.kernels)?;
        matches!(self.job.graph.kernels[k as usize], Kernel::Table { .. }).then_some(k).ok_or(())
    }
    fn kept(&mut self) -> Result<u32, ()> {
        let k = self.table()?;
        matches!(self.job.graph.kernels[k as usize], Kernel::Table { fill: Fill::Recurrence(Recur::Kept(_)), .. })
            .then_some(k)
            .ok_or(())
    }
    fn done(&self) -> Result<(), ()> {
        [self.words, self.lits, self.syms, self.fns, self.kernels, self.roots]
            .into_iter()
            .all(<[u32]>::is_empty)
            .then_some(())
            .filter(|()| self.blobs.is_empty())
            .ok_or(())
    }
}

fn scene(c: &mut Cursor<'_>) -> Result<SceneDecl, ()> {
    let ground = c.choice(&[Ground::Nothing, Ground::Plane, Ground::Sea])?;
    let outline = c.choice(&[Outline::Hull, Outline::Screen])?;
    let screen_ink = c.flag()?;
    let periodic = c.flag()?;
    let sun = c.triple()?;
    let clear = c.triple()?;
    let period = periodic.then(|| c.lit()).transpose()?;
    Ok(SceneDecl { sun, ground, clear, outline, period, screen_ink })
}

fn camera(c: &mut Cursor<'_>) -> Result<Camera, ()> {
    let orthographic = c.flag()?;
    let name = c.text()?;
    let eye = c.triple()?;
    let target = c.triple()?;
    let value = c.lit()?;
    let lens = if orthographic {
        CameraLens::Orthographic { height: value }
    } else {
        CameraLens::Perspective { fov_y: value }
    };
    Ok(Camera { name, eye, target, lens })
}

fn thing(c: &mut Cursor<'_>) -> Result<Thing, ()> {
    let kind = match c.text()?.as_str() {
        "surface" => ThingKind::Surface,
        "relief" => ThingKind::Relief,
        "ground" => ThingKind::Ground,
        "sky" => ThingKind::Sky,
        "sea" => ThingKind::Sea,
        "strand" => ThingKind::Strand,
        "telegraph" => ThingKind::Telegraph,
        "smear" => ThingKind::Smear,
        "field" => ThingKind::Field,
        "traced" => ThingKind::Traced,
        "mesh" => ThingKind::Mesh,
        "copies" => ThingKind::Copies,
        "impostors" => ThingKind::Impostors,
        "shader" => ThingKind::Shader,
        "cast" => ThingKind::Cast,
        "light" => ThingKind::Light,
        "shadows" => ThingKind::Shadows,
        "volume" => ThingKind::Volume,
        "density" => ThingKind::Density,
        "glow" => ThingKind::Glow,
        "lightning" => ThingKind::Lightning,
        "rain" => ThingKind::Rain,
        "specks" => ThingKind::Specks,
        "wake" => ThingKind::Wake,
        s => ThingKind::Unowned(s.to_owned()),
    };
    let name = c.text()?;
    let (nw, nl, nf, nb) = (c.word()?, c.word()?, c.word()?, c.word()?);
    let payload = ThingPayload {
        words: many(nw, || c.word())?,
        lits: many(nl, || c.lit())?,
        fns: many(nf, || c.job.fn_ref(take(&mut c.fns)?).ok_or(()))?,
        blobs: many(nb, || {
            let [offset, length] = take(&mut c.blobs)?;
            Ok(BlobRange { offset, length })
        })?,
    };
    let tape = Emitted {
        shaders: many(c.word()?, || shader(c))?,
        draws: many(c.word()?, || draw(c))?,
        posts: posts(c)?,
        notes: many(c.word()?, || c.text())?,
    };
    Ok(Thing { kind, name, payload, tape })
}

fn shader(c: &mut Cursor<'_>) -> Result<Shader, ()> {
    Ok(Shader {
        key: c.text()?,
        code: c.text()?,
        depth_entry: c.opt_text()?,
        mixed_code: c.opt_text()?,
        libs: many(c.word()?, || c.raw())?,
    })
}

fn posts(c: &mut Cursor<'_>) -> Result<Box<[Post]>, ()> {
    many(c.word()?, || Ok(Post { label: c.text()?, wgsl: c.text()?, mixed_wgsl: c.opt_text()? }))
}

fn draw(c: &mut Cursor<'_>) -> Result<DrawSpec, ()> {
    let label = c.text()?;
    let shader = c.text()?;
    let scene_pass_fragment = c.opt_text()?;
    let vertices = c.lit()?;
    let instances = c.lit()?;
    let phase = c.choice(&[
        Phase::Background,
        Phase::Opaque,
        Phase::OnBodies,
        Phase::Water,
        Phase::OnWater,
        Phase::Transparent,
    ])?;
    let cull = c.choice(&[Cull::None, Cull::Back, Cull::Front])?;
    let depth_write = c.flag()?;
    let blend =
        c.choice(&[Blend::None, Blend::Alpha, Blend::Premultiplied, Blend::Add, Blend::AlphaMin, Blend::AlphaPull])?;
    let layer = c.choice(&[Layer::Scene, Layer::Construction])?;
    let scene_depth = c.choice(&[None, Some(false), Some(true)])?;
    let constants = many(c.word()?, || Ok((c.text()?, c.lit()?)))?;
    let depth = match c.word()? {
        0 => None,
        1 => Some(DrawDepth::Vertex),
        2 => Some(DrawDepth::Entry(c.text()?)),
        _ => return Err(()),
    };
    let detail = match c.word()? {
        0 => DetailSpec::Fixed,
        1 => DetailSpec::Grid {
            bound: c.table()?,
            counts: many(c.word()?, || c.pair())?,
            point: c.root()?,
            anchor: c.root()?,
        },
        2 => DetailSpec::Projected { cell: c.lit()?, plane: c.lit()?, reach: c.lit()?, lift: c.lit()? },
        _ => return Err(()),
    };
    Ok(DrawSpec {
        label,
        shader,
        scene_pass_fragment,
        vertices,
        instances,
        phase,
        cull,
        depth_write,
        blend,
        layer,
        scene_depth,
        constants,
        depth,
        detail,
    })
}

fn effect(c: &mut Cursor<'_>) -> Result<Effect, ()> {
    let stage = c.choice(&[
        Stage::Compute,
        Stage::Things,
        Stage::Style,
        Stage::Passes,
        Stage::Cast,
        Stage::Layout,
        Stage::Rest,
        Stage::Details,
    ])?;
    let op = match c.word()? {
        0 => EffectOp::Register(c.roots()),
        1 => EffectOp::Declare(c.table()?),
        2 => EffectOp::Lower(Program {
            name: c.text()?,
            roots: c.roots(),
            runs: c.rate()?,
            vars: many(c.words.len() as u32, || Ok((c.text()?, c.rate()?)))?,
        }),
        _ => return Err(()),
    };
    Ok(Effect { stage, op })
}

fn style(c: &mut Cursor<'_>) -> Result<EmitRequest, ()> {
    let pick = c.word()?;
    let name = c.text()?;
    let ret = c.text()?;
    let roots = c.roots();
    if pick as usize >= roots.len() {
        return Err(());
    }
    let args = many(c.words.len() as u32, || Ok((c.text()?, c.ty()?)))?;
    Ok(EmitRequest { name, args, ret, roots, pick })
}

fn lighting(c: &mut Cursor<'_>) -> Result<Lighting, ()> {
    let sampling = c.choice(&[Sampling::Point, Sampling::Sampled])?;
    let sun = match c.word()? {
        0 => Sun::Fixed(c.triple()?),
        1 => Sun::Input { name: c.text()?, dir: c.triple()? },
        2 => Sun::Expression(c.root()?),
        _ => return Err(()),
    };
    let amount = if c.flag()? { SunAmount::Given(c.root()?) } else { SunAmount::Full };
    let (at, xi, normal) = (c.root()?, c.root()?, c.root()?);
    let lights = many(c.word()?, || {
        let (distance, size, shadow) = (c.flag()?, c.flag()?, c.flag()?);
        Ok(Arrival {
            toward: c.root()?,
            amount: c.root()?,
            distance: distance.then(|| c.root()).transpose()?,
            size: size.then(|| c.root()).transpose()?,
            shadow: shadow.then(|| Ok(Shadow { point: c.root()?, surface: c.root()? })).transpose()?,
        })
    })?;
    Ok(Lighting { sun, amount, sampling, at, xi, normal, lights })
}

fn lamp(c: &mut Cursor<'_>) -> Result<LampPass, ()> {
    Ok(LampPass { light: c.word()?, pass: c.text()?, at: c.root()?, half: c.root()? })
}

/// The compute Kernel owns each output. The pass declaration preserves the consumer's alias and read order.
fn compute(c: &mut Cursor<'_>) -> Result<ComputePass, ()> {
    let kernel = take(&mut c.kernels)?;
    if !matches!(c.job.graph.kernels[kernel as usize], Kernel::Compute(_)) {
        return Err(());
    }
    let reads = many(c.syms.len() as u32, || {
        let symbol = c.text()?;
        let kernel = take(&mut c.kernels)?;
        let output = c.word()?;
        let Kernel::Compute(producer) = &c.job.graph.kernels[kernel as usize] else {
            return Err(());
        };
        if producer.writes.get(output as usize).is_none() {
            return Err(());
        }
        Ok(ComputeRead { symbol, kernel, output })
    })?;
    Ok(ComputePass { kernel, reads })
}

fn recurrence(c: &mut Cursor<'_>) -> Result<Recurrence, ()> {
    let name = c.text()?;
    let total = c.word()?;
    let n = c.word()?;
    let replay = c.flag()?;
    let start = c.lit()?;
    let kernel = c.kept()?;
    let after = many(n, || c.kept())?;
    let replay = replay.then(|| Ok(Replay { even: c.table()?, odd: c.table()?, step: c.root()? })).transpose()?;
    Ok(Recurrence { kernel, name, total, start, after, replay })
}

fn pass_def(c: &mut Cursor<'_>) -> Result<(usize, PassDef), ()> {
    let ordinal = c.word()? as usize;
    let discovery_order = c.flag()?.then(|| c.word()).transpose()?;
    let texture = c.text()?;
    let has_shader = c.flag()?;
    let p = c.job.graph.passes.get(ordinal).ok_or(())?;
    let (name, size, what) = match p.target {
        Target::Texture { name, size, keep, .. } => {
            let Lens::Table { kernel: camera } = c.job.graph.cameras[p.camera as usize].lens else {
                return Err(());
            };
            (name, size, PassTarget::Scene { keep, camera })
        }
        Target::Screen { name, size, uv, colour } => {
            let uv = c.job.graph.sym(uv).to_owned();
            (name, size, PassTarget::Screen { colour, uv, shader: has_shader.then(|| shader(c)).transpose()? })
        }
        Target::View => return Err(()),
    };
    let size = match size {
        Size::View(at) => TextureSize::View { scale: f64::from_bits(c.job.graph.lits[at as usize]) },
        Size::Fixed(width, height) => TextureSize::Fixed { width, height },
    };
    let name = c.job.graph.sym(name).to_owned();
    Ok((ordinal, PassDef { name, texture, size, what, discovery_order }))
}

fn authored(c: &mut Cursor<'_>) -> Result<Authored, ()> {
    Ok(Authored { text: c.text()?, file: c.text()?, line: c.word()?, what: c.text()? })
}

fn cast(c: &mut Cursor<'_>) -> Result<Cast, ()> {
    Ok(Cast { wgsl: c.text()?, libs: many(c.fns.len() as u32, || c.raw())? })
}

fn raw_flags(c: &mut Cursor<'_>) -> Result<(usize, RawFlags), ()> {
    let ordinal = c.word()?;
    let raw = c.raw_at(ordinal)? as usize;
    Ok((raw, RawFlags { gpu_only: c.flag()?, raw_compute: c.opt_text()? }))
}
