//! Noise derivative library functions and the expression forms for higher derivatives.

use super::*;
use crate::{graph::{LibFn, lit}, wgsl};
use std::f64::consts::{SQRT_2, TAU};

const WGSL2: &str = r#"fn loft_noise2_grad(p: vec2<f32>) -> vec2<f32> {
  let ix = loft_lat(p.x);
  let iy = loft_lat(p.y);
  let f = p - floor(p);
  let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
  let du = 30.0 * f * f * (f - 1.0) * (f - 1.0);
  let ga = loft_grad2(loft_h2(ix, iy));
  let gb = loft_grad2(loft_h2(ix + 1u, iy));
  let gc = loft_grad2(loft_h2(ix, iy + 1u));
  let gd = loft_grad2(loft_h2(ix + 1u, iy + 1u));
  let va = dot(ga, f);
  let vb = dot(gb, f - vec2<f32>(1.0, 0.0));
  let vc = dot(gc, f - vec2<f32>(0.0, 1.0));
  let vd = dot(gd, f - vec2<f32>(1.0, 1.0));
  let k = va - vb - vc + vd;
  let g = ga + u.x * (gb - ga) + u.y * (gc - ga) + u.x * u.y * (ga - gb - gc + gd);
  return (g + du * vec2<f32>(vb - va + u.y * k, vc - va + u.x * k)) * {scale};
}"#;

const WGSL3: &str = r#"fn loft_noise3_grad(p: vec3<f32>) -> vec3<f32> {
  let ix = loft_lat(p.x);
  let iy = loft_lat(p.y);
  let iz = loft_lat(p.z);
  let f = p - floor(p);
  let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
  let du = 30.0 * f * f * (f - 1.0) * (f - 1.0);
  let g000 = loft_grad3(loft_h3(ix, iy, iz));
  let g100 = loft_grad3(loft_h3(ix + 1u, iy, iz));
  let g010 = loft_grad3(loft_h3(ix, iy + 1u, iz));
  let g110 = loft_grad3(loft_h3(ix + 1u, iy + 1u, iz));
  let g001 = loft_grad3(loft_h3(ix, iy, iz + 1u));
  let g101 = loft_grad3(loft_h3(ix + 1u, iy, iz + 1u));
  let g011 = loft_grad3(loft_h3(ix, iy + 1u, iz + 1u));
  let g111 = loft_grad3(loft_h3(ix + 1u, iy + 1u, iz + 1u));
  let v000 = dot(g000, f);
  let v100 = dot(g100, f - vec3<f32>(1.0, 0.0, 0.0));
  let v010 = dot(g010, f - vec3<f32>(0.0, 1.0, 0.0));
  let v110 = dot(g110, f - vec3<f32>(1.0, 1.0, 0.0));
  let v001 = dot(g001, f - vec3<f32>(0.0, 0.0, 1.0));
  let v101 = dot(g101, f - vec3<f32>(1.0, 0.0, 1.0));
  let v011 = dot(g011, f - vec3<f32>(0.0, 1.0, 1.0));
  let v111 = dot(g111, f - vec3<f32>(1.0, 1.0, 1.0));
  // Each mix(a, b, u) adds a + u (b - a) to the value and ga + u (gb - ga) + (b - a) u' e_axis to the gradient.
  let x00 = v000 + u.x * (v100 - v000);
  let x10 = v010 + u.x * (v110 - v010);
  let x01 = v001 + u.x * (v101 - v001);
  let x11 = v011 + u.x * (v111 - v011);
  let gx00 = g000 + u.x * (g100 - g000) + vec3<f32>(du.x * (v100 - v000), 0.0, 0.0);
  let gx10 = g010 + u.x * (g110 - g010) + vec3<f32>(du.x * (v110 - v010), 0.0, 0.0);
  let gx01 = g001 + u.x * (g101 - g001) + vec3<f32>(du.x * (v101 - v001), 0.0, 0.0);
  let gx11 = g011 + u.x * (g111 - g011) + vec3<f32>(du.x * (v111 - v011), 0.0, 0.0);
  let y0 = x00 + u.y * (x10 - x00);
  let y1 = x01 + u.y * (x11 - x01);
  let gy0 = gx00 + u.y * (gx10 - gx00) + vec3<f32>(0.0, du.y * (x10 - x00), 0.0);
  let gy1 = gx01 + u.y * (gx11 - gx01) + vec3<f32>(0.0, du.y * (x11 - x01), 0.0);
  return (gy0 + u.z * (gy1 - gy0) + vec3<f32>(0.0, 0.0, du.z * (y1 - y0))) * {scale};
}"#;

// Same normalisation as noise.ts for the range [-1, 1].
fn scale3() -> f64 { 2.0 / 3.0_f64.sqrt() }

impl Rules {
    pub(super) fn lib_rule(&mut self, a: &mut Arena, fun: LibId, args: &[Expr], ds: &[Expr]) -> Result<Expr, Error> {
        if let Some(rule) = self.registered.get(&fun) { return rule(a, args, ds); }
        let lib = a.lib_fn(fun);
        match lib.name.as_str() {
            // Hashes are piecewise constant between the cell walls.
            "loft_hash2" | "loft_hash3" | "loft_hash22" | "loft_hash33" => zero(a, lib.ret),
            "loft_noise2" | "loft_noise3" => {
                let ty = a[args[0]].ty;
                let grad = self.noise_grad(a, fun, ty)?;
                let g = a.lib(grad, &args[..1])?;
                s_dot(a, g, ds[0])
            }
            "loft_noise2_grad" | "loft_noise3_grad" => self.hessian(a, fun, args[0], ds[0]),
            "loft_solve_jet" => {
                let z = a.lift(0.0)?;
                let next = a.lib(fun, &[args[0], args[2], args[3], args[4], z])?;
                prim!(a, Add, ds[1], prim!(a, Mul, next, ds[0])?)
            }
            // Cellular needs the nearest feature point, which it does not
            // return. Screen-space derivatives are GPU-only.
            name => Err(Error::Scene(format!(
                "library function \"{name}\" has no registered derivative (register one with defineDerivative)"
            ))),
        }
    }

    /// Gradients remain one library call per octave, using the original
    /// noise function's helper objects so no shader defines a name twice.
    fn noise_grad(&mut self, a: &mut Arena, source: LibId, ty: Ty) -> Result<LibId, Error> {
        let (name, h, grad, templ, scale) = if ty == Ty::V2 {
            ("loft_noise2_grad", "loft_h2", "loft_grad2", WGSL2, SQRT_2)
        } else {
            ("loft_noise3_grad", "loft_h3", "loft_grad3", WGSL3, scale3())
        };
        if let Some(&f) = self.libs.get(name) { return Ok(f); }
        let deps = [h, "loft_lat", grad].map(|name| dep_named(a, source, name)).into_iter().collect::<Result<Vec<_>, _>>()?;
        Ok(self.add_lib(a, name, ty, templ.replace("{scale}", &lit::to_string(scale)), deps))
    }

    fn add_lib(&mut self, a: &mut Arena, name: &str, ret: Ty, wgsl: String, deps: Vec<LibId>) -> LibId {
        let id = a.add_lib(LibFn {
            name: name.to_owned(), ret, wgsl, deps: deps.into(), reads: Box::new([]),
            js: None, raw_compute: None,
        });
        self.libs.insert(name.to_owned(), id);
        id
    }

    /// Built once over private leaves, then substituted for each call. The
    /// expression form is larger than the library gradients used by normals.
    fn hessian(&mut self, a: &mut Arena, source: LibId, p: Expr, dp: Expr) -> Result<Expr, Error> {
        let ty = a[p].ty;
        let (at, dir, hv) = if let Some(&hit) = self.hessians.get(&ty) {
            hit
        } else {
            let name = &a.lib_fn(source).name;
            let (at, dir) = (format!("grad.{name}.p"), format!("grad.{name}.v"));
            let (at, dir) = (a.variable(&at, ty), a.variable(&dir, ty));
            let hash = self.hash(a, source, ty)?;
            let value = if ty == Ty::V2 { noise2_expr(a, at, hash)? } else { noise3_expr(a, at, hash)? };
            let parts = (0..usize::from(ty.lits())).map(|i| {
                let d = axis(a, ty, i)?;
                Ok(forward(a, self, &[value], &HashMap::from([(at, d)]))?[0])
            }).collect::<Result<Vec<_>, Error>>()?;
            let g = a.vec_of(ty, &parts)?;
            let hv = forward(a, self, &[g], &HashMap::from([(at, dir)]))?[0];
            self.hessians.insert(ty, (at, dir, hv));
            (at, dir, hv)
        };
        let hv = replace(a, &[hv], &HashMap::from([(at, p)]))?;
        Ok(replace(a, &hv, &HashMap::from([(dir, dp)]))?[0])
    }

    fn hash(&mut self, a: &mut Arena, source: LibId, ty: Ty) -> Result<LibId, Error> {
        let (name, ret, text, h) = if ty == Ty::V2 {
            ("loft_hash2", Ty::F32, wgsl::LOFT_HASH2, "loft_h2")
        } else {
            ("loft_hash33", Ty::V3, wgsl::LOFT_HASH33, "loft_h3")
        };
        if let Some(&f) = self.libs.get(name) { return Ok(f); }
        let mut deps = vec![dep_named(a, source, h)?, dep_named(a, source, "loft_lat")?, dep_named(a, source, "loft_unit")?];
        if ty == Ty::V3 { deps.push(dep_named(a, source, "loft_pcg")?); }
        Ok(self.add_lib(a, name, ret, text.to_owned(), deps))
    }
}

/// Find an existing object in the dependency tree in the same order as grad.ts.
fn dep_named(a: &Arena, root: LibId, name: &str) -> Result<LibId, Error> {
    fn find(a: &Arena, root: LibId, name: &str) -> Option<LibId> {
        a.lib_fn(root).deps.iter().find_map(|&d| {
            if a.lib_fn(d).name == name { Some(d) } else { find(a, d, name) }
        })
    }
    find(a, root, name).ok_or_else(|| Error::Internal(format!(
        "grad.ts: \"{}\" no longer depends on \"{name}\"; update the noise gradients to match noise.ts",
        a.lib_fn(root).name
    )))
}

fn quintic_e(a: &mut Arena, t: Expr) -> Result<Expr, Error> {
    let cube = prim!(a, Mul, prim!(a, Mul, t, t)?, t)?;
    let q = prim!(a, Add, prim!(a, Mul, t, prim!(a, Sub, prim!(a, Mul, t, 6.0)?, 15.0)?)?, 10.0)?;
    prim!(a, Mul, cube, q)
}
fn offset(a: &mut Arena, c: Expr, o: &[f64]) -> Result<Expr, Error> {
    if o.iter().all(|&x| x == 0.0) { Ok(c) } else { prim!(a, Add, c, const_of(a, o)?) }
}

/// Gradient noise expressed over the public hashes, as in the interval analysis.
fn noise2_expr(a: &mut Arena, p: Expr, hash: LibId) -> Result<Expr, Error> {
    let c = prim!(a, Floor, p)?;
    let fr = prim!(a, Sub, p, c)?;
    let w = quintic_e(a, fr)?;
    let corner = |a: &mut Arena, o: &[f64]| {
        let off = offset(a, c, o)?;
        let h = a.lib(hash, &[off])?;
        let angle = prim!(a, Mul, h, TAU)?;
        let g = [prim!(a, Cos, angle)?, prim!(a, Sin, angle)?];
        let g = a.vec_of(Ty::V2, &g)?;
        simplify!(a, s_dot, g, prim!(a, Sub, fr, const_of(a, o)?)?)
    };
    let x0 = prim!(a, Mix, corner(a, &[0.0, 0.0])?, corner(a, &[1.0, 0.0])?, a.get(w, "x")?)?;
    let x1 = prim!(a, Mix, corner(a, &[0.0, 1.0])?, corner(a, &[1.0, 1.0])?, a.get(w, "x")?)?;
    prim!(a, Mul, prim!(a, Mix, x0, x1, a.get(w, "y")?)?, SQRT_2)
}
fn noise3_expr(a: &mut Arena, p: Expr, hash: LibId) -> Result<Expr, Error> {
    let c = prim!(a, Floor, p)?;
    let fr = prim!(a, Sub, p, c)?;
    let w = quintic_e(a, fr)?;
    let corner = |a: &mut Arena, o: &[f64]| {
        let off = offset(a, c, o)?;
        let h = a.lib(hash, &[off])?;
        let z = prim!(a, Sub, 1.0, prim!(a, Mul, 2.0, a.get(h, "x")?)?)?;
        let angle = prim!(a, Mul, a.get(h, "y")?, TAU)?;
        let r = prim!(a, Sqrt, prim!(a, Mul, prim!(a, Sub, 1.0, z)?, prim!(a, Add, 1.0, z)?)?)?;
        let g = [prim!(a, Mul, r, prim!(a, Cos, angle)?)?, prim!(a, Mul, r, prim!(a, Sin, angle)?)?, z];
        let g = a.vec_of(Ty::V3, &g)?;
        simplify!(a, s_dot, g, prim!(a, Sub, fr, const_of(a, o)?)?)
    };
    let xs = |a: &mut Arena, y, z| prim!(a, Mix, corner(a, &[0.0, y, z])?, corner(a, &[1.0, y, z])?, a.get(w, "x")?);
    let y0 = prim!(a, Mix, xs(a, 0.0, 0.0)?, xs(a, 1.0, 0.0)?, a.get(w, "y")?)?;
    let y1 = prim!(a, Mix, xs(a, 0.0, 1.0)?, xs(a, 1.0, 1.0)?, a.get(w, "y")?)?;
    prim!(a, Mul, prim!(a, Mix, y0, y1, a.get(w, "z")?)?, scale3())
}
