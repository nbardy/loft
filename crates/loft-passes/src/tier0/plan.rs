//! Derive the private render plan from checked instructions. The wire program
//! remains intact, including when a bounded loop disappears from the plan.
use super::{Instruction, MAX_WORDS, Tape, allocate, author::CONSTANTS, operands};
use loft_abi::Tier0Opcode as Op;

pub(super) const LEAVES: usize = 4;
pub(super) const HEADER: usize = 6 + LEAVES * 5;
pub(super) const EXPANSION: usize = MAX_WORDS;

#[derive(Clone, Copy, Debug)]
pub(super) enum Operand {
    Zero,
    Constant(u16),
    Row(u16),
    Frame(u16),
}

impl Operand {
    fn encode(self) -> [u32; 2] {
        match self {
            Self::Zero => [0, 0],
            Self::Constant(i) => [1, u32::from(i)],
            Self::Row(i) => [2, u32::from(i)],
            Self::Frame(i) => [3, u32::from(i)],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Field {
    Sphere {
        center: Operand,
        radius: Operand,
    },
    Box {
        center: Operand,
        half: Operand,
    },
    Plane {
        axis: u16,
        direction: i8,
        offset: Operand,
    },
}

impl Field {
    pub(super) fn encode(self) -> [u32; 5] {
        let (op, center, extent) = match self {
            Self::Sphere { center, radius } => (42, center, radius),
            Self::Box { center, half } => (43, center, half),
            Self::Plane {
                axis,
                direction,
                offset,
            } => {
                let group = match direction {
                    1 => 7,
                    -1 => 10,
                    0 => 13,
                    _ => unreachable!("classified coordinate"),
                };
                (group + u32::from(axis), Operand::Zero, offset)
            }
        };
        let [ck, cs] = center.encode();
        let [ek, es] = extent.encode();
        [op, ck, cs, ek, es]
    }
}

#[derive(Debug)]
pub(super) enum FieldTree {
    Leaf(Field),
    Join {
        intersection: bool,
        a: Box<Self>,
        b: Box<Self>,
    },
}

impl FieldTree {
    pub(super) fn code(&self, leaves: &mut Vec<Field>, joins: &mut Vec<bool>) -> String {
        match self {
            Self::Leaf(field) => {
                leaves.push(*field);
                "x".into()
            }
            Self::Join { intersection, a, b } => {
                let a = a.code(leaves, joins);
                let b = b.code(leaves, joins);
                joins.push(*intersection);
                format!("({a}{b})")
            }
        }
    }
}

// Ordered tree shapes, independent of leaf values and min/max choices.
pub(super) const TREES: &[&str] = &[
    "x",
    "(xx)",
    "((xx)x)",
    "(x(xx))",
    "(((xx)x)x)",
    "((x(xx))x)",
    "((xx)(xx))",
    "(x((xx)x))",
    "(x(x(xx)))",
];

pub(super) enum Plan {
    Field(FieldTree),
    Run {
        words: Vec<u32>,
        prefix: usize,
        staged: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Dependence {
    Constant,
    Row,
    Frame,
    Sample,
}

#[derive(Clone, Copy)]
struct Node {
    ins: Instruction,
    dependence: Dependence,
}

impl Plan {
    pub(super) fn new(
        code: &[Instruction],
        wire: &[u32],
        constants: &mut Vec<[f32; 4]>,
        proof: &mut Vec<Option<[f64; 4]>>,
    ) -> Self {
        let Some((nodes, out)) = graph(code, constants, proof) else {
            return Self::wire(wire);
        };
        let mut leaves = LEAVES;
        if let Some(tree) = field(&nodes, out, &mut leaves) {
            return Self::Field(tree);
        }

        let mut live = vec![false; nodes.len()];
        live[out] = true;
        for (i, node) in nodes.iter().enumerate().rev() {
            if live[i] {
                let (n, _) = operands(node.ins.op);
                for &arg in &node.ins.args[..n] {
                    live[arg as usize] = true;
                }
            }
        }
        let mut schedule = Vec::new();
        for pass in [false, true] {
            for (i, node) in nodes.iter().enumerate() {
                if live[i] && (node.dependence == Dependence::Sample) == pass {
                    schedule.push(node.ins);
                }
            }
        }
        let prefix = nodes
            .iter()
            .zip(live)
            .filter(|(n, live)| *live && n.dependence != Dependence::Sample)
            .count();
        schedule.push(Instruction {
            op: Op::Ret,
            dst: 0,
            args: [out as u32, 0, 0],
            imm: 0,
        });
        match allocate(&schedule) {
            Ok(words) => Self::Run {
                words,
                prefix,
                staged: true,
            },
            Err(_) => Self::wire(wire),
        }
    }

    fn wire(words: &[u32]) -> Self {
        Self::Run {
            words: words.to_vec(),
            prefix: 0,
            staged: false,
        }
    }

    pub(super) fn encode(&self, header: &mut [u32; HEADER], bytes: &mut Vec<u8>) {
        match self {
            Self::Field(tree) => {
                let mut leaves = Vec::new();
                let mut joins = Vec::new();
                let shape = tree.code(&mut leaves, &mut joins);
                header[1] = match tree {
                    FieldTree::Leaf(Field::Sphere { .. }) => 1,
                    FieldTree::Leaf(Field::Box { .. }) => 2,
                    _ => {
                        3 + TREES
                            .iter()
                            .position(|x| *x == shape)
                            .expect("prewarmed tree") as u32
                    }
                };
                header[5] = joins
                    .iter()
                    .enumerate()
                    .fold(0, |mask, (i, &v)| mask | (u32::from(v) << i));
                for (out, leaf) in header[6..].chunks_exact_mut(5).zip(leaves) {
                    out.copy_from_slice(&leaf.encode());
                }
            }
            Self::Run { words, prefix, .. } => {
                header[4] = words.len() as u32;
                header[5] = *prefix as u32;
                for &word in words {
                    let (op, dst, a, b, imm) = Op::unpack(word).expect("checked tape");
                    let control = (u32::from(imm) << 8) | op as u32;
                    let registers = u32::from_le_bytes([dst, a, b, (imm & 31) as u8]);
                    bytes.extend(control.to_le_bytes());
                    bytes.extend(registers.to_le_bytes());
                }
            }
        }
    }

    pub(super) fn describe(&self) -> String {
        match self {
            Self::Field(FieldTree::Leaf(leaf)) => format!("{leaf:?}"),
            Self::Field(tree) => format!("fused {tree:?}"),
            Self::Run {
                words,
                prefix,
                staged,
            } => {
                let loops = words.iter().any(|&w| w >> 26 == Op::LoopBegin as u32);
                format!(
                    "residual block PC {prefix}..{}, {} visits per sample before loops, staged {staged}, loop {loops}, bank {} B, loop stack {} B",
                    words.len(),
                    words.len() - prefix,
                    super::author::live_registers(words) * 16,
                    if loops { 128 } else { 0 }
                )
            }
        }
    }
}

fn graph(
    code: &[Instruction],
    constants: &mut Vec<[f32; 4]>,
    proof: &mut Vec<Option<[f64; 4]>>,
) -> Option<(Vec<Node>, usize)> {
    let mut nodes = Vec::<Node>::new();
    let mut bindings = vec![0; code.len()];
    let mut loops = Vec::<u32>::new();
    let mut pc = 0;
    for _ in 0..EXPANSION {
        let mut ins = *code.get(pc)?;
        let (n, writes) = operands(ins.op);
        for arg in &mut ins.args[..n] {
            *arg = bindings[*arg as usize];
        }
        let constant = |i: u32| {
            let ins = nodes[i as usize].ins;
            (ins.op == Op::LoadConst).then(|| constants[ins.imm as usize])
        };
        match ins.op {
            Op::Nop => {}
            Op::Mov => bindings[ins.dst as usize] = ins.args[0],
            Op::Ret => return Some((nodes, ins.args[0] as usize)),
            Op::LoopBegin => {
                let trips = constant(ins.args[0])?[0] as u32;
                if trips == 0 {
                    pc = usize::from(ins.imm) + 1;
                    continue;
                }
                loops.push(trips);
            }
            Op::LoopEnd => {
                let trips = loops.last_mut()?;
                *trips -= 1;
                if *trips != 0 {
                    pc = usize::from(ins.imm) + 1;
                    continue;
                }
                loops.pop();
            }
            _ => {
                debug_assert!(writes);
                let original = ins.dst as usize;
                ins.dst = nodes.len() as u32;
                let mut dependence = match ins.op {
                    Op::LoadConst => Dependence::Constant,
                    Op::LoadRow => Dependence::Row,
                    Op::LoadFrame => Dependence::Frame,
                    Op::LoadInput => Dependence::Sample,
                    _ => ins.args[..n]
                        .iter()
                        .map(|&i| nodes[i as usize].dependence)
                        .max()
                        .unwrap_or(Dependence::Constant),
                };
                if dependence == Dependence::Constant
                    && n != 0
                    && constants.len() < CONSTANTS
                    && let Some(value) = fold(ins, &constant, n)
                {
                    let proof_value =
                        proof_fold(ins, |i| proof[nodes[i as usize].ins.imm as usize]);
                    proof.push(proof_value);
                    ins.op = Op::LoadConst;
                    ins.imm = constants.len() as u16;
                    ins.args = [0; 3];
                    constants.push(value);
                    dependence = Dependence::Constant;
                }
                bindings[original] = ins.dst;
                nodes.push(Node { ins, dependence });
            }
        }
        pc += 1;
    }
    None
}

fn fold(
    ins: Instruction,
    constant: &impl Fn(u32) -> Option<[f32; 4]>,
    n: usize,
) -> Option<[f32; 4]> {
    // Transcendental and resource operations execute on the GPU. Fold the
    // portable finite arithmetic that also constructs vector literals.
    if !matches!(
        ins.op,
        Op::Swizzle
            | Op::Splat
            | Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Neg
            | Op::Abs
            | Op::Min
            | Op::Max
            | Op::Lt
            | Op::Le
            | Op::Eq
            | Op::Select
    ) {
        return None;
    }
    let mut args = [[0.; 4]; 3];
    for (out, &arg) in args.iter_mut().zip(&ins.args[..n]) {
        *out = constant(arg)?;
    }
    let value = Tape::fold(ins.op, ins.imm, n, args);
    value.into_iter().all(f32::is_finite).then_some(value)
}

fn proof_fold(ins: Instruction, get: impl Fn(u32) -> Option<[f64; 4]>) -> Option<[f64; 4]> {
    let a = get(ins.args[0])?;
    let binary = |f: fn(f64, f64) -> f64| {
        let b = get(ins.args[1])?;
        Some(std::array::from_fn(|i| f(a[i], b[i])))
    };
    match ins.op {
        Op::Splat => Some([a[usize::from(ins.imm & 3)]; 4]),
        Op::Swizzle => Some(std::array::from_fn(|i| {
            a[usize::from(ins.imm >> (2 * i) & 3)]
        })),
        Op::Add => binary(|a, b| a + b),
        Op::Sub => binary(|a, b| a - b),
        Op::Mul => binary(|a, b| a * b),
        Op::Div => binary(|a, b| a / b),
        Op::Min => binary(f64::min),
        Op::Max => binary(f64::max),
        Op::Neg => Some(a.map(|x| -x)),
        Op::Abs => Some(a.map(f64::abs)),
        _ => None,
    }
    .filter(|v| v.iter().all(|x| x.is_finite()))
}

fn field(nodes: &[Node], out: usize, leaves: &mut usize) -> Option<FieldTree> {
    if *leaves == 0 {
        return None;
    }
    let ins = nodes[out].ins;
    let a = ins.args[0] as usize;
    let b = ins.args[1] as usize;
    if matches!(ins.op, Op::Min | Op::Max) {
        return Some(FieldTree::Join {
            intersection: ins.op == Op::Max,
            a: Box::new(field(nodes, a, leaves)?),
            b: Box::new(field(nodes, b, leaves)?),
        });
    }
    let operand = |i: usize| {
        let ins = nodes[i].ins;
        match ins.op {
            Op::LoadConst => Some(Operand::Constant(ins.imm)),
            Op::LoadRow => Some(Operand::Row(ins.imm)),
            Op::LoadFrame => Some(Operand::Frame(ins.imm)),
            _ => None,
        }
    };
    let position = |i: usize| nodes[i].ins.op == Op::LoadInput && nodes[i].ins.imm == 0;
    let field = match ins.op {
        Op::SdSphere | Op::SdBox => {
            let point = nodes[a].ins;
            let center = match point.op {
                Op::LoadInput if position(a) => Operand::Zero,
                Op::Sub if position(point.args[0] as usize) => operand(point.args[1] as usize)?,
                _ => return None,
            };
            let extent = operand(b)?;
            if ins.op == Op::SdSphere {
                Field::Sphere {
                    center,
                    radius: extent,
                }
            } else {
                Field::Box {
                    center,
                    half: extent,
                }
            }
        }
        Op::Sub => {
            let (axis, direction, offset) = match (coordinate(nodes, a), coordinate(nodes, b)) {
                (Some((axis, dir)), None) => (axis, dir, operand(b)?),
                _ => return None,
            };
            Field::Plane {
                axis,
                direction,
                offset,
            }
        }
        _ => return None,
    };
    *leaves -= 1;
    Some(FieldTree::Leaf(field))
}

fn coordinate(nodes: &[Node], i: usize) -> Option<(u16, i8)> {
    let ins = nodes[i].ins;
    let a = nodes[ins.args[0] as usize].ins;
    match ins.op {
        Op::Splat if a.op == Op::LoadInput && a.imm == 0 && ins.imm < 3 => Some((ins.imm, 1)),
        Op::Abs => coordinate(nodes, ins.args[0] as usize).map(|(axis, _)| (axis, 0)),
        Op::Neg => coordinate(nodes, ins.args[0] as usize)
            .and_then(|(axis, dir)| (dir != 0).then_some((axis, -dir))),
        _ => None,
    }
}
