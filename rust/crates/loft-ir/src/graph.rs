//! The graph arena a job decodes into (plan §1.3): typed nodes in recording order, so every operand precedes its user,
//! and the declarations (functions, loops, kernels, ...) whose roots are nodes. Built only by `wire::decode`, which
//! validates every bound, tag and type rule; consumers read the public fields and never re-check them.
use serde::{Deserialize, Serialize};

/// A value's type. `Rec` is a multi-result function's record (its id is the recorder's).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ty {
    F32,
    V2,
    V3,
    V4,
    Bool,
    Rec(u8),
}

impl Ty {
    /// Wire code: 0..=4 the scalars and vectors, 8.. a record (8 + id). 5..=7 are refused.
    pub fn from_code(c: u8) -> Option<Ty> {
        match c {
            0 => Some(Ty::F32),
            1 => Some(Ty::V2),
            2 => Some(Ty::V3),
            3 => Some(Ty::V4),
            4 => Some(Ty::Bool),
            5..=7 => None,
            8..=255 => Some(Ty::Rec(c - 8)),
        }
    }
    /// Components of a number type; None for `Bool` and records.
    pub fn width(self) -> Option<u8> {
        match self {
            Ty::F32 => Some(1),
            Ty::V2 => Some(2),
            Ty::V3 => Some(3),
            Ty::V4 => Some(4),
            Ty::Bool | Ty::Rec(_) => None,
        }
    }
    fn of_width(w: u8) -> Option<Ty> {
        [Ty::F32, Ty::V2, Ty::V3, Ty::V4].get(usize::from(w).checked_sub(1)?).copied()
    }
}

/// Where a value comes from without an operation. Ids index `Graph::decls` (params, inputs) or name a binder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Leaf {
    Time,
    Param(u32),
    Input(u32),
    Bind(u32),
}

/// A swizzle selection: `len` components (1..=4), component k in bits 2k..2k+2 of `comps`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sel {
    pub len: u8,
    pub comps: u8,
}

/// What a node computes. Wire op codes: 0 Lit, 1 Leaf, 2 Call, 3 Fetch, 4 Loop, 5 Grad, 6 Raw, 7 Swizzle, 16+i
/// `Prim::ALL[i]`; 8..=15 are reserved and refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    /// The first of `width(ty)` entries of `Graph::lits` (exact f64 bits as authored; a bool is 0 or 1).
    Lit(u32),
    Leaf(Leaf),
    Prim(Prim),
    Swizzle(Sel),
    /// A recorded graph function (an `Fn` decl), called with the node's args; inlined during interning.
    Call(u32),
    /// vec4 lane `lane` of the row the f32 arg names, of a `Kernel` decl's resource.
    Fetch {
        kernel: u32,
        lane: u8,
    },
    /// Result component `k` of a `Loop` decl.
    Loop {
        decl: u32,
        k: u8,
    },
    /// d args[0] / d args[1]: a request, rewritten to its derivative graph during interning.
    Grad,
    /// A raw WGSL function (its source is sym `.0`), called with the node's args. GPU only.
    Raw(u32),
}

macro_rules! prims {
    ($($p:ident $name:literal $rule:ident),* $(,)?) => {
        /// The operators. One row each here (name and type rule); loft-core's `PRIMS` adds evaluation, interval and
        /// derivative rules in the same order (`Prim::ALL`), checked by a const assertion there.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum Prim { $($p),* }
        impl Prim {
            pub const ALL: &[Prim] = &[$(Prim::$p),*];
            /// The recorder's name, as `src/expr.ts` spells it (`Fn`), which the canonical print uses.
            pub fn name(self) -> &'static str { match self { $(Prim::$p => $name),* } }
            fn rule(self) -> Rule { match self { $(Prim::$p => Rule::$rule),* } }
        }
    };
}
prims! {
    Add "add" Map2, Sub "sub" Map2, Mul "mul" Map2, Div "div" Map2, Min "min" Map2, Max "max" Map2, Pow "pow" Map2,
    Mod "mod" Map2, Rem "rem" Map2, Atan2 "atan2" Map2, Step "step" Map2,
    Neg "neg" Map1, Abs "abs" Map1, Sign "sign" Map1, Floor "floor" Map1, Ceil "ceil" Map1, Trunc "trunc" Map1,
    RoundEven "roundEven" Map1, Fract "fract" Map1, Sqrt "sqrt" Map1, InverseSqrt "inverseSqrt" Map1, Exp "exp" Map1,
    Exp2 "exp2" Map1, Log "log" Map1, Log2 "log2" Map1, Sin "sin" Map1, Cos "cos" Map1, Tan "tan" Map1,
    Asin "asin" Map1, Acos "acos" Map1, Atan "atan" Map1, Sinh "sinh" Map1, Cosh "cosh" Map1, Tanh "tanh" Map1,
    Saturate "saturate" Map1, Normalize "normalize" Map1,
    Mix "mix" Map3, Clamp "clamp" Map3, Smoothstep "smoothstep" Map3,
    Length "length" Norm, Distance "distance" Dot, Dot "dot" Dot, Cross "cross" Cross,
    Lt "lt" Cmp, Gt "gt" Cmp, Le "le" Cmp, Ge "ge" Cmp, Eq "eq" Cmp, Ne "ne" Cmp,
    And "and" Logic, Or "or" Logic, Not "not" Not, Select "select" Select,
    Vec "vec" Vec, Pick "pick" Pick,
}

enum Rule {
    Map1,
    Map2,
    Map3,
    Norm,
    Dot,
    Cross,
    Cmp,
    Logic,
    Not,
    Select,
    Vec,
    Pick,
}

/// Component-wise result of two number types: equal, or a scalar splats to the other's width.
fn wider(a: Ty, b: Ty) -> Result<Ty, (Ty, Ty)> {
    match (a.width(), b.width()) {
        (Some(_), Some(_)) if a == b => Ok(a),
        (Some(1), Some(_)) => Ok(b),
        (Some(_), Some(1)) => Ok(a),
        _ => Err((a, b)),
    }
}

fn number(a: Ty) -> Result<Ty, (Ty, Ty)> {
    a.width().map(|_| a).ok_or((Ty::F32, a))
}

fn exactly(want: Ty, got: Ty) -> Result<Ty, (Ty, Ty)> {
    if want == got { Ok(got) } else { Err((want, got)) }
}

impl Prim {
    /// The result type of this operator over `args`, given the type the recorder wrote (needed only by `Vec`, whose
    /// target width is not implied by its parts' order). Err is (wanted, given) for the first operand that fails.
    /// Operand counts are the wire's to check (`arity`); a count that slips through is refused as (recorded, recorded).
    pub fn result(self, args: &[Ty], recorded: Ty) -> Result<Ty, (Ty, Ty)> {
        match (self.rule(), args) {
            (Rule::Map1, [a]) => number(*a),
            (Rule::Map2, [a, b]) => wider(number(*a)?, number(*b)?),
            (Rule::Map3, [a, b, c]) => wider(wider(number(*a)?, number(*b)?)?, number(*c)?),
            (Rule::Norm, [a]) => number(*a).map(|_| Ty::F32),
            (Rule::Dot, [a, b]) => exactly(number(*a)?, *b).map(|_| Ty::F32),
            (Rule::Cross, [a, b]) => exactly(Ty::V3, *a).and(exactly(Ty::V3, *b)),
            (Rule::Cmp, [a, b]) => exactly(Ty::F32, *a).and(exactly(Ty::F32, *b)).map(|_| Ty::Bool),
            (Rule::Logic, [a, b]) => exactly(Ty::Bool, *a).and(exactly(Ty::Bool, *b)),
            (Rule::Not, [a]) => exactly(Ty::Bool, *a),
            (Rule::Select, [c, a, b]) => exactly(Ty::Bool, *c).and(wider(number(*a)?, number(*b)?)),
            (Rule::Vec, parts) => {
                let mut sum = 0u8;
                for p in parts {
                    sum = sum.saturating_add(number(*p)?.width().unwrap_or(0));
                }
                let want = recorded.width().filter(|w| *w > 1).ok_or((Ty::V4, recorded))?;
                match parts {
                    [one] if *one == Ty::F32 => Ok(recorded),
                    _ if sum == want => Ok(recorded),
                    _ => Err((recorded, Ty::of_width(sum).unwrap_or(Ty::Rec(sum)))),
                }
            }
            (Rule::Pick, [i, first, rest @ ..]) => {
                exactly(Ty::F32, *i)?;
                rest.iter().try_fold(number(*first)?, |t, r| exactly(t, *r))
            }
            (
                Rule::Map1
                | Rule::Map2
                | Rule::Map3
                | Rule::Norm
                | Rule::Dot
                | Rule::Cross
                | Rule::Cmp
                | Rule::Logic
                | Rule::Not
                | Rule::Select
                | Rule::Pick,
                _,
            ) => Err((recorded, recorded)),
        }
    }
    /// Whether `n` operands is a legal count: fixed for most, 1..=4 parts for `Vec`, an index and 1+ values for `Pick`.
    pub fn takes(self, n: usize) -> bool {
        match self.rule() {
            Rule::Map1 | Rule::Norm | Rule::Not => n == 1,
            Rule::Map2 | Rule::Dot | Rule::Cross | Rule::Cmp | Rule::Logic => n == 2,
            Rule::Map3 | Rule::Select => n == 3,
            Rule::Vec => (1..=4).contains(&n),
            Rule::Pick => n >= 2,
        }
    }
}

/// A source position, the file resolved from the job's symbols.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Site {
    pub file: Box<str>,
    pub line: u32,
    pub col: u32,
}

/// One node: its operation, its type, and its operands as `args[args..args + n]` (node ids, all below its own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub op: Op,
    pub ty: Ty,
    pub args: u32,
    pub n: u32,
}

/// The eleven declaration kinds (plan §3.2). Wire tag = the order here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tag {
    Fn,
    Loop,
    Kernel,
    Draw,
    Pass,
    Medium,
    Check,
    Solve,
    Param,
    Input,
    Camera,
}

impl Tag {
    pub const ALL: [Tag; 11] = [
        Tag::Fn,
        Tag::Loop,
        Tag::Kernel,
        Tag::Draw,
        Tag::Pass,
        Tag::Medium,
        Tag::Check,
        Tag::Solve,
        Tag::Param,
        Tag::Input,
        Tag::Camera,
    ];
}

/// A declaration: its kind, a count whose meaning is the kind's (an `Fn`'s or `Loop`'s binders), an optional name
/// (sym id), and its roots as `roots[roots..roots + n]`. Root layouts checked at decode:
/// `Fn` [binder × arity, result..]; `Loop` [count, init × a, binder × a, index binder, next × a, stop?];
/// `Param` [default, min, max] f32 literals; `Input` [default] number literal. The others' layouts belong to the
/// lanes that define them (2.1, 2.3, 3.1) and are only bounds-checked here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decl {
    pub tag: Tag,
    pub arity: u32,
    pub name: Option<u32>,
    pub roots: u32,
    pub n: u32,
}

/// The program a job asks for (plan §3.1). Wire code = the order here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Program {
    Scene,
    Solve,
    Evaluate,
    Check,
}

/// A decoded, validated job graph. Spans are kept as decoded; `sym`, `args_of` and `roots_of` index them.
#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub nodes: Box<[Node]>,
    pub args: Box<[u32]>,
    /// f64 bits, exactly as authored.
    pub lits: Box<[u64]>,
    /// f32 bits of supplied samples (curve controls, author tables).
    pub rows: Box<[u32]>,
    pub blobs: Box<[u8]>,
    pub syms: Box<str>,
    pub sym_ends: Box<[u32]>,
    pub sites: Box<[Site]>,
    /// Per node: 0 = no site, k = `sites[k - 1]`.
    pub node_sites: Box<[u32]>,
    pub decls: Box<[Decl]>,
    pub roots: Box<[u32]>,
}

impl Graph {
    pub fn args_of(&self, node: &Node) -> &[u32] {
        &self.args[node.args as usize..(node.args + node.n) as usize]
    }
    pub fn roots_of(&self, decl: &Decl) -> &[u32] {
        &self.roots[decl.roots as usize..(decl.roots + decl.n) as usize]
    }
    pub fn sym(&self, i: u32) -> &str {
        let start = i.checked_sub(1).map_or(0, |p| self.sym_ends[p as usize]);
        &self.syms[start as usize..self.sym_ends[i as usize] as usize]
    }
    pub fn site(&self, node: u32) -> Option<&Site> {
        self.node_sites[node as usize].checked_sub(1).map(|k| &self.sites[k as usize])
    }
}
