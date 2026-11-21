//! A single render-kind tape. Motion and raw shader code stay in Tier 1.
#[path = "tier0_author.rs"]
mod author;
pub use author::{TAPE_BYTES, TraceEdit, authored_trace};
#[path = "tier0/session.rs"]
mod session;
use loft_abi::Tier0Opcode as Op;
pub use session::{EditClass, PreparedEdit, TraceSource};
use std::collections::BTreeMap;

pub const REGISTERS: usize = 32;
pub const MAX_WORDS: usize = 2048;
pub const VM_WGSL: &str = concat!(
    include_str!("tier0/compute.wgsl"),
    include_str!("tier0.wgsl")
);
const VM_OPERATIONS: &str = include_str!("tier0.wgsl");

type V = [f32; 4];

#[derive(Clone, Copy, Debug)]
pub struct Instruction {
    pub op: Op,
    /// Virtual register written by the instruction, when it produces a value.
    pub dst: u32,
    pub args: [u32; 3],
    /// Constant, load slot, lane selectors, octave count or matching loop PC.
    pub imm: u16,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TapeError {
    Size,
    InvalidWord(usize),
    Loop(usize),
    MissingReturn,
    Undefined(usize, u32),
    Registers(usize),
    Resource(usize),
}

// Ret consumes a value. A loop consumes the trip count once, at entry.
fn operands(op: Op) -> (usize, bool) {
    use Op::*;
    match op {
        Nop | LoopEnd => (0, false),
        LoadConst | LoadRow | LoadInput | LoadFrame => (0, true),
        LoopBegin | Ret => (1, false),
        Mov | Swizzle | Splat | Neg | Abs | Floor | Fract | Sqrt | Sin | Cos | Exp | Log
        | LenNorm | XformPt | XformDir | MirrorX | GradNoise3 | SmoothVor3 | Fbm2 | SampleLutC2
        | SampleTex2d => (1, true),
        Add | Sub | Mul | Div | Min | Max | Lt | Le | Eq | Step | Atan2 | Pow | Dot3 | Cross
        | RepeatCell | SdSphere | SdBox | SminPoly | SsubPoly => (2, true),
        Fma | Clamp | Mix | Select | Smoothstep | Smootherstep | SdRoundBox | SdCapsule => {
            (3, true)
        }
    }
}

/// Allocate live intervals in instruction order. Endpoints are inclusive at
/// reads, so a destination can reuse one of its last-use operands.
pub fn allocate(code: &[Instruction]) -> Result<Vec<u32>, TapeError> {
    if code.is_empty() || code.len() > MAX_WORDS {
        return Err(TapeError::Size);
    }
    let mut intervals = BTreeMap::<u32, (usize, usize)>::new();
    let mut loops = Vec::new();
    let mut conditional = BTreeMap::new();
    for (pc, ins) in code.iter().enumerate() {
        let (n, writes) = operands(ins.op);
        for &value in &ins.args[..n] {
            if conditional
                .get(&value)
                .is_some_and(|&start| !loops.contains(&start))
            {
                return Err(TapeError::Undefined(pc, value));
            }
            intervals
                .get_mut(&value)
                .ok_or(TapeError::Undefined(pc, value))?
                .1 = pc;
        }
        if writes {
            intervals
                .entry(ins.dst)
                .and_modify(|span| span.1 = pc)
                .or_insert_with(|| {
                    if let Some(&start) = loops.last() {
                        conditional.insert(ins.dst, start);
                    }
                    (pc, pc)
                });
        }
        match ins.op {
            Op::LoopBegin => loops.push(pc),
            Op::LoopEnd => {
                let start = loops.pop().ok_or(TapeError::Loop(pc))?;
                // A value entering the loop stays live over the back edge.
                for (first, last) in intervals.values_mut() {
                    if *first < start && *last >= start {
                        *last = pc;
                    }
                }
            }
            _ => {}
        }
    }
    let mut assignments = BTreeMap::<u32, u8>::new();
    let mut active = [None; REGISTERS];
    let mut words = Vec::with_capacity(code.len());
    for (pc, ins) in code.iter().enumerate() {
        let (n, writes) = operands(ins.op);
        let mut regs = [0; 3];
        for (r, value) in regs.iter_mut().zip(&ins.args[..n]) {
            *r = assignments[value];
        }
        for end in &mut active {
            if end.is_some_and(|end| end <= pc) {
                *end = None;
            }
        }
        let mut dst = 0;
        if writes {
            let (start, end) = intervals[&ins.dst];
            dst = if start == pc {
                let free = active
                    .iter()
                    .position(Option::is_none)
                    .ok_or(TapeError::Registers(pc))? as u8;
                assignments.insert(ins.dst, free);
                free
            } else {
                assignments[&ins.dst]
            };
            if end > pc {
                active[usize::from(dst)] = Some(end);
            }
        }
        let imm = if n == 3 { u16::from(regs[2]) } else { ins.imm };
        words.push(
            ins.op
                .pack(dst, regs[0], regs[1], imm)
                .ok_or(TapeError::InvalidWord(pc))?,
        );
    }
    Tape::new(&words)?;
    Ok(words)
}

pub struct Tape(Vec<(Op, u8, u8, u8, u16)>);

impl Tape {
    pub fn new(words: &[u32]) -> Result<Self, TapeError> {
        if words.is_empty() || words.len() > MAX_WORDS {
            return Err(TapeError::Size);
        }
        let mut code = Vec::with_capacity(words.len());
        let mut loops = Vec::new();
        for (pc, &word) in words.iter().enumerate() {
            let ins = Op::unpack(word).ok_or(TapeError::InvalidWord(pc))?;
            let end = usize::from(ins.4);
            match ins.0 {
                Op::LoopBegin if loops.len() < REGISTERS && end > pc && end < words.len() => {
                    loops.push((pc, end))
                }
                Op::LoopBegin => return Err(TapeError::Loop(pc)),
                Op::LoopEnd if loops.pop() != Some((end, pc)) => return Err(TapeError::Loop(pc)),
                Op::Ret if pc + 1 != words.len() || !loops.is_empty() => {
                    return Err(TapeError::MissingReturn);
                }
                _ => {}
            }
            code.push(ins);
        }
        if !matches!(code.last(), Some((Op::Ret, ..))) {
            return Err(TapeError::MissingReturn);
        }
        Ok(Self(code))
    }

    pub fn run(&self, resources: &Resources<'_>) -> Result<V, TapeError> {
        use Op::*;
        let mut regs = [[0.; 4]; REGISTERS];
        let mut pc = 0;
        let mut loops = Vec::new();
        loop {
            let (op, dst, a, b, imm) = self.0[pc];
            let [a, b, c] = [a, b, (imm & 31) as u8].map(|r| regs[r as usize]);
            let imm = usize::from(imm);
            let get = |data: &[V]| data.get(imm).copied().ok_or(TapeError::Resource(pc));
            let binary = |f: fn(f32, f32) -> f32| std::array::from_fn(|i| f(a[i], b[i]));
            let ternary =
                |f: fn(f32, f32, f32) -> f32| std::array::from_fn(|i| f(a[i], b[i], c[i]));
            let value = match op {
                Nop => {
                    pc += 1;
                    continue;
                }
                LoadConst => get(resources.constants)?,
                LoadRow => get(resources.row)?,
                LoadInput => get(resources.inputs)?,
                LoadFrame => get(resources.frame)?,
                Mov => a,
                Swizzle => std::array::from_fn(|i| a[(imm >> (2 * i)) & 3]),
                Splat => [a[imm & 3]; 4],
                Add => binary(|a, b| a + b),
                Sub => binary(|a, b| a - b),
                Mul => binary(|a, b| a * b),
                Div => binary(|a, b| a / b),
                Fma => ternary(f32::mul_add),
                Neg => a.map(|x| -x),
                Abs => a.map(f32::abs),
                Min => binary(f32::min),
                Max => binary(f32::max),
                Clamp => ternary(|x, lo, hi| x.max(lo).min(hi)),
                Mix => ternary(mix),
                Select => {
                    if c[0] != 0. {
                        b
                    } else {
                        a
                    }
                }
                Lt => binary(|a, b| f32::from(a < b)),
                Le => binary(|a, b| f32::from(a <= b)),
                Eq => binary(|a, b| f32::from(a == b)),
                Step => binary(|a, b| f32::from(b >= a)),
                Smoothstep => ternary(|lo, hi, x| {
                    let t = ((x - lo) / (hi - lo)).clamp(0., 1.);
                    t * t * (3. - 2. * t)
                }),
                Smootherstep => ternary(|lo, hi, x| quintic(((x - lo) / (hi - lo)).clamp(0., 1.))),
                Floor => a.map(f32::floor),
                Fract => a.map(fract),
                Sqrt => a.map(f32::sqrt),
                Sin => a.map(f32::sin),
                Cos => a.map(f32::cos),
                Atan2 => binary(f32::atan2),
                Exp => a.map(f32::exp),
                Log => a.map(f32::ln),
                Pow => binary(f32::powf),
                Dot3 => [dot(a, b); 4],
                Cross => [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                    0.,
                ],
                LenNorm => {
                    let l = dot(a, a).sqrt();
                    [a[0] / l, a[1] / l, a[2] / l, l]
                }
                XformPt | XformDir => {
                    let m = resources
                        .constants
                        .get(imm..imm + 4)
                        .ok_or(TapeError::Resource(pc))?;
                    let w = f32::from(op == XformPt);
                    std::array::from_fn(|i| {
                        m[0][i] * a[0] + m[1][i] * a[1] + m[2][i] * a[2] + m[3][i] * w
                    })
                }
                MirrorX => [
                    a[0].abs(),
                    a[1],
                    a[2],
                    f32::from(a[0] > 0.) - f32::from(a[0] < 0.),
                ],
                RepeatCell => std::array::from_fn(|i| {
                    if i == 3 {
                        a[i]
                    } else {
                        a[i] - b[i] * (a[i] / b[i] + 0.5).floor()
                    }
                }),
                SdSphere => [dot(a, a).sqrt() - b[0]; 4],
                SdBox => [sd_box(a, b); 4],
                SdRoundBox => [sd_box(a, b.map(|x| x - c[0])) - c[0]; 4],
                SdCapsule => {
                    let pa = std::array::from_fn(|i| a[i] - b[i]);
                    let ba = std::array::from_fn(|i| c[i] - b[i]);
                    let h = (dot(pa, ba) / dot(ba, ba).max(1e-12)).clamp(0., 1.);
                    let d = std::array::from_fn(|i| pa[i] - ba[i] * h);
                    [dot(d, d).sqrt() - c[3]; 4]
                }
                SminPoly => [smin(a[0], b[0], get(resources.constants)?[0]); 4],
                SsubPoly => [-smin(-a[0], b[0], get(resources.constants)?[0]); 4],
                GradNoise3 => [noise(a); 4],
                SmoothVor3 => [voronoi(a); 4],
                Fbm2 => {
                    let (mut p, mut gain, mut out) = ([a[0], a[1], 0., 0.], 1., 0.);
                    for _ in 0..imm {
                        out += gain * noise(p);
                        p = p.map(|x| x * 2.);
                        gain *= 0.5;
                    }
                    [out; 4]
                }
                SampleLutC2 => {
                    let info = get(resources.lut)?;
                    let start = info[0] as usize;
                    let end = start
                        .checked_add(info[1] as usize)
                        .ok_or(TapeError::Resource(pc))?;
                    let lut = resources
                        .lut
                        .get(start..end)
                        .filter(|s| !s.is_empty())
                        .ok_or(TapeError::Resource(pc))?;
                    let x = a[0].clamp(0., 1.) * (lut.len() - 1) as f32;
                    let i = x as usize;
                    std::array::from_fn(|j| {
                        mix(
                            lut[i][j],
                            lut[(i + 1).min(lut.len() - 1)][j],
                            quintic(fract(x)),
                        )
                    })
                }
                SampleTex2d => resources
                    .textures
                    .get(imm)
                    .and_then(|texture| texture.sample([a[0], a[1]]))
                    .ok_or(TapeError::Resource(pc))?,
                LoopBegin => {
                    let count = a[0] as u32;
                    if count == 0 {
                        pc = imm + 1;
                    } else {
                        loops.push(count);
                        pc += 1;
                    }
                    continue;
                }
                LoopEnd => {
                    let left = loops.last_mut().expect("validated loop");
                    *left -= 1;
                    if *left == 0 {
                        loops.pop();
                        pc += 1;
                    } else {
                        pc = imm + 1;
                    }
                    continue;
                }
                Ret => return Ok(a),
            };
            regs[dst as usize] = value;
            pc += 1;
        }
    }
}

pub struct Resources<'a> {
    pub constants: &'a [V],
    pub row: &'a [V],
    pub inputs: &'a [V],
    pub frame: &'a [V],
    /// Entries referenced by the tape give the first texel and texel count in x/y.
    pub lut: &'a [V],
    pub textures: &'a [Texture<'a>],
}

pub struct Texture<'a> {
    pub size: [usize; 2],
    pub texels: &'a [V],
}

impl Texture<'_> {
    fn sample(&self, uv: [f32; 2]) -> Option<V> {
        let [w, h] = self.size;
        if w == 0 || h == 0 || w.checked_mul(h)? != self.texels.len() {
            return None;
        }
        let [x, y] = std::array::from_fn(|i| uv[i].clamp(0., 1.) * self.size[i] as f32 - 0.5);
        let range = |p: f32, n: usize| {
            let i = p.floor() as isize;
            let to_index = |k: isize| k.clamp(0, n as isize - 1) as usize;
            (to_index(i), to_index(i + 1))
        };
        let (x0, x1) = range(x, w);
        let (y0, y1) = range(y, h);
        let at = |x, y| self.texels[y * w + x];
        let [a, b, c, d] = [at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1)];
        Some(std::array::from_fn(|i| {
            mix(
                mix(a[i], b[i], fract(x)),
                mix(c[i], d[i], fract(x)),
                fract(y),
            )
        }))
    }
}

fn dot(a: V, b: V) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn fract(x: f32) -> f32 {
    x - x.floor()
}
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a * (1. - t) + b * t
}
fn quintic(t: f32) -> f32 {
    t * t * t * ((6. * t - 15.) * t + 10.)
}
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = (k - (a - b).abs()).max(0.) / k;
    a.min(b) - h * h * k * 0.25
}
fn sd_box(p: V, b: V) -> f32 {
    let q: V = std::array::from_fn(|i| p[i].abs() - b[i]);
    let pos = q.map(|x| x.max(0.));
    dot(pos, pos).sqrt() + q[0].max(q[1].max(q[2])).min(0.)
}
fn hash(p: [i32; 3]) -> u32 {
    let [x, y, z] = p.map(|v| v as u32);
    let h = x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663) ^ z.wrapping_mul(83492791);
    let h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}
fn noise(p: V) -> f32 {
    let cell = p.map(|x| x.floor() as i32);
    let f = p.map(fract);
    let u = f.map(quintic);
    let mut out = 0.;
    for corner in 0..8 {
        let bits: [i32; 3] = std::array::from_fn(|i| (corner >> i) & 1);
        let h = hash(std::array::from_fn(|i| cell[i].wrapping_add(bits[i])));
        let mut grad = [0.; 4];
        for i in 0..3 {
            grad[i] = if (h >> i) & 1 == 0 { -1. } else { 1. };
        }
        let d: V = std::array::from_fn(|i| if i < 3 { f[i] - bits[i] as f32 } else { 0. });
        let w = |i: usize| if bits[i] == 0 { 1. - u[i] } else { u[i] };
        out += dot(grad, d) * (w(0) * w(1) * w(2));
    }
    out * 0.57735026
}
fn voronoi(p: V) -> f32 {
    let cell = p.map(|x| x.floor() as i32);
    let f = p.map(fract);
    let mut out = f32::INFINITY;
    for z in -1i32..=1 {
        for y in -1i32..=1 {
            for x in -1i32..=1 {
                let d = [x, y, z];
                let h = hash(std::array::from_fn(|i| cell[i].wrapping_add(d[i])));
                let q: V = std::array::from_fn(|i| {
                    if i < 3 {
                        d[i] as f32 + ((h >> (8 * i)) & 255) as f32 / 256. - f[i]
                    } else {
                        0.
                    }
                });
                let distance = dot(q, q).sqrt();
                out = if out.is_infinite() {
                    distance
                } else {
                    smin(out, distance, 0.12)
                };
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "tier0/diagnostics.rs"]
mod diagnostics;
