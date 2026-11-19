use super::{closed, white};
use loft_passes::emit::paths::cpu::{self, Hit, Ray, Rgb, World};

struct BoxFurnace;

impl World for BoxFurnace {
    fn trace(&self, ray: Ray) -> Option<Hit> {
        let (axis, t) = (0..3).filter(|&i| ray.d[i] != 0.0)
            .map(|i| (i, (ray.d[i].signum() - ray.o[i]) / ray.d[i]))
            .filter(|(_, t)| *t > 0.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        let mut n = [0.0; 3];
        n[axis] = -ray.d[axis].signum();
        Some(Hit { p: std::array::from_fn(|i| ray.o[i] + t * ray.d[i]), n })
    }
    fn sky(&self, _: Rgb) -> Rgb { [0.0; 3] }
    fn eval(&self, hit: Hit, _: Rgb, wi: Rgb) -> Rgb {
        if wi == [0.0; 3] { return [1.0; 3]; }
        [0.5 * hit.n.into_iter().zip(wi).map(|(a,b)| a*b).sum::<f32>().max(0.0) * std::f32::consts::FRAC_1_PI; 3]
    }
    fn direct(&self, _: Hit, _: Rgb, _: u32, _: u32) -> Rgb { [0.0; 3] }
}

struct WhiteFurnace;

impl World for WhiteFurnace {
    fn trace(&self, ray: Ray) -> Option<Hit> {
        let dot = |a: Rgb, b: Rgb| a.into_iter().zip(b).map(|(x,y)| x*y).sum::<f32>();
        let b = dot(ray.o, ray.d);
        let disc = b*b + 1.0 - dot(ray.o, ray.o);
        if disc < 0.0 { return None; }
        let t = -b - disc.sqrt();
        if t <= 0.0 { return None; }
        let p = std::array::from_fn(|i| ray.o[i] + t * ray.d[i]);
        Some(Hit { p, n: p })
    }
    fn sky(&self, _: Rgb) -> Rgb { [0.5; 3] }
    fn eval(&self, hit: Hit, wo: Rgb, wi: Rgb) -> Rgb {
        let cosine = |w: Rgb| hit.n.into_iter().zip(w).map(|(a,b)| a*b).sum::<f32>().max(0.0);
        let (mu_o, mu_i) = (cosine(wo), cosine(wi));
        if mu_i == 0.0 { return [0.0; 3]; }
        // Exact directional albedo for roughness-one, unit-Fresnel,
        // height-correlated GGX. The complement is the multiscatter lobe.
        let lost = |mu: f32| mu * (1.0 + 1.0/mu).ln();
        let e_average = 4.0/3.0 * (1.0 - std::f32::consts::LN_2);
        let single = 0.5 / (mu_o + mu_i);
        let multiple = lost(mu_o)*lost(mu_i) / (1.0-e_average);
        [(single + multiple)*mu_i*std::f32::consts::FRAC_1_PI; 3]
    }
    fn direct(&self, _: Hit, _: Rgb, _: u32, _: u32) -> Rgb { [0.0; 3] }
}

fn mean(samples: u32, mut sample: impl FnMut(u32) -> Rgb) -> Rgb {
    let mut sum = [0.; 3];
    for i in 0..samples {
        for (acc, v) in sum.iter_mut().zip(sample(cpu::hash(i))) { *acc += f64::from(v); }
    }
    sum.map(|v| (v / f64::from(samples)) as f32)
}

#[test]
fn closed_furnace_twin() {
    closed(|depth, samples| {
        let got = mean(samples, |seed| cpu::sample(&BoxFurnace, Ray { o: [0.; 3], d: [0., 0., 1.] }, seed, depth));
        eprintln!("closed {depth} {got:?}");
        got
    });
}

#[test]
fn white_furnace_twin() {
    white(|pixel, samples| {
        let p = pixel.map(|v| (v as f32 + 0.5 - 16.0) / 16.0);
        let got = mean(samples, |seed| cpu::sample(&WhiteFurnace, Ray { o: [p[0], p[1], 3.0], d: [0., 0., -1.] }, seed, 1));
        eprintln!("white {pixel:?} {got:?}");
        got
    });
}

#[test]
fn thousand_lights_retarget_and_count_zero_candidates() {
    let target = |i: u32| 0.05 + f32::from((i % 29) as u8) / 28.0;
    let source = |i: u32| 0.15 + f32::from((i % 13) as u8) / 12.0;
    let expected: f64 = (0..1000).map(|i| f64::from(target(i))).sum();
    let count = 32768;
    let mut sum = 0.0;
    for i in 0..count {
        let mut old = cpu::Reservoir::default();
        let mut new = cpu::Reservoir::default();
        for j in 0..16 {
            let key = cpu::hash(i ^ 0x92d461ce);
            let light = (cpu::random(key, 2*j) * 1000.0) as u32;
            let xi = cpu::random(key, 2*j+1);
            if j < 8 { old.add(light, source(light), source(light)*1000.0, 1, xi); }
            else { new.add(light, target(light), target(light)*1000.0, 1, xi); }
        }
        new.reuse(&old, target(old.light), cpu::random(i, 42));
        sum += f64::from(new.scale()*target(new.light));
    }
    let mean = sum / f64::from(count);
    eprintln!("1000 lights {mean:.6} vs {expected:.6} ({:.4}%)", 100.0*(mean/expected-1.0));
    assert!((mean/expected-1.0).abs() <= 0.01);
    let mut black = cpu::Reservoir::default();
    black.add(7, 0.0, 0.0, 4, 0.1);
    let mut lit = cpu::Reservoir::default();
    lit.add(3, 1.0, 1.0, 1, 0.0);
    lit.reuse(&black, 0.0, 0.2);
    assert_eq!(lit.count, 5);
    assert_eq!(lit.scale(), 0.2);
}

#[test]
fn spectral_stack_and_integer_photons() {
    use cpu::{MediumStack, OpticalLayer};
    let layers = [
        OpticalLayer { boundary: 11, ior: 1.52, absorb: 0.3 },
        OpticalLayer { boundary: 12, ior: 1.33, absorb: 0.1 },
        OpticalLayer { boundary: 13, ior: 1.0, absorb: 0.0 },
    ];
    let mut stack = MediumStack::default();
    let mut eta2 = 1.0;
    for layer in layers {
        let (eta, through) = stack.interface(layer, true, 1.0, 0.999).unwrap();
        assert!(through);
        eta2 *= eta*eta;
    }
    assert!(stack.interface(layers[0], false, 1.0, 0.999).is_none());
    assert_eq!(stack.segment(2.0), 1.0);
    for layer in layers.into_iter().rev() {
        let (eta, through) = stack.interface(layer, false, 1.0, 0.999).unwrap();
        assert!(through);
        eta2 *= eta*eta;
    }
    assert!((eta2 - 1.0).abs() < 1e-6);
    stack.interface(layers[0], true, 1.0, 0.999).unwrap();
    assert_eq!(stack.interface(layers[0], false, 0.5, 0.999), Some((1.52, false)));
    assert!((stack.segment(2.0) - (-0.6_f32).exp()).abs() < 1e-7);
    let a = cpu::photon_fixed(0.25);
    let b = cpu::photon_fixed(0.75);
    assert_eq!(a + b, 16384);
    assert!(65536_u32.checked_mul(cpu::photon_fixed(1.0)).is_some());
}
