//! Live edits of one kind: recompile the scene, refuse anything that changed outside the kind, then swap
//! the kind's recompiled render and shade pipelines, rows and culling bounds into the loaded scene.
use super::*;
use loft_runner::frame::ModuleRecipe;
use std::time::Duration;

pub struct EditReceipt {
    /// Parse, check, bounds and WGSL emission of the edited scene.
    pub compile: Duration,
    /// The kind's module and its two pipelines.
    pub pipelines: Duration,
    pub upload: Duration,
    /// From the edit's arrival to the new pipelines being installed.
    pub elapsed: Duration,
    pub dirtied_rows: Vec<u32>,
    pub caster_changed: bool,
}

pub struct EditSession {
    pub loaded: Loaded,
    program: Program,
    kind: usize,
    recipe: ModuleRecipe,
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.iter().map(|x| x.to_bits()).eq(b.iter().map(|x| x.to_bits()))
}

/// What the edit changed outside kind `k`, if anything; the session swaps only that kind.
fn outside_kind(old: &Program, new: &Program, k: usize) -> Option<&'static str> {
    let m = old.kinds[k].module;
    let modules = old.modules.len() == new.modules.len()
        && old.modules.iter().zip(&new.modules).enumerate().all(|(i, (a, b))| i == m || a.wgsl == b.wgsl);
    let kinds = old.kinds.len() == new.kinds.len()
        && old.kinds.iter().zip(&new.kinds).enumerate().all(|(i, (a, b))| a.name == b.name && a.module == b.module
            && a.stride == b.stride && a.n_rows == b.n_rows && a.draw == b.draw && (i == k || same(&a.rows, &b.rows)));
    let instances = old.instances.len() == new.instances.len()
        && old.instances.iter().zip(&new.instances).all(|(a, b)| a.kind == b.kind && a.row == b.row
            && (a.kind as usize == k || (a.lo == b.lo && a.hi == b.hi)));
    [
        (modules, "another module's text"),
        (kinds, "the kind layout or another kind's rows"),
        (instances, "the instance layout or another kind's bounds"),
        (same(&old.params, &new.params) && old.param_ranges == new.param_ranges, "the parameters"),
        (same(&old.luts, &new.luts) && old.priors == new.priors, "the LUTs"),
        (old.passes == new.passes && old.shadow_size == new.shadow_size
            && (old.cull, old.classify, old.present) == (new.cull, new.classify, new.present), "the passes"),
        (format!("{:?} {:?}", old.camera, old.sun) == format!("{:?} {:?}", new.camera, new.sun), "the camera or sun"),
        (new.dispatches.is_empty() && new.bakes.is_empty() && new.path.is_none(), "motion, bakes or path tracing"),
    ].into_iter().find(|(unchanged, _)| !unchanged).map(|(_, what)| what)
}

impl EditSession {
    pub fn new(g: &Gpu, program: Program, kind: &str, size: (u32, u32)) -> Result<Self, String> {
        let k = program.kinds.iter().position(|t| t.name == kind).ok_or_else(|| format!("no kind named {kind}"))?;
        if !program.dispatches.is_empty() || !program.bakes.is_empty() || program.path.is_some() {
            return Err("live edits need a render-only scene: no motion, bakes or path tracing".into());
        }
        let mut loaded = Loaded::load(g, &program, size, None, Some(k), Aov::Beauty)?;
        let (recipe, _) = loaded.runner.get_mut().unwrap().module_pipelines(loaded.id, loaded.modules[k])
            .map_err(|_| format!("kind {kind} has no drawn module with a shade dispatch to replace"))?;
        Ok(Self { loaded, program, kind: k, recipe })
    }

    /// Either the whole edit is live when this returns, or nothing changed.
    pub fn edit(&mut self, g: &Gpu, src: &loft_passes::source::Graph, overrides: &[(String, f64)], start: Instant)
        -> Result<EditReceipt, String> {
        let (next, _, _) = loft_passes::compile(src, overrides).map_err(|d| d.render(src))?;
        let compiled = Instant::now();
        let k = self.kind;
        if let Some(what) = outside_kind(&self.program, &next, k) {
            return Err(format!("the edit changes {what} outside kind {}; reload the scene", next.kinds[k].name));
        }
        let module = next.kinds[k].module;
        // The same lowering the load gave this module, for the radiance format the load resolved.
        let hdr = loft_ir::precision::Role::Radiance.format(self.loaded.precision);
        let wgsl = loft_passes::bundle::replacement(&next.modules[module].wgsl, hdr)?;
        let scope = g.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let pipelines = self.recipe.compile(&g.device, &wgsl);
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(e.to_string());
        }
        let created = Instant::now();

        let (old, new) = (&self.program.kinds[k], &next.kinds[k]);
        let stride = new.stride as usize;
        let dirtied_rows: Vec<u32> = (0..new.n_rows as usize)
            .filter(|r| !same(&old.rows[r * stride..][..stride], &new.rows[r * stride..][..stride]))
            .map(|r| r as u32).collect();
        let moved: Vec<usize> = (0..next.instances.len()).filter(|&i| {
            let (a, b) = (&self.program.instances[i], &next.instances[i]);
            a.lo != b.lo || a.hi != b.hi
        }).collect();
        let casters = next.passes.iter().find_map(|p| if let Pass::Shadow(c) = p { Some(c.as_slice()) } else { None })
            .unwrap_or_default();
        let caster_changed = !moved.is_empty() && casters.contains(&(k as u32));
        let l = &mut self.loaded;
        let rows = l.rows(k)?;
        for &r in &dirtied_rows {
            let at = r as usize * stride;
            g.queue.write_buffer(&rows, (at * 4) as u64, &f32_bytes(&new.rows[at..at + stride]));
        }
        for &i in &moved {
            let b = next.instances[i];
            let mut bytes = [0; 32];
            bytes[..12].copy_from_slice(&f32_bytes(&b.lo));
            bytes[12..16].copy_from_slice(&b.kind.to_le_bytes());
            bytes[16..28].copy_from_slice(&f32_bytes(&b.hi));
            bytes[28..].copy_from_slice(&b.row.to_le_bytes());
            g.queue.write_buffer(&l.instance_bounds, (i * bytes.len()) as u64, &bytes);
        }
        if caster_changed {
            let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
            for inst in next.instances.iter().filter(|i| casters.contains(&i.kind)) {
                for axis in 0..3 {
                    lo[axis] = lo[axis].min(inst.lo[axis]);
                    hi[axis] = hi[axis].max(inst.hi[axis]);
                }
            }
            l.light_view = Some(sun_camera(l.sun, lo, hi, next.shadow_size));
        }
        l.runner.get_mut().unwrap().install_module_pipelines(l.id, l.modules[k], &pipelines).map_err(runner_error)?;
        l.twin_sources[k] = next.modules[module].naga.clone();
        let live = Instant::now();
        self.program = next;
        Ok(EditReceipt {
            compile: compiled - start,
            pipelines: created - compiled,
            upload: live - created,
            elapsed: live - start,
            dirtied_rows,
            caster_changed,
        })
    }
}
